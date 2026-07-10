mod claude;
mod lexis;

pub use claude::{ask_claude, retrieval_prompt, ClaudeContext};
pub use lexis::LexisAsk;

/// Provides repository context for a task. The port that isolates the Lexis
/// integration from the engine and agents.
pub trait CodeContext {
    fn lookup(&mut self, task: &str) -> String;
}

pub struct ContextPack {
    pub text: String,
}

pub fn build_context(provider: &mut dyn CodeContext, task: &str) -> ContextPack {
    let raw = provider.lookup(task);
    let text = if raw.trim().is_empty() {
        "No repository context available.".to_string()
    } else {
        raw
    };
    ContextPack { text }
}
