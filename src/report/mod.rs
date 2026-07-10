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
    /// The code context captured for the run (lexis/Claude). Saved as context.md.
    pub context: String,
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
    if !record.context.trim().is_empty() {
        fs::write(dir.join("context.md"), &record.context)?;
    }
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

/// A run is worth keeping if it changed the working tree during the run, or if
/// it did not simply succeed (a failure/abort is worth inspecting). A successful
/// no-op (stub or nothing-to-do) is just noise on disk.
pub fn worth_saving(status: Status, changed: bool) -> bool {
    changed || status != Status::Accepted
}

/// Keep only the newest `keep` run directories; delete the rest.
pub fn prune(runs_dir: &Path, keep: usize) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(runs_dir) else {
        return Ok(());
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    // Run ids are run-<epoch-seconds>, so name order is chronological.
    dirs.sort();
    if dirs.len() <= keep {
        return Ok(());
    }
    for old in &dirs[..dirs.len() - keep] {
        let _ = fs::remove_dir_all(old);
    }
    Ok(())
}

/// Remove every saved run directory in `runs_dir` and return how many were
/// removed. A missing directory is not an error: nothing to clean means zero.
pub fn clean(runs_dir: &Path) -> io::Result<usize> {
    let Ok(entries) = fs::read_dir(runs_dir) else {
        return Ok(0);
    };
    let dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    for dir in &dirs {
        fs::remove_dir_all(dir)?;
    }
    Ok(dirs.len())
}

fn file_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
