use super::protocol::{build_prompt, response_from};
use super::{AgentRequest, AgentResponse, AgentRunner};
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
        response_from(result.success, result.stdout)
    }
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
