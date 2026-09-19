use crate::{
    artist_discovery::{cached_json, read_json, validate_artist},
    musicbrainz,
    state_store::StateStore,
};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CREDENTIAL: &str = "fanart.tv credentials";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Credentials {
    api_key: String,
    personal_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum CredentialsRequest {
    Save {
        #[serde(flatten)]
        credentials: Credentials,
    },
    Clear,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    configured: bool,
    personal_key_configured: bool,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtistArtwork {
    background_url: Option<String>,
    portrait_url: Option<String>,
    warning: Option<String>,
}

fn entry() -> Result<Entry, String> {
    Entry::new("Aurora", CREDENTIAL).map_err(|_| "Could not open the credential vault.".into())
}

fn credentials() -> Option<Credentials> {
    entry()
        .ok()
        .and_then(|e| e.get_password().ok())
        .and_then(|value| serde_json::from_str(&value).ok())
        .or_else(|| {
            if cfg!(debug_assertions) {
                std::env::var("FANART_TV")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
                    .map(|api_key| Credentials {
                        api_key,
                        personal_key: std::env::var("FANART_TV_PERSONAL")
                            .ok()
                            .filter(|v| !v.trim().is_empty()),
                    })
            } else {
                None
            }
        })
}

pub(crate) fn status() -> Status {
    let value = credentials();
    Status {
        configured: value.is_some(),
        personal_key_configured: value.is_some_and(|v| v.personal_key.is_some()),
    }
}

fn validate_key(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() != 32 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("fanart.tv keys must contain 32 hexadecimal characters.".into());
    }
    Ok(value.to_owned())
}

pub(crate) fn save(request: CredentialsRequest) -> Result<Status, String> {
    match request {
        CredentialsRequest::Save { credentials } => {
            let credentials = Credentials {
                api_key: validate_key(&credentials.api_key)?,
                personal_key: credentials
                    .personal_key
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| validate_key(&v))
                    .transpose()?,
            };
            let payload = serde_json::to_string(&credentials)
                .map_err(|_| "Could not prepare fanart.tv credentials.".to_owned())?;
            entry()?.set_password(&payload).map_err(|_| {
                "Could not save fanart.tv credentials in the credential vault.".to_owned()
            })?;
        }
        CredentialsRequest::Clear => match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("Could not remove fanart.tv credentials.".into()),
        },
    }
    Ok(status())
}

fn artwork_url(items: &Value) -> Option<String> {
    let mut images = items.as_array()?.iter().collect::<Vec<_>>();
    images.sort_by_key(|image| {
        std::cmp::Reverse(
            image["likes"]
                .as_str()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0),
        )
    });
    images.into_iter().find_map(|image| {
        let url = reqwest::Url::parse(image["url"].as_str()?).ok()?;
        (url.scheme() == "https"
            && url.host_str() == Some("assets.fanart.tv")
            && url.username().is_empty()
            && url.password().is_none())
        .then(|| url.into())
    })
}

pub(crate) fn load(
    artist: &str,
    refresh: bool,
    store: &StateStore,
) -> Result<ArtistArtwork, String> {
    let artist = validate_artist(artist)?;
    let intelligence = musicbrainz::load_artist_intelligence_with_store(artist.to_owned(), store)?;
    let Some(identity) = intelligence
        .identity
        .filter(|_| !matches!(intelligence.match_state, "ignored" | "conflict"))
    else {
        return Ok(ArtistArtwork { warning: Some("Connect this artist to an unambiguous MusicBrainz identity to use fanart.tv artwork.".into()), ..Default::default() });
    };
    let (payload, stale) =
        cached_json(store, &format!("fanart:{}", identity.mbid), refresh, || {
            let credentials = credentials().ok_or(
                "Add fanart.tv credentials in Settings → Metadata for artist backgrounds.",
            )?;
            let mut request = crate::lastfm::client()?
                .get(format!(
                    "https://webservice.fanart.tv/v3/music/{}",
                    identity.mbid
                ))
                .header("api-key", credentials.api_key);
            if let Some(personal) = credentials.personal_key {
                request = request.header("client-key", personal);
            }
            let payload = read_json(
                request
                    .send()
                    .map_err(|_| "fanart.tv is temporarily unavailable.".to_owned())?,
            )?;
            if payload["mbid_id"].as_str() != Some(&identity.mbid) {
                return Err("fanart.tv returned a different artist identity.".into());
            }
            Ok(payload)
        })?;
    Ok(ArtistArtwork {
        background_url: artwork_url(&payload["artistbackground"]),
        portrait_url: artwork_url(&payload["artistthumb"]),
        warning: stale.then(|| "Showing saved fanart.tv artwork; refresh is unavailable.".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn art_uses_liked_images_on_the_provider_host_only() {
        let images = json!([{"url":"https://evil.test/art.jpg","likes":"999"},{"url":"https://assets.fanart.tv/low.jpg","likes":"2"},{"url":"https://assets.fanart.tv/high.jpg","likes":"8"}]);
        assert_eq!(
            artwork_url(&images).as_deref(),
            Some("https://assets.fanart.tv/high.jpg")
        );
        assert!(artwork_url(&json!([{"url":"http://assets.fanart.tv/x"}])).is_none());
    }
    #[test]
    fn credentials_validate_without_echoing_secret_values() {
        assert!(validate_key("0123456789abcdef0123456789abcdef").is_ok());
        assert!(validate_key("secret").is_err());
        let request: CredentialsRequest = serde_json::from_value(
            json!({"mode":"save","apiKey":"0123456789abcdef0123456789abcdef","personalKey":null}),
        )
        .unwrap();
        assert!(matches!(request, CredentialsRequest::Save { .. }));
    }
}
