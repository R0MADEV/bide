use crate::{Status, StepOutcome};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct StepRecord {
    pub name: String,
    pub outcome: StepOutcome,
    pub output: String,
}

/// The record of one run: the task, every step it took (retries included) and
/// the final status. This is what bide persists so a run stays inspectable.
pub struct RunRecord {
    pub task: String,
    pub steps: Vec<StepRecord>,
    pub status: Status,
}

pub fn render(record: &RunRecord) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# bide run report\n");
    let _ = writeln!(out, "Task: {}", record.task);
    let _ = writeln!(out, "Result: {:?}\n", record.status);
    let _ = writeln!(out, "## Steps\n");

    for (index, step) in record.steps.iter().enumerate() {
        let _ = writeln!(out, "{}. {} — {:?}", index + 1, step.name, step.outcome);
        if !step.output.trim().is_empty() {
            let _ = writeln!(out, "\n```\n{}\n```\n", step.output.trim());
        }
    }
    out
}

pub fn save(record: &RunRecord, runs_dir: &Path, id: &str) -> io::Result<PathBuf> {
    let dir = runs_dir.join(id);
    fs::create_dir_all(&dir)?;
    let path = dir.join("report.md");
    fs::write(&path, render(record))?;
    Ok(path)
}
