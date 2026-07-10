use super::http::send_and_extract;
use super::protocol::{build_prompt, response_from};
use super::{AgentRequest, AgentResponse, AgentRunner};
use reqwest::blocking::Client;
use serde_json::{json, Value};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

/// An AgentRunner backed by the OpenAI chat completions API.
pub struct OpenAiAgent {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl OpenAiAgent {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        OpenAiAgent {
            client: Client::new(),
            api_key,
            model,
            max_tokens,
        }
    }

    fn call(&self, prompt: &str) -> (bool, String) {
        let request = self
            .client
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&build_request_body(&self.model, prompt, self.max_tokens));
        send_and_extract(request, extract_content)
    }
}

impl AgentRunner for OpenAiAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        let prompt = build_prompt(&request.role, &request.input);
        let (success, output) = self.call(&prompt);
        response_from(success, output)
    }
}

fn build_request_body(model: &str, prompt: &str, max_tokens: u32) -> Value {
    json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": prompt }],
    })
}

fn extract_content(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let content = value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;
    Some(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_carries_the_model_prompt_and_token_cap() {
        let body = build_request_body("gpt-4o", "hello", 4096);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["messages"][0]["content"], "hello");
    }

    #[test]
    fn extracts_the_assistant_content() {
        let body = r#"{"choices":[{"message":{"content":"VERDICT: PROCEED"}}]}"#;
        assert_eq!(extract_content(body).as_deref(), Some("VERDICT: PROCEED"));
    }

    #[test]
    fn missing_content_is_none() {
        assert!(extract_content("{}").is_none());
    }
}
