use super::http::send_and_extract;
use super::protocol::{build_prompt, response_from};
use super::{AgentRequest, AgentResponse, AgentRunner};
use reqwest::blocking::Client;
use serde_json::{json, Value};

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 1024;

/// An AgentRunner backed by the Anthropic messages API.
pub struct AnthropicAgent {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicAgent {
    pub fn new(api_key: String, model: String) -> Self {
        AnthropicAgent {
            client: Client::new(),
            api_key,
            model,
        }
    }

    fn call(&self, prompt: &str) -> (bool, String) {
        let request = self
            .client
            .post(ENDPOINT)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", VERSION)
            .json(&build_request_body(&self.model, prompt));
        send_and_extract(request, extract_content)
    }
}

impl AgentRunner for AnthropicAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        let prompt = build_prompt(&request.role, &request.input);
        let (success, output) = self.call(&prompt);
        response_from(success, output)
    }
}

fn build_request_body(model: &str, prompt: &str) -> Value {
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "messages": [{ "role": "user", "content": prompt }],
    })
}

fn extract_content(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let content = value.get("content")?.get(0)?.get("text")?.as_str()?;
    Some(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_carries_model_max_tokens_and_prompt() {
        let body = build_request_body("claude-sonnet-4-6", "hello");
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["max_tokens"], MAX_TOKENS);
        assert_eq!(body["messages"][0]["content"], "hello");
    }

    #[test]
    fn extracts_the_message_text() {
        let body = r#"{"content":[{"type":"text","text":"VERDICT: PROCEED"}]}"#;
        assert_eq!(extract_content(body).as_deref(), Some("VERDICT: PROCEED"));
    }

    #[test]
    fn missing_text_is_none() {
        assert!(extract_content(r#"{"content":[]}"#).is_none());
    }
}
