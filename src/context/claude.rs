use super::CodeContext;
use crate::exec;
use serde_json::Value;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(300);

/// Read-only tools the retrieval agent may use headlessly: the lexis search
/// tools plus the built-in file readers. No edit tools, so it can only look.
const ALLOWED_TOOLS: &str = "mcp__lexis__search_code,mcp__lexis__get_symbol,\
    mcp__lexis__read_file,mcp__lexis__list_symbols,mcp__lexis__outline,\
    mcp__lexis__get_context,mcp__lexis__find_references,mcp__lexis__list_entrypoints,\
    Read,Grep,Glob";

/// Gathers repository context by running Claude Code headlessly and letting it
/// use the lexis MCP tools to find and return the code relevant to the task.
/// This is the retrieval agent: it reports real code, it does not edit.
pub struct ClaudeContext {
    program: String,
    progress: crate::tools::Progress,
}

impl ClaudeContext {
    pub fn new(program: &str, progress: crate::tools::Progress) -> Self {
        ClaudeContext {
            program: program.to_string(),
            progress,
        }
    }
}

impl CodeContext for ClaudeContext {
    fn lookup(&mut self, task: &str) -> String {
        let progress = &self.progress;
        ask_claude_streaming(&self.program, &retrieval_prompt(task), |line| progress(line))
    }
}

fn claude_command(program: &str, prompt: &str, streaming: bool) -> Command {
    let mut command = Command::new(program);
    command
        .arg("-p")
        .arg(prompt)
        .arg("--allowedTools")
        .arg(ALLOWED_TOOLS);
    if streaming {
        command
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose");
    }
    command
}

/// Runs Claude Code headlessly with the lexis read tools allowed and returns its
/// reply. Shared by context retrieval and the interactive router.
pub fn ask_claude(program: &str, prompt: &str) -> String {
    let captured = exec::run(claude_command(program, prompt, false), TIMEOUT);
    if !captured.success {
        return String::new();
    }
    captured.stdout.trim().to_string()
}

/// Like `ask_claude`, but streams live progress: each tool the agent uses (a
/// file read, a search) is reported to `on_event` as it happens, so a long call
/// shows what it is doing. Returns the final answer text.
pub fn ask_claude_streaming(program: &str, prompt: &str, on_event: impl FnMut(&str)) -> String {
    let command = claude_command(program, prompt, true);
    let (success, output) = stream_claude(command, TIMEOUT, on_event);
    if success {
        output
    } else {
        String::new()
    }
}

/// Run a prepared Claude Code command in stream-json mode, reporting each tool it
/// uses to `on_event` and returning (success, final text). The final text is the
/// result event, or the raw output if no result event arrived. Shared by the
/// router/question path and the implement step.
pub fn stream_claude(
    command: Command,
    timeout: Duration,
    mut on_event: impl FnMut(&str),
) -> (bool, String) {
    let mut answer: Option<String> = None;
    let captured = exec::run_streaming(command, timeout, |line| match parse_stream_line(line) {
        Some(StreamEvent::Progress(text)) => on_event(&text),
        Some(StreamEvent::Result(text)) => answer = Some(text),
        None => {}
    });
    let text = answer.unwrap_or_else(|| captured.stdout.trim().to_string());
    (captured.success, text)
}

/// Build a Claude Code command in stream-json mode with the given extra args
/// (e.g. a permission mode), so callers get live tool events.
pub fn streaming_command(program: &str, prompt: &str, extra: &[&str]) -> Command {
    let mut command = claude_command(program, prompt, true);
    command.args(extra);
    command
}

/// A meaningful event pulled from Claude Code's stream-json output.
enum StreamEvent {
    /// A live progress line (a tool the agent just used).
    Progress(String),
    /// The final answer text.
    Result(String),
}

fn parse_stream_line(line: &str) -> Option<StreamEvent> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("type")?.as_str()? {
        "result" => Some(StreamEvent::Result(value.get("result")?.as_str()?.to_string())),
        "assistant" => tool_uses(&value).map(StreamEvent::Progress),
        _ => None,
    }
}

/// Collect the tool calls in an assistant message into one progress line, e.g.
/// "→ Read src/main.rs". Returns None when the message carries no tool use.
fn tool_uses(value: &Value) -> Option<String> {
    let content = value.get("message")?.get("content")?.as_array()?;
    let lines: Vec<String> = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(describe_tool)
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n"))
}

fn describe_tool(block: &Value) -> String {
    let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
    let input = block.get("input");
    let detail = input
        .and_then(|input| {
            input
                .get("file_path")
                .or_else(|| input.get("pattern"))
                .or_else(|| input.get("query"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    if detail.is_empty() {
        return format!("→ {name}");
    }
    format!("→ {name} {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_use_becomes_a_progress_line() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/main.rs","limit":1}}]}}"#;
        match parse_stream_line(line) {
            Some(StreamEvent::Progress(text)) => assert_eq!(text, "→ Read src/main.rs"),
            _ => panic!("expected a progress line"),
        }
    }

    #[test]
    fn the_result_event_carries_the_final_answer() {
        let line = r#"{"type":"result","subtype":"success","result":"the answer"}"#;
        match parse_stream_line(line) {
            Some(StreamEvent::Result(text)) => assert_eq!(text, "the answer"),
            _ => panic!("expected the result text"),
        }
    }

    #[test]
    fn noise_events_and_plain_text_are_ignored() {
        // System/init events and assistant messages without a tool use carry no
        // progress line.
        assert!(parse_stream_line(r#"{"type":"system","subtype":"init"}"#).is_none());
        assert!(parse_stream_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#
        )
        .is_none());
        assert!(parse_stream_line("not json").is_none());
    }
}

pub fn retrieval_prompt(task: &str) -> String {
    format!(
        "Use the lexis code-search tools (search_code, get_symbol, read_file) to \
         find the code relevant to the task below. Output the relevant file paths \
         and the actual code (the functions and types involved), concise but \
         complete. Do NOT edit any files and do NOT implement the task — only \
         report the relevant existing code so another agent can plan.\n\nTask: {task}"
    )
}
