use super::CodeContext;
use std::path::PathBuf;
use std::process::Command;

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
        let output = Command::new("lexis")
            .arg("ask")
            .arg(task)
            .arg("-p")
            .arg(&self.path)
            .output();

        let Ok(output) = output else {
            return String::new();
        };
        if !output.status.success() {
            return String::new();
        }
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}
