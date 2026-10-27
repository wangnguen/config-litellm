//! The two calls ccl makes to a LiteLLM proxy.
//!
//! `GET /v1/models` answers which models a key may use and checks the key on
//! the way; it is the same call Claude Code makes for gateway model discovery.
//! `POST /v1/messages` with `max_tokens: 1` is the real probe, the one way to
//! learn that a listed model also answers.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::models::base_id;
use crate::{Error, Result};

/// A model list comes back well inside this; past it the gateway counts as down.
const LIST_TIMEOUT: Duration = Duration::from_secs(10);
/// A cold model can take a while over its first token.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
const ANTHROPIC_VERSION: &str = "2023-06-01";

fn agent(timeout: Duration) -> ureq::Agent {
    // Status codes come back as responses, not errors, so the gateway's own
    // reason ("key expired", "budget exceeded") reaches the person reading.
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim().trim_end_matches('/'))
}

/// Every model id the gateway lists for `key`, sorted.
pub fn list_models(base_url: &str, key: &str) -> Result<Vec<String>> {
    // LiteLLM reads the bearer token; Anthropic itself reads x-api-key. Both
    // go, so the same call works against either.
    let mut resp = agent(LIST_TIMEOUT)
        .get(&endpoint(base_url, "/v1/models"))
        .header("Authorization", &format!("Bearer {key}"))
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .call()
        .map_err(|e| Error::Unreachable(e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    parse_models(status, &body)
}

/// Ask `model` for a single token. `Ok` is how long the answer took.
pub fn probe(base_url: &str, key: &str, model: &str) -> Result<Duration> {
    let body = json!({
        "model": base_id(model),
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}],
    })
    .to_string();
    let started = Instant::now();
    let mut resp = agent(PROBE_TIMEOUT)
        .post(&endpoint(base_url, "/v1/messages"))
        .header("Authorization", &format!("Bearer {key}"))
        .header("x-api-key", key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| Error::Unreachable(e.to_string()))?;
    let elapsed = started.elapsed();
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(elapsed);
    }
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let json: Value = serde_json::from_str(&text).unwrap_or_default();
    Err(Error::Unexpected(match reason(&json) {
        Some(msg) => format!("HTTP {status}: {}", first_sentence(&msg)),
        None => format!("HTTP {status}"),
    }))
}

/// Turn a `/v1/models` response into a sorted id list or a typed error.
fn parse_models(status: u16, body: &str) -> Result<Vec<String>> {
    let json: Value = serde_json::from_str(body).unwrap_or_default();
    match status {
        200..=299 => {
            let items = json["data"].as_array().ok_or_else(|| {
                Error::Unexpected(
                    "the gateway answered, but not with a model list; is base_url right?".into(),
                )
            })?;
            let mut ids: Vec<String> = items
                .iter()
                .filter_map(|m| m["id"].as_str().map(str::to_string))
                .collect();
            ids.sort();
            ids.dedup();
            Ok(ids)
        }
        401 | 403 => Err(Error::Rejected(match reason(&json) {
            Some(msg) => format!("{} (HTTP {status})", first_sentence(&msg)),
            None => format!("HTTP {status}, wrong or expired key"),
        })),
        404 => Err(Error::Unexpected(
            "HTTP 404, no /v1/models here; is base_url right?".into(),
        )),
        _ => Err(Error::Unexpected(match reason(&json) {
            Some(msg) => format!("HTTP {status}: {}", first_sentence(&msg)),
            None => format!("HTTP {status}"),
        })),
    }
}

/// The gateway's explanation, wherever this LiteLLM version put it.
fn reason(json: &Value) -> Option<String> {
    json["error"]["message"]
        .as_str()
        .or_else(|| json["error"].as_str())
        .or_else(|| json["detail"].as_str())
        .map(str::to_string)
}

/// The first sentence of a gateway message. LiteLLM follows the reason with
/// the key's hash and table names, which help nobody reading a terminal.
fn first_sentence(message: &str) -> &str {
    let message = message.trim();
    match message.find(". ") {
        Some(end) => &message[..end],
        None => message.trim_end_matches('.'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_list_comes_back_sorted_and_deduped() {
        let body = r#"{"data": [{"id": "b"}, {"id": "a"}, {"id": "b"}, {"object": "x"}]}"#;
        assert_eq!(parse_models(200, body).unwrap(), ["a", "b"]);
    }

    #[test]
    fn rejected_key_keeps_only_the_gateway_reason() {
        let body = r#"{"error": {"message": "Authentication Error. Key hash abc123, table X."}}"#;
        match parse_models(401, body) {
            Err(Error::Rejected(msg)) => assert_eq!(msg, "Authentication Error (HTTP 401)"),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn a_success_without_a_list_points_at_the_url() {
        assert!(matches!(
            parse_models(200, "<html>login</html>"),
            Err(Error::Unexpected(_))
        ));
    }

    #[test]
    fn trailing_slashes_do_not_double_up() {
        assert_eq!(
            endpoint("https://gw.example/ ", "/v1/models"),
            "https://gw.example/v1/models"
        );
    }
}
