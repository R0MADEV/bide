mod agent_step;
mod anthropic;
mod claude_code;
mod openai;
mod protocol;

pub use agent_step::AgentStep;
pub use anthropic::AnthropicAgent;
pub use claude_code::{AgentProcess, AgentProcessResult, ClaudeCodeAgent};
pub use openai::OpenAiAgent;
pub use protocol::build_prompt;

/// A request for an agent to reason about something. Agents analyse, plan or
/// review; they never act on the system.
pub struct AgentRequest {
    pub role: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Proceed,
    Reject(String),
    Failed(String),
}

pub struct AgentResponse {
    pub output: String,
    pub verdict: Verdict,
}

/// Runs an agent and returns its structured response. The port that isolates the
/// LLM/Claude Code integration from the engine.
pub trait AgentRunner {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse;
}
