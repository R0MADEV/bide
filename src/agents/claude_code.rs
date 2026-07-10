use super::protocol::{build_prompt, response_from};
use super::{AgentRequest, AgentResponse, AgentRunner};
use crate::exec;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(300);

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

    pub fn with_cli(program: &str) -> Self {
        ClaudeCodeAgent::new(Box::new(ClaudeCli {
            program: program.to_string(),
        }))
    }
}

impl AgentRunner for ClaudeCodeAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        let prompt = build_prompt(&request.role, &request.input);
        let result = self.process.run(&prompt);
        response_from(result.success, result.stdout)
    }
}

/// Real process: invokes the `claude` binary in print mode and captures stdout.
struct ClaudeCli {
    program: String,
}

impl AgentProcess for ClaudeCli {
    fn run(&mut self, prompt: &str) -> AgentProcessResult {
        let mut command = Command::new(&self.program);
        command.arg("--print").arg(prompt);
        let captured = exec::run(command, TIMEOUT);
        // On success the response is on stdout; on failure surface stderr/timeout
        // so the report shows why the agent call failed.
        let stdout = if captured.success {
            captured.stdout
        } else {
            captured.merged()
        };
        AgentProcessResult {
            success: captured.success,
            stdout,
        }
    }
}
