//! Ollama's `/api/chat`, JSON mode, no streaming. Pure request/response functions
//! plus a thin blocking HTTP shim.

use serde_json::{json, Value};

use crate::llm::{Llm, LlmError, Prompt};

pub const DEFAULT_URL: &str = "http://127.0.0.1:11434";
pub const DEFAULT_MODEL: &str = "llama3.1";

#[derive(Debug, Clone)]
pub struct Ollama {
    pub url: String,
    pub model: String,
}

impl Default for Ollama {
    fn default() -> Self {
        Self { url: DEFAULT_URL.into(), model: DEFAULT_MODEL.into() }
    }
}

pub fn request_body(model: &str, p: &Prompt) -> Value {
    json!({
        "model": model,
        "stream": false,
        "format": "json",
        "options": { "temperature": 0 },
        "messages": [
            { "role": "system", "content": p.system },
            { "role": "user", "content": p.user },
        ],
    })
}

/// The assistant message's text out of a chat response.
pub fn parse_response(body: &str) -> Result<String, LlmError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| LlmError::Parse { provider: "ollama", reason: e.to_string() })?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(LlmError::Http(format!("ollama: {err}")));
    }
    v.pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| LlmError::Parse { provider: "ollama", reason: "no message.content".into() })
}

impl Llm for Ollama {
    fn name(&self) -> &'static str {
        "ollama"
    }
    fn complete(&self, prompt: &Prompt) -> Result<String, LlmError> {
        let url = format!("{}/api/chat", self.url.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(120)).build();
        match agent.post(&url).send_json(request_body(&self.model, prompt)) {
            Ok(resp) => parse_response(&resp.into_string().map_err(|e| LlmError::Http(e.to_string()))?),
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                match parse_response(&body) {
                    Err(LlmError::Http(m)) => Err(LlmError::Http(m)),
                    _ => Err(LlmError::Http(format!("HTTP {code} from {url}"))),
                }
            }
            Err(ureq::Error::Transport(t)) => Err(LlmError::Unreachable("ollama", t.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_json_mode_without_streaming() {
        let b = request_body("llama3.1", &Prompt { system: "s".into(), user: "u".into() });
        assert_eq!(b["format"], "json");
        assert_eq!(b["stream"], false);
        assert_eq!(b["messages"][1]["content"], "u");
    }

    #[test]
    fn fixture_reply_parses_to_a_proposal() {
        let text = parse_response(include_str!("../fixtures/ollama-chat.json")).unwrap();
        let p = crate::parse_reply(&text).unwrap();
        assert_eq!(p.items.len(), 3);
        assert!(parse_response(r#"{"error":"model 'nope' not found"}"#)
            .unwrap_err()
            .to_string()
            .contains("not found"));
        assert!(parse_response("<html>").is_err());
    }
}
