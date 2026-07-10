use crate::{Status, StepOutcome};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct StepRecord {
    pub name: String,
    pub outcome: StepOutcome,
    pub output: String,
    pub prompt: String,
}

/// The record of one run: the task, every step it took (retries included) and
/// the final status. This is what bide persists so a run stays inspectable.
pub struct RunRecord {
    pub task: String,
    pub steps: Vec<StepRecord>,
    pub status: Status,
    pub diff: String,
}

pub fn render(record: &RunRecord) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# bide run report\n");
    let _ = writeln!(out, "Task: {}", record.task);
    let _ = writeln!(out, "Result: {:?}\n", record.status);
    let _ = writeln!(out, "## Steps\n");

    for (index, step) in record.steps.iter().enumerate() {
        let _ = writeln!(out, "{}. {} — {:?}", index + 1, step.name, step.outcome);
        if !step.prompt.trim().is_empty() {
            let _ = writeln!(out, "\nPrompt sent:\n```\n{}\n```", step.prompt.trim());
        }
        if !step.output.trim().is_empty() {
            let _ = writeln!(out, "\nOutput:\n```\n{}\n```\n", step.output.trim());
        }
    }

    if !record.diff.trim().is_empty() {
        let _ = writeln!(out, "## Diff\n\n```diff\n{}\n```", record.diff.trim());
    }
    out
}

pub fn save(record: &RunRecord, runs_dir: &Path, id: &str) -> io::Result<PathBuf> {
    let dir = runs_dir.join(id);
    fs::create_dir_all(&dir)?;
    let path = dir.join("report.md");
    fs::write(&path, render(record))?;
    save_step_artifacts(&dir, &record.steps)?;
    Ok(path)
}

fn save_step_artifacts(dir: &Path, steps: &[StepRecord]) -> io::Result<()> {
    if steps.is_empty() {
        return Ok(());
    }
    let steps_dir = dir.join("steps");
    fs::create_dir_all(&steps_dir)?;
    for (index, step) in steps.iter().enumerate() {
        let file = steps_dir.join(format!("{:02}-{}.md", index + 1, file_slug(&step.name)));
        fs::write(file, &step.output)?;
    }
    Ok(())
}

fn file_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
