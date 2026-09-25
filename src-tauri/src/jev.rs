//! Bounded judgments through OpenRouter; no catalog writes or generated reasons.
use keyring::Entry;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

pub(crate) const MODEL: &str = "typesafe/jev-1.13";
const ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    configured: bool,
    source: &'static str,
    model: &'static str,
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum CredentialsRequest {
    Save {
        #[serde(rename = "apiKey")]
        api_key: String,
    },
    Clear,
}

fn entry() -> Result<Entry, String> {
    Entry::new("Aurora", "openrouter-api-key")
        .map_err(|_| "Could not open the credential vault.".into())
}

fn key() -> Result<Option<(String, &'static str)>, String> {
    match entry()?.get_password() {
        Ok(value) if !value.trim().is_empty() => return Ok(Some((value, "vault"))),
        Ok(_) | Err(keyring::Error::NoEntry) => {}
        Err(_) => {
            return Err("Could not read the OpenRouter key from the credential vault.".into());
        }
    }
    // Development only. Never bundle a key into Vite or a release binary.
    if cfg!(debug_assertions) {
        return Ok(std::env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| (value, "environment")));
    }
    Ok(None)
}

pub(crate) fn status() -> Result<Status, String> {
    let source = key()?.map_or("none", |(_, source)| source);
    Ok(Status {
        configured: source != "none",
        source,
        model: MODEL,
    })
}

pub(crate) fn save(request: CredentialsRequest) -> Result<Status, String> {
    match request {
        CredentialsRequest::Save { api_key } => {
            let value = api_key.trim();
            if value.len() < 20 || value.len() > 512 || value.chars().any(char::is_whitespace) {
                return Err("Enter a valid OpenRouter API key.".into());
            }
            entry()?
                .set_password(value)
                .map_err(|_| "Could not save the OpenRouter key securely.")?;
        }
        CredentialsRequest::Clear => match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("Could not remove the saved OpenRouter key.".into()),
        },
    }
    status()
}

pub(crate) fn request(body: Value) -> Result<Value, String> {
    let (key, _) = key()?.ok_or("Add an OpenRouter key in Settings → Connections to use Jev.")?;
    send(&key, body)
}

fn send(key: &str, body: Value) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not prepare the Jev connection.")?;
    let response = client
        .post(ENDPOINT)
        .bearer_auth(key)
        .header("X-Title", "Aurora Tonight's Album")
        .json(&body)
        .send()
        .map_err(|_| "Jev could not be reached within 25 seconds.")?;
    if !response.status().is_success() {
        // Provider bodies/headers may echo secrets or submitted metadata.
        return Err(format!(
            "OpenRouter returned HTTP {}. Check your key, credit balance and Jev availability.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(524_289)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read Jev's response.")?;
    if bytes.len() > 524_288 {
        return Err("Jev's response exceeded the size limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Jev returned an unreadable response.".into())
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Score {
    pub(crate) score: f64,
    pub(crate) confidence: f64,
}

pub(crate) fn parse_score(payload: &Value, id: &str) -> Result<Score, String> {
    let answer = &payload["answers"][id];
    let score = answer["score"]
        .as_f64()
        .ok_or("Jev omitted a requested score.")?;
    let confidence = answer["confidence"]
        .as_f64()
        .ok_or("Jev omitted score confidence.")?;
    let probabilities = &answer["probabilities"];
    let distribution: Option<Vec<f64>> = (0..4)
        .map(|i| probabilities[i.to_string()].as_f64())
        .collect();
    let valid_distribution = distribution.is_some_and(|values| {
        values
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && (values.iter().sum::<f64>() - 1.0).abs() <= 0.02
    });
    if answer["type"] != "score"
        || !score.is_finite()
        || !(0.0..=3.0).contains(&score)
        || !confidence.is_finite()
        || !(0.0..=1.0).contains(&confidence)
        || !valid_distribution
    {
        return Err("Jev returned an invalid typed score.".into());
    }
    Ok(Score { score, confidence })
}

pub(crate) fn test_connection() -> Result<String, String> {
    let payload = request(
        json!({ "model": MODEL, "state": "Quiet instrumental music with soft sustained notes.", "questions": {
            "test": { "type": "score", "instructions": "How well does the description support quiet music?", "criteria": ["Conflicts", "Insufficient evidence", "Some support", "Direct support"] }
        }}),
    )?;
    parse_score(&payload, "test")?;
    Ok("Jev returned a valid typed score through OpenRouter.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_invalid_and_wrong_type_answers() {
        let mut payload = json!({"answers":{"a":{"type":"score","score":2.5,"confidence":0.8,"probabilities":{"0":0.0,"1":0.0,"2":0.5,"3":0.5}}}});
        assert_eq!(parse_score(&payload, "a").unwrap().score, 2.5);
        assert!(parse_score(&payload, "b").is_err());
        for value in [json!(-1), json!(4), json!("2")] {
            payload["answers"]["a"]["score"] = value;
            assert!(parse_score(&payload, "a").is_err());
        }
        payload["answers"]["a"]["score"] = json!(2);
        payload["answers"]["a"]["probabilities"]["3"] = json!(0);
        assert!(parse_score(&payload, "a").is_err());
        payload["answers"]["a"]["type"] = json!("choice");
        assert!(parse_score(&payload, "a").is_err());
    }
}
