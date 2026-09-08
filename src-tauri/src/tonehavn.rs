//! Trusted Tonehavn authentication. Secrets stay in the native credential store.
use keyring::Entry;
use reqwest::{Method, StatusCode, Url, blocking::Client, header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, sync::Mutex, time::Duration};

static AUTH_LOCK: Mutex<()> = Mutex::new(());
const SESSION_COOKIE: &str = "__Host-tonehavn_session";
const CSRF_COOKIE: &str = "__Host-tonehavn_csrf";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    server: String,
    device_id: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: String::new(),
            device_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Credential {
    server: String,
    token: String,
    csrf: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    mode: String,
    expires_at: i64,
    csrf_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    server: String,
    signed_in: bool,
    saved_session: bool,
    expires_at: Option<i64>,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Login {
    server: String,
    username: String,
    password: String,
    totp_code: String,
}

fn origin(input: &str) -> Result<String, String> {
    let url = Url::parse(input.trim()).map_err(|_| "Enter Tonehavn’s HTTPS server address.")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Use an HTTPS server address without a path, credentials, query, or fragment.".into(),
        );
    }
    Ok(url.origin().ascii_serialization())
}

fn read_config(root: &Path) -> Result<Config, String> {
    let path = root.join("tonehavn-auth.json");
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "Could not read Tonehavn connection settings.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(_) => Err("Could not read Tonehavn connection settings.".into()),
    }
}

fn save_config(root: &Path, config: &Config) -> Result<(), String> {
    use std::io::Write;
    let mut file =
        tempfile::NamedTempFile::new_in(root).map_err(|_| "Could not save Tonehavn settings.")?;
    file.write_all(&serde_json::to_vec(config).map_err(|_| "Could not encode Tonehavn settings.")?)
        .map_err(|_| "Could not save Tonehavn settings.")?;
    file.persist(root.join("tonehavn-auth.json"))
        .map_err(|_| "Could not save Tonehavn settings.")?;
    Ok(())
}

fn entry(server: &str) -> Result<Entry, String> {
    let key = format!("session-{:x}", Sha256::digest(server.as_bytes()));
    Entry::new("com.soundtrackgeek.aurora.tonehavn", &key)
        .map_err(|_| "Could not open the system credential store.")
        .map_err(String::from)
}

fn token_is_valid(token: &str) -> bool {
    (32..=512).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn read_credential(server: &str) -> Result<Option<Credential>, String> {
    match entry(server)?.get_password() {
        Ok(value) => {
            let credential: Credential = serde_json::from_str(&value)
                .map_err(|_| "The saved Tonehavn session is invalid.")?;
            if credential.server != server
                || !token_is_valid(&credential.token)
                || !token_is_valid(&credential.csrf)
            {
                return Err("The saved Tonehavn session is invalid.".into());
            }
            Ok(Some(credential))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => {
            Err("Could not read Tonehavn credentials from the system credential store.".into())
        }
    }
}

fn save_credential(credential: &Credential) -> Result<(), String> {
    entry(&credential.server)?
        .set_password(
            &serde_json::to_string(credential).map_err(|_| "Could not encode the session.")?,
        )
        .map_err(|_| "Could not save the Tonehavn session in the system credential store.".into())
}

fn clear_credential(server: &str) -> Result<(), String> {
    match entry(server)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Could not remove the saved Tonehavn session.".into()),
    }
}

fn client() -> Result<Client, String> {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("Aurora/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Could not create the Tonehavn connection.".into())
}

fn request(
    client: &Client,
    method: Method,
    server: &str,
    path: &str,
    credential: Option<&Credential>,
) -> reqwest::blocking::RequestBuilder {
    let mut request = client
        .request(method, format!("{server}{path}"))
        .header(header::ORIGIN, server);
    if let Some(c) = credential {
        request = request
            .header(
                header::COOKIE,
                format!("{SESSION_COOKIE}={}; {CSRF_COOKIE}={}", c.token, c.csrf),
            )
            .header("x-tonehavn-csrf", &c.csrf);
    }
    request
}

fn response_error(code: StatusCode) -> String {
    match code {
        StatusCode::UNAUTHORIZED => "Sign-in was rejected or the session expired. Check your credentials and use a fresh authenticator code.",
        StatusCode::FORBIDDEN => "Tonehavn rejected this request. Check that the server address matches its configured HTTPS address.",
        StatusCode::TOO_MANY_REQUESTS => "Tonehavn has temporarily limited sign-in attempts. Wait before trying again.",
        _ => "Tonehavn could not complete the request. Check the server and try again.",
    }.into()
}

fn envelope(response: reqwest::blocking::Response) -> Result<Envelope, String> {
    if !response.status().is_success() {
        return Err(response_error(response.status()));
    }
    let mut bytes = Vec::new();
    response
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read Tonehavn’s response.")?;
    parse_envelope(&bytes)
}

fn parse_envelope(bytes: &[u8]) -> Result<Envelope, String> {
    if bytes.len() > 65_536 {
        return Err("Tonehavn returned an oversized session response.".into());
    }
    let value: Envelope = serde_json::from_slice(bytes)
        .map_err(|_| "Tonehavn returned an invalid session response.")?;
    if value.mode != "trusted"
        || !token_is_valid(&value.csrf_token)
        || value.expires_at <= crate::state_sync::now_ms()
    {
        return Err("Tonehavn did not return a current trusted session.".into());
    }
    Ok(value)
}

fn session_token(headers: &header::HeaderMap) -> Result<String, String> {
    let mut tokens = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .filter_map(|s| s.split(';').next())
        .filter_map(|s| s.split_once('='))
        .filter(|(name, value)| *name == SESSION_COOKIE && token_is_valid(value))
        .map(|(_, value)| value.to_owned());
    let token = tokens
        .next()
        .ok_or("Tonehavn did not return a valid session cookie.")?;
    if tokens.next().is_some() {
        return Err("Tonehavn returned ambiguous session cookies.".into());
    }
    Ok(token)
}

fn status_value(server: String, saved: bool, verified: Option<Envelope>, message: &str) -> Status {
    Status {
        server,
        saved_session: saved,
        signed_in: verified.is_some(),
        expires_at: verified.map(|e| e.expires_at),
        message: message.into(),
    }
}

pub(crate) fn status(root: &Path) -> Result<Status, String> {
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let config = read_config(root)?;
    if config.server.is_empty() {
        return Ok(status_value(config.server, false, None, "Not signed in."));
    }
    let server = origin(&config.server)?;
    let Some(mut saved) = read_credential(&server)? else {
        return Ok(status_value(server, false, None, "Not signed in."));
    };
    let response = match request(
        &client()?,
        Method::GET,
        &server,
        "/api/v1/auth/session",
        Some(&saved),
    )
    .send()
    {
        Ok(response) => response,
        Err(_) => {
            return Ok(status_value(
                server,
                true,
                None,
                "Could not reach Tonehavn securely. Your saved session is retained; check Tailscale and retry.",
            ));
        }
    };
    if response.status() == StatusCode::UNAUTHORIZED {
        clear_credential(&server)?;
        return Ok(status_value(
            server,
            false,
            None,
            "Your session expired or was revoked. Sign in again.",
        ));
    }
    let current = match envelope(response) {
        Ok(current) => current,
        Err(message) => return Ok(status_value(server, true, None, &message)),
    };
    saved.csrf = current.csrf_token.clone();
    save_credential(&saved)?;
    Ok(status_value(
        server,
        true,
        Some(current),
        "Connected as a trusted device.",
    ))
}

pub(crate) fn login(root: &Path, input: Login) -> Result<Status, String> {
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let mut config = read_config(root)?;
    if !config.server.is_empty() && read_credential(&origin(&config.server)?)?.is_some() {
        return Err("Sign out of your saved Tonehavn session before signing in again.".into());
    }
    let server = origin(&input.server)?;
    if input.username.trim().is_empty()
        || input.username.len() > 256
        || input.password.is_empty()
        || input.password.len() > 4096
        || input.totp_code.len() != 6
        || !input.totp_code.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("Enter your username, password, and six-digit authenticator code.".into());
    }
    config.server = server.clone();
    save_config(root, &config)?;
    let client = client()?;
    let response = request(&client, Method::POST, &server, "/api/v1/auth/login", None)
        .json(&serde_json::json!({"username":input.username.trim(), "password":input.password, "totpCode":input.totp_code,
            "mode":"trusted", "deviceId":config.device_id, "deviceLabel":format!("Aurora on {}", std::env::consts::OS)}))
        .send().map_err(|_| "Could not sign in securely. Check the server address and Tailscale connection.")?;
    if !response.status().is_success() {
        return Err(response_error(response.status()));
    }
    let token = session_token(response.headers())?;
    let current = envelope(response)?;
    let saved = Credential {
        server: server.clone(),
        token,
        csrf: current.csrf_token.clone(),
    };
    if let Err(error) = save_credential(&saved) {
        // Do not leave a usable remote session behind after a vault failure.
        let revoked = request(
            &client,
            Method::POST,
            &server,
            "/api/v1/auth/logout",
            Some(&saved),
        )
        .send()
        .is_ok_and(|r| r.status().is_success());
        return Err(if revoked {
            error
        } else {
            format!("{error} Revoke the Aurora session from Tonehavn’s session settings.")
        });
    }
    Ok(status_value(
        server,
        true,
        Some(current),
        "Connected as a trusted device.",
    ))
}

pub(crate) fn logout(root: &Path) -> Result<Status, String> {
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let config = read_config(root)?;
    let server = origin(&config.server)?;
    if let Some(mut saved) = read_credential(&server)? {
        let client = client()?;
        let current = request(
            &client,
            Method::GET,
            &server,
            "/api/v1/auth/session",
            Some(&saved),
        )
        .send()
        .map_err(
            |_| "Could not reach Tonehavn to revoke this session. Reconnect and retry sign-out.",
        )?;
        if current.status() != StatusCode::UNAUTHORIZED {
            saved.csrf = envelope(current)?.csrf_token;
            let response = request(
                &client,
                Method::POST,
                &server,
                "/api/v1/auth/logout",
                Some(&saved),
            )
            .send()
            .map_err(|_| "Could not revoke the session. Reconnect and retry sign-out.")?;
            if !response.status().is_success() && response.status() != StatusCode::UNAUTHORIZED {
                return Err(response_error(response.status()));
            }
        }
        clear_credential(&server)?;
    }
    Ok(status_value(
        server,
        false,
        None,
        "Signed out. The saved session was removed.",
    ))
}

pub(crate) fn edit_affinity(
    root: &Path,
    source_path: &str,
    expected: &crate::tag_model::TagValues,
    desired: &crate::tag_model::TagValues,
) -> Result<(), String> {
    use crate::tag_model::LoveState;
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let config = read_config(root)?;
    if config.server.is_empty() {
        return Err("Sign in to Tonehavn in Settings → Connections before editing.".into());
    }
    let server = origin(&config.server)?;
    let mut saved = read_credential(&server)?
        .ok_or("Sign in to Tonehavn in Settings → Connections before editing.")?;
    let client = client()?;
    let response = request(
        &client,
        Method::GET,
        &server,
        "/api/v1/auth/session",
        Some(&saved),
    )
    .send()
    .map_err(|_| "Tonehavn is unavailable. No edit was sent.")?;
    let current = envelope(response)?;
    saved.csrf = current.csrf_token;
    save_credential(&saved)?;
    let mut body = serde_json::json!({"sourcePath":source_path, "expected":{"rating":expected.rating.map(|r|r as u8),"loved":expected.love_state==LoveState::Loved}});
    if desired.rating != expected.rating {
        body["rating"] = serde_json::json!(desired.rating.map_or(0, |r| r as u8));
    }
    if desired.love_state != expected.love_state {
        body["loved"] = serde_json::json!(desired.love_state == LoveState::Loved);
    }
    if desired == expected {
        return Ok(());
    }
    let response = request(&client,Method::POST,&server,"/api/v1/aurora/affinity",Some(&saved)).timeout(Duration::from_secs(120)).json(&body).send()
        .map_err(|_| "No confirmed edit receipt arrived. The PC may have applied it; reload the track before trying again.")?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err("Tonehavn could not resolve this file. Install Tonehavn 0.38.4 or newer and ensure its catalog includes the exact MP3.".into());
    }
    if response.status() == StatusCode::CONFLICT {
        return Err(
            "The PC file or rating/Love values changed. If you just edited this song, wait for Tonehavn’s catalog refresh to finish; otherwise reload the track before trying again."
                .into(),
        );
    }
    if !response.status().is_success() {
        return Err(format!(
            "Tonehavn did not confirm the edit (HTTP {}). Reload the track before retrying.",
            response.status().as_u16()
        ));
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Receipt {
        track_id: uuid::Uuid,
        rating: Option<i64>,
        loved: bool,
    }
    let mut bytes = Vec::new();
    response
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "The PC edit response was interrupted. Reload the track before retrying.")?;
    if bytes.len() > 65_536 {
        return Err("The PC edit response was oversized. Reload the track before retrying.".into());
    }
    let receipt: Receipt = serde_json::from_slice(&bytes)
        .map_err(|_| "The PC edit receipt was invalid. Reload the track before retrying.")?;
    if receipt.track_id.is_nil()
        || receipt.rating != desired.rating.map(|r| (r * 20.0) as i64)
        || receipt.loved != (desired.love_state == LoveState::Loved)
    {
        return Err(
            "The PC receipt did not match the requested edit. Reload the track before retrying."
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origins_are_https_and_cannot_redirect_credentials_through_url_parts() {
        assert_eq!(
            origin("https://EXAMPLE.ts.net/").unwrap(),
            "https://example.ts.net"
        );
        for invalid in [
            "http://example.ts.net",
            "https://user:secret@example.ts.net",
            "https://example.ts.net/api",
            "https://example.ts.net/?key=x",
            "https://example.ts.net/#x",
        ] {
            assert!(origin(invalid).is_err());
        }
    }
    #[test]
    fn cookies_are_exact_and_header_safe() {
        let mut headers = header::HeaderMap::new();
        headers.append(
            header::SET_COOKIE,
            format!("other={}; Path=/", "a".repeat(64)).parse().unwrap(),
        );
        assert!(session_token(&headers).is_err());
        headers.append(
            header::SET_COOKIE,
            format!(
                "{SESSION_COOKIE}={}; Secure; HttpOnly; Path=/",
                "b".repeat(64)
            )
            .parse()
            .unwrap(),
        );
        assert_eq!(session_token(&headers).unwrap(), "b".repeat(64));
        headers.append(
            header::SET_COOKIE,
            format!("{SESSION_COOKIE}={}; Path=/", "c".repeat(64))
                .parse()
                .unwrap(),
        );
        assert!(session_token(&headers).is_err());
        assert!(!token_is_valid(&format!("{}; injected=x", "a".repeat(64))));
    }
    #[test]
    fn session_responses_reject_shared_expired_and_malformed_tokens() {
        let make = |mode: &str, expires: i64, csrf: &str| {
            serde_json::to_vec(&serde_json::json!({
                "mode": mode, "expiresAt": expires, "csrfToken": csrf
            }))
            .unwrap()
        };
        let csrf = "a".repeat(64);
        assert!(parse_envelope(&make("trusted", i64::MAX, &csrf)).is_ok());
        assert!(parse_envelope(&make("shared", i64::MAX, &csrf)).is_err());
        assert!(parse_envelope(&make("trusted", 1, &csrf)).is_err());
        assert!(parse_envelope(&make("trusted", i64::MAX, "bad;token")).is_err());
        assert!(parse_envelope(&vec![b' '; 65_537]).is_err());
    }

    #[test]
    fn authenticated_requests_carry_origin_cookie_and_csrf_without_url_secrets() {
        let server = "https://example.ts.net";
        let saved = Credential {
            server: server.into(),
            token: "a".repeat(64),
            csrf: "b".repeat(64),
        };
        let request = request(
            &client().unwrap(),
            Method::POST,
            server,
            "/api/v1/auth/logout",
            Some(&saved),
        )
        .build()
        .unwrap();
        assert_eq!(request.headers()[header::ORIGIN], server);
        assert_eq!(request.headers()["x-tonehavn-csrf"], saved.csrf);
        assert_eq!(
            request.headers()[header::COOKIE],
            format!(
                "{SESSION_COOKIE}={}; {CSRF_COOKIE}={}",
                saved.token, saved.csrf
            )
        );
        assert!(!request.url().as_str().contains(&saved.token));
        assert!(request.url().query().is_none());
    }

    #[test]
    fn frontend_status_never_serializes_credentials() {
        let status = status_value(
            "https://example.ts.net".into(),
            true,
            Some(Envelope {
                mode: "trusted".into(),
                expires_at: i64::MAX,
                csrf_token: "secret-csrf-value".into(),
            }),
            "Connected",
        );
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("secret-csrf-value"));
        assert!(!json.contains("token"));
    }
}
