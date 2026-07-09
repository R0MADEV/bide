use super::{AgentRequest, AgentResponse, AgentRunner, Verdict};
use std::process::Command;

/// Runs the external agent for one prompt and returns its raw output. The port
/// that isolates spawning the `claude` CLI from the prompt/verdict logic.
pub trait AgentProcess {
    fn run(&mut self, prompt: &str) -> AgentProcessResult;
}

pub struct AgentProcessResult {
    pub success: bool,
    pub stdout: String,
}

/// An AgentRunner backed by Claude Code: it builds a role prompt, runs the
/// process, and parses the reply into a verdict.
pub struct ClaudeCodeAgent {
    process: Box<dyn AgentProcess>,
}

impl ClaudeCodeAgent {
    pub fn new(process: Box<dyn AgentProcess>) -> Self {
        ClaudeCodeAgent { process }
    }

    pub fn with_cli() -> Self {
        ClaudeCodeAgent::new(Box::new(ClaudeCli))
    }
}

impl AgentRunner for ClaudeCodeAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        let prompt = build_prompt(&request.role, &request.input);
        let result = self.process.run(&prompt);
        if !result.success {
            return AgentResponse {
                verdict: Verdict::Failed("agent process failed".to_string()),
                output: result.stdout,
            };
        }
        AgentResponse {
            verdict: parse_verdict(&result.stdout),
            output: result.stdout,
        }
    }
}

pub fn build_prompt(role: &str, input: &str) -> String {
    format!(
        "You are the {role} agent in the bide workflow. \
         Do the {role} job for the task below, then end your reply with exactly \
         one line: `VERDICT: PROCEED` if the workflow should continue, or \
         `VERDICT: REJECT: <reason>` if it should not.\n\nTask: {input}\n"
    )
}

fn parse_verdict(output: &str) -> Verdict {
    let marker = output.lines().map(str::trim).find(|line| line.starts_with("VERDICT:"));
    let Some(line) = marker else {
        return Verdict::Failed("agent did not emit a verdict".to_string());
    };

    let body = line["VERDICT:".len()..].trim();
    if body.eq_ignore_ascii_case("PROCEED") {
        return Verdict::Proceed;
    }
    if let Some(reason) = reject_reason(body) {
        return Verdict::Reject(reason);
    }
    Verdict::Failed(format!("unknown verdict: {body}"))
}

fn reject_reason(body: &str) -> Option<String> {
    let rest = body.strip_prefix("REJECT")?;
    Some(rest.trim_start_matches(':').trim().to_string())
}

/// Real process: invokes `claude` in print mode and captures stdout.
struct ClaudeCli;

impl AgentProcess for ClaudeCli {
    fn run(&mut self, prompt: &str) -> AgentProcessResult {
        let output = Command::new("claude").arg("--print").arg(prompt).output();
        let Ok(output) = output else {
            return AgentProcessResult {
                success: false,
                stdout: String::new(),
            };
        };
        AgentProcessResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        }
    }
}
