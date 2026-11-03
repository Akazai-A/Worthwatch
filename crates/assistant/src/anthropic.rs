//! Anthropic's Messages API: the cloud opt-in. The key comes from the environment,
//! the prompt is shown to the user before it is sent, and the reply is read by the
//! same strict parser as every other provider.

use serde_json::{json, Value};

use crate::llm::{Llm, LlmError, Prompt};

pub const URL: &str = "https://api.anthropic.com/v1/messages";
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
pub const KEY_VAR: &str = "ANTHROPIC_API_KEY";

#[derive(Debug, Clone)]
pub struct Anthropic {
    pub model: String,
    pub api_key: String,
}

pub fn request_body(model: &str, p: &Prompt) -> Value {
    json!({
        "model": model,
        "max_tokens": 1024,
        "system": p.system,
        "messages": [{ "role": "user", "content": p.user }],
    })
}

/// The text blocks of the reply, joined.
pub fn parse_response(body: &str) -> Result<String, LlmError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| LlmError::Parse { provider: "anthropic", reason: e.to_string() })?;
    if let Some(msg) = v.pointer("/error/message").and_then(Value::as_str) {
        return Err(LlmError::Http(format!("anthropic: {msg}")));
    }
    let text: String = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks.iter().filter_map(|b| b.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("")
        })
        .unwrap_or_default();
    if text.is_empty() {
        return Err(LlmError::Parse { provider: "anthropic", reason: "no text in the reply".into() });
    }
    Ok(text)
}

impl Llm for Anthropic {
    fn name(&self) -> &'static str {
        "cloud"
    }
    fn complete(&self, prompt: &Prompt) -> Result<String, LlmError> {
        if self.api_key.is_empty() {
            return Err(LlmError::NoKey);
        }
        let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(60)).build();
        let req = agent
            .post(URL)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json");
        match req.send_json(request_body(&self.model, prompt)) {
            Ok(resp) => parse_response(&resp.into_string().map_err(|e| LlmError::Http(e.to_string()))?),
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                match parse_response(&body) {
                    Err(LlmError::Http(m)) => Err(LlmError::Http(m)),
                    _ => Err(LlmError::Http(format!("HTTP {code} from {URL}"))),
                }
            }
            Err(ureq::Error::Transport(t)) => Err(LlmError::Unreachable("anthropic", t.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_reply_parses_and_errors_surface() {
        let text = parse_response(include_str!("../fixtures/anthropic-messages.json")).unwrap();
        let p = crate::parse_reply(&text).unwrap();
        assert_eq!(p.query.as_deref(), Some("net worth"));
        let e = parse_response(
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        assert!(e.unwrap_err().to_string().contains("invalid x-api-key"));
        let b = request_body("m", &Prompt { system: "s".into(), user: "u".into() });
        assert_eq!(b["system"], "s");
        assert_eq!(b["messages"][0]["role"], "user");
    }

    #[test]
    fn no_key_means_no_request() {
        let a = Anthropic { model: "m".into(), api_key: String::new() };
        assert!(matches!(
            a.complete(&Prompt { system: String::new(), user: String::new() }),
            Err(LlmError::NoKey)
        ));
    }
}
