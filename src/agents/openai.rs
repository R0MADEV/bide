use super::protocol::{build_prompt, response_from};
use super::{AgentRequest, AgentResponse, AgentRunner};
use crate::tools::Progress;
use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

/// An AgentRunner backed by the OpenAI chat completions API. It streams the
/// reply so a long step shows the reasoning appearing line by line.
pub struct OpenAiAgent {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    progress: Progress,
}

impl OpenAiAgent {
    pub fn new(api_key: String, model: String, max_tokens: u32, progress: Progress) -> Self {
        OpenAiAgent {
            client: Client::new(),
            api_key,
            model,
            max_tokens,
            progress,
        }
    }

    fn call(&self, prompt: &str) -> (bool, String) {
        let body = build_request_body(&self.model, prompt, self.max_tokens);
        let response = match self.client.post(ENDPOINT).bearer_auth(&self.api_key).json(&body).send() {
            Ok(response) => response,
            Err(error) => return (false, format!("request failed: {error}")),
        };
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return (false, format!("HTTP {status}: {}", body.trim()));
        }
        (true, stream_reply(response, &*self.progress))
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
        "stream": true,
        "messages": [{ "role": "user", "content": prompt }],
    })
}

fn stream_reply(response: Response, on_line: &dyn Fn(&str)) -> String {
    drain_stream(BufReader::new(response).lines().map_while(Result::ok), on_line)
}

/// Consume the SSE `data:` lines, forwarding each completed line of the reply to
/// `on_line` as it arrives, and returning the full text. OpenAI streams sub-line
/// token fragments, so they are buffered and emitted a whole line at a time.
fn drain_stream(lines: impl Iterator<Item = String>, on_line: &dyn Fn(&str)) -> String {
    let mut full = String::new();
    let mut pending = String::new();
    for line in lines {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Some(delta) = delta_content(data) else {
            continue;
        };
        full.push_str(&delta);
        pending.push_str(&delta);
        while let Some(index) = pending.find('\n') {
            let done: String = pending.drain(..=index).collect();
            on_line(done.trim_end());
        }
    }
    if !pending.trim().is_empty() {
        on_line(pending.trim_end());
    }
    full
}

fn delta_content(data: &str) -> Option<String> {
    let value: Value = serde_json::from_str(data).ok()?;
    let content = value
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()?;
    Some(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_carries_the_model_prompt_token_cap_and_streams() {
        let body = build_request_body("gpt-4o", "hello", 4096);
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["content"], "hello");
    }

    #[test]
    fn extracts_a_delta_fragment() {
        let data = r#"{"choices":[{"delta":{"content":" there"}}]}"#;
        assert_eq!(delta_content(data).as_deref(), Some(" there"));
    }

    #[test]
    fn a_role_only_or_final_delta_has_no_content() {
        assert!(delta_content(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#).is_none());
        assert!(delta_content(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#).is_none());
    }

    #[test]
    fn token_fragments_are_buffered_into_whole_lines() {
        // "Line one\nLine two" split across token deltas, then [DONE].
        let stream = [
            r#"data: {"choices":[{"delta":{"content":"Line "}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"one\nLine "}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"two"}}]}"#,
            "data: [DONE]",
        ];
        let lines = std::cell::RefCell::new(Vec::<String>::new());
        let full = drain_stream(stream.iter().map(|s| s.to_string()), &|line| {
            lines.borrow_mut().push(line.to_string())
        });
        assert_eq!(*lines.borrow(), vec!["Line one", "Line two"]);
        assert_eq!(full, "Line one\nLine two");
    }
}
