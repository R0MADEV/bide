use super::CodeContext;
use crate::exec;
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
}

impl ClaudeContext {
    pub fn new(program: &str) -> Self {
        ClaudeContext {
            program: program.to_string(),
        }
    }
}

impl CodeContext for ClaudeContext {
    fn lookup(&mut self, task: &str) -> String {
        let mut command = Command::new(&self.program);
        command
            .arg("-p")
            .arg(retrieval_prompt(task))
            .arg("--allowedTools")
            .arg(ALLOWED_TOOLS);
        let captured = exec::run(command, TIMEOUT);
        if !captured.success {
            return String::new();
        }
        captured.stdout.trim().to_string()
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
