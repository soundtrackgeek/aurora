//! Trusted Tonehavn authentication. Secrets stay in the native credential store.
use keyring::Entry;
use reqwest::{Method, StatusCode, Url, blocking::Client, header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, sync::Mutex, time::Duration};

static AUTH_LOCK: Mutex<()> = Mutex::new(());
const SESSION_COOKIE: &str = "__Host-tonehavn_session";
const CSRF_COOKIE: &str = "__Host-tonehavn_csrf";
const EMAIL_LINK_LIFETIME_MS: i64 = 15 * 60 * 1_000;
const EMAIL_SIGN_IN_REQUIRED: &str =
    "Sign in to Tonehavn with an email link in Settings → Connections before editing.";

#[derive(Serialize, Deserialize)]
struct PendingEmailLogin {
    server: String,
    verifier: String,
    requested_at: i64,
}

impl PendingEmailLogin {
    fn expires_at(&self) -> i64 {
        self.requested_at.saturating_add(EMAIL_LINK_LIFETIME_MS)
    }

    fn is_current(&self, now: i64) -> bool {
        now >= self.requested_at && now < self.expires_at()
    }

    fn token_from_link(&self, link: &str, now: i64) -> Result<String, String> {
        let invalid = "Paste the complete email link requested in Aurora on this computer. If it expired, request a new link.";
        let url = Url::parse(link.trim()).map_err(|_| invalid)?;
        if !self.is_current(now)
            || url.origin().ascii_serialization() != self.server
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/auth/email"
            || url.query().is_some()
        {
            return Err(invalid.into());
        }
        let token = url
            .fragment()
            .and_then(|fragment| fragment.strip_prefix("token="))
            .filter(|token| is_email_token(token))
            .ok_or(invalid)?;
        Ok(token.to_owned())
    }
}

fn is_email_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

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
    pending_email_expires_at: Option<i64>,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmailLogin {
    server: String,
    email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EmailComplete {
    link: String,
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
    vault_entry(server, "session")
}

fn vault_entry(server: &str, kind: &str) -> Result<Entry, String> {
    let key = format!("{kind}-{:x}", Sha256::digest(server.as_bytes()));
    Entry::new("com.soundtrackgeek.aurora.tonehavn", &key)
        .map_err(|_| "Could not open the system credential store.")
        .map_err(String::from)
}

fn read_pending(server: &str) -> Result<Option<PendingEmailLogin>, String> {
    match vault_entry(server, "email-request")?.get_password() {
        Ok(value) => {
            let pending: PendingEmailLogin = serde_json::from_str(&value)
                .map_err(|_| "The pending Tonehavn sign-in is invalid. Start again.")?;
            if pending.server != server || !is_email_token(&pending.verifier) {
                return Err("The pending Tonehavn sign-in is invalid. Start again.".into());
            }
            if !pending.is_current(crate::state_sync::now_ms()) {
                clear_pending(server)?;
                return Ok(None);
            }
            Ok(Some(pending))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => {
            Err("Could not read the pending Tonehavn sign-in from the credential store.".into())
        }
    }
}

fn save_pending(pending: &PendingEmailLogin) -> Result<(), String> {
    vault_entry(&pending.server, "email-request")?
        .set_password(&serde_json::to_string(pending).map_err(|_| "Could not encode sign-in.")?)
        .map_err(|_| "Could not save the pending Tonehavn sign-in securely.".into())
}

fn clear_pending(server: &str) -> Result<(), String> {
    match vault_entry(server, "email-request")?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Could not remove the pending Tonehavn sign-in.".into()),
    }
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
        StatusCode::UNAUTHORIZED => "Your Tonehavn session expired or was revoked. Sign in again with an email link in Settings → Connections.",
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
        pending_email_expires_at: None,
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
        if let Some(pending) = read_pending(&server)? {
            return Ok(pending_status(&pending));
        }
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

fn pending_status(pending: &PendingEmailLogin) -> Status {
    let mut status = status_value(
        pending.server.clone(),
        false,
        None,
        "Check your email. If the address is authorized, a sign-in link should arrive shortly. Paste the latest link requested in Aurora on this computer within 15 minutes.",
    );
    status.pending_email_expires_at = Some(pending.expires_at());
    status
}

pub(crate) fn request_email_login(root: &Path, input: EmailLogin) -> Result<Status, String> {
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let mut config = read_config(root)?;
    if !config.server.is_empty() && read_credential(&origin(&config.server)?)?.is_some() {
        return Err("Sign out of your saved Tonehavn session before signing in again.".into());
    }
    let server = origin(&input.server)?;
    let email = input.email.trim();
    if email.len() > 254
        || !email.is_ascii()
        || !email.contains('@')
        || email
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err("Enter your Tonehavn email address.".into());
    }
    config.server = server.clone();
    save_config(root, &config)?;
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "Could not create a secure sign-in request.")?;
    let pending = PendingEmailLogin {
        server: server.clone(),
        verifier: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        requested_at: crate::state_sync::now_ms(),
    };
    // Persist before delivery so a restart or an uncertain network result can still redeem the link.
    save_pending(&pending)?;
    let client = client()?;
    let response = email_request(&client, &pending, email)
        .send().map_err(|_| "Could not confirm email delivery. Check your inbox, server address and Tailscale connection before requesting another link.")?;
    if !response.status().is_success() {
        clear_pending(&server)?;
        return Err(email_response_error(response.status()));
    }
    Ok(pending_status(&pending))
}

fn email_request(
    client: &Client,
    pending: &PendingEmailLogin,
    email: &str,
) -> reqwest::blocking::RequestBuilder {
    request(
        client,
        Method::POST,
        &pending.server,
        "/api/v1/auth/email/request",
        None,
    )
    .timeout(Duration::from_secs(30))
    .json(&serde_json::json!({"email":email,"verifier":pending.verifier}))
}

fn email_completion(
    client: &Client,
    pending: &PendingEmailLogin,
    token: &str,
) -> reqwest::blocking::RequestBuilder {
    request(
        client,
        Method::POST,
        &pending.server,
        "/api/v1/auth/email/complete",
        None,
    )
    .json(
        &serde_json::json!({"token":token,"verifier":pending.verifier,
            "deviceLabel":format!("Aurora on {}", std::env::consts::OS)}),
    )
}

fn email_response_error(code: StatusCode) -> String {
    match code {
        StatusCode::UNAUTHORIZED => "This email link expired, was already used, or was requested on another device. Request a new link in Aurora on this computer.",
        StatusCode::NOT_FOUND => "Email sign-in requires Tonehavn 0.41.0 or newer on the PC.",
        StatusCode::SERVICE_UNAVAILABLE => "Tonehavn could not send the email. Check email sign-in and SMTP settings on the PC, then try again.",
        _ => return response_error(code),
    }.into()
}

pub(crate) fn complete_email_login(root: &Path, input: EmailComplete) -> Result<Status, String> {
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let config = read_config(root)?;
    let server = origin(&config.server)?;
    if read_credential(&server)?.is_some() {
        return Err("Sign out of your saved Tonehavn session before signing in again.".into());
    }
    let pending = read_pending(&server)?
        .ok_or("Request a new email sign-in link in Aurora on this computer first.")?;
    let token = pending.token_from_link(&input.link, crate::state_sync::now_ms())?;
    let client = client()?;
    let response = email_completion(&client, &pending, &token).send().map_err(
        |_| "Could not confirm sign-in. Retry the link; if it was already used, request a new one.",
    )?;
    if !response.status().is_success() {
        if response.status() == StatusCode::UNAUTHORIZED {
            clear_pending(&server)?;
        }
        return Err(email_response_error(response.status()));
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
    clear_pending(&server)?;
    Ok(status_value(
        server,
        true,
        Some(current),
        "Connected as a trusted device.",
    ))
}

pub(crate) fn cancel_email_login(root: &Path) -> Result<Status, String> {
    let _guard = AUTH_LOCK
        .lock()
        .map_err(|_| "Tonehavn authentication is unavailable.")?;
    let config = read_config(root)?;
    let server = origin(&config.server)?;
    clear_pending(&server)?;
    Ok(status_value(
        server,
        false,
        None,
        "Request a new sign-in link.",
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
    clear_pending(&server)?;
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
        return Err(EMAIL_SIGN_IN_REQUIRED.into());
    }
    let server = origin(&config.server)?;
    let mut saved = read_credential(&server)?.ok_or(EMAIL_SIGN_IN_REQUIRED)?;
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
    if response.status() == StatusCode::UNAUTHORIZED {
        clear_credential(&server)?;
        return Err(format!(
            "{} No rating or Love edit was sent.",
            response_error(response.status())
        ));
    }
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
    if response.status() == StatusCode::UNAUTHORIZED {
        clear_credential(&server)?;
        return Err(response_error(response.status()));
    }
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

    fn pending() -> PendingEmailLogin {
        PendingEmailLogin {
            server: "https://example.ts.net".into(),
            verifier: "b".repeat(64),
            requested_at: 1_000,
        }
    }

    #[test]
    fn email_links_require_exact_origin_path_fragment_and_current_request() {
        let pending = pending();
        let token = "a".repeat(64);
        let link = format!("{}/auth/email#token={token}", pending.server);
        assert_eq!(pending.token_from_link(&link, 1_000).unwrap(), token);
        assert_eq!(
            pending
                .token_from_link(&format!("  {link}\n"), 1_001)
                .unwrap(),
            token
        );
        for invalid in [
            format!("https://evil.ts.net/auth/email#token={token}"),
            format!("http://example.ts.net/auth/email#token={token}"),
            format!("https://user:secret@example.ts.net/auth/email#token={token}"),
            format!("https://example.ts.net:444/auth/email#token={token}"),
            format!("https://example.ts.net/other#token={token}"),
            format!("https://example.ts.net/auth/email?token={token}"),
            format!("https://example.ts.net/auth/email?x=y#token={token}"),
            format!("{link}&token={token}"),
            format!("{link}&other=value"),
            "https://example.ts.net/auth/email#token=short".into(),
            format!("https://example.ts.net/auth/email#token={}", "G".repeat(64)),
            token.clone(),
        ] {
            assert!(pending.token_from_link(&invalid, 1_000).is_err());
        }
        assert!(pending.token_from_link(&link, 999).is_err());
        assert!(
            pending
                .token_from_link(&link, pending.expires_at())
                .is_err()
        );
        assert!(
            pending
                .token_from_link(&link, pending.expires_at() - 1)
                .is_ok()
        );
    }

    #[test]
    fn email_requests_match_tonehavn_contract_and_keep_secrets_out_of_urls() {
        let pending = pending();
        let client = client().unwrap();
        let token = "c".repeat(64);
        for (request, path, expected_body) in [
            (
                email_request(&client, &pending, "owner@example.com"),
                "/api/v1/auth/email/request",
                serde_json::json!({"email":"owner@example.com","verifier":pending.verifier}),
            ),
            (
                email_completion(&client, &pending, &token),
                "/api/v1/auth/email/complete",
                serde_json::json!({"token":token,"verifier":pending.verifier,
                    "deviceLabel":format!("Aurora on {}", std::env::consts::OS)}),
            ),
        ] {
            let request = request.build().unwrap();
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.url().as_str(), format!("{}{path}", pending.server));
            assert_eq!(request.headers()[header::ORIGIN], pending.server);
            assert!(!request.headers().contains_key(header::COOKIE));
            let body: serde_json::Value =
                serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
            assert_eq!(body, expected_body);
        }
    }

    #[test]
    fn pending_requests_survive_serialization_without_exposing_verifier_to_frontend_or_config() {
        let pending = pending();
        let restored: PendingEmailLogin =
            serde_json::from_slice(&serde_json::to_vec(&pending).unwrap()).unwrap();
        assert_eq!(restored.verifier, pending.verifier);
        assert_eq!(restored.server, pending.server);
        assert_eq!(restored.expires_at(), pending.expires_at());
        let status = serde_json::to_value(pending_status(&restored)).unwrap();
        assert_eq!(status["pendingEmailExpiresAt"], pending.expires_at());
        assert_eq!(status["signedIn"], false);
        assert_eq!(status["savedSession"], false);
        assert!(!status.to_string().contains(&pending.verifier));
        let config = serde_json::to_string(&Config::default()).unwrap();
        for secret in ["verifier", "email", "token", "password"] {
            assert!(!config.contains(secret));
        }
    }

    #[test]
    fn email_errors_offer_recovery_without_legacy_credentials() {
        assert!(email_response_error(StatusCode::UNAUTHORIZED).contains("Request a new link"));
        assert!(email_response_error(StatusCode::NOT_FOUND).contains("0.41.0"));
        assert!(email_response_error(StatusCode::SERVICE_UNAVAILABLE).contains("SMTP"));
        for code in [
            StatusCode::UNAUTHORIZED,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::FORBIDDEN,
        ] {
            let error = email_response_error(code);
            assert!(!error.contains("authenticator"));
            assert!(!error.contains("password"));
        }
    }
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
