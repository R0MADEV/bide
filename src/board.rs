use std::fmt::Write as _;

/// Shared state the steps read and write as the workflow runs. A step's output
/// becomes context for the steps that follow (plan → critic → fix).
#[derive(Default)]
pub struct Blackboard {
    entries: Vec<(String, String)>,
}

impl Blackboard {
    pub fn new() -> Self {
        Blackboard::default()
    }

    pub fn record(&mut self, step: &str, output: &str) {
        self.entries.push((step.to_string(), output.to_string()));
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn summary(&self) -> String {
        let mut out = String::new();
        for (name, output) in &self.entries {
            let _ = writeln!(out, "### {name}\n{}\n", output.trim());
        }
        out
    }
}
