use super::CodeContext;
use crate::exec;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(120);

/// Repository context from Lexis: runs `lexis ask` for the task and returns its
/// synthesized answer. Only reached when the workflow opts into Lexis.
pub struct LexisAsk {
    path: PathBuf,
}

impl LexisAsk {
    pub fn new(path: PathBuf) -> Self {
        LexisAsk { path }
    }
}

impl CodeContext for LexisAsk {
    fn lookup(&mut self, task: &str) -> String {
        let mut command = Command::new("lexis");
        command.arg("ask").arg(task).arg("-p").arg(&self.path);
        let captured = exec::run(command, TIMEOUT);
        if !captured.success {
            return String::new();
        }
        captured.stdout.trim().to_string()
    }
}
