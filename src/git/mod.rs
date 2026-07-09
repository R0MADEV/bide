use std::process::Command;

pub struct GitStatus {
    pub clean: bool,
    pub changed_files: Vec<String>,
}

/// Repository operations bide needs. Deterministic, no LLM. The port isolates
/// the `git` CLI so the parsing logic can be tested without a real repo.
pub trait Git {
    fn status(&mut self) -> GitStatus;
    fn current_branch(&mut self) -> Option<String>;
    fn create_branch(&mut self, name: &str) -> bool;
    fn diff(&mut self) -> String;
}

pub fn parse_status(porcelain: &str) -> GitStatus {
    let changed_files: Vec<String> = porcelain
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.get(3..).unwrap_or(line).trim().to_string())
        .collect();
    GitStatus {
        clean: changed_files.is_empty(),
        changed_files,
    }
}

pub fn branch_name(task: &str) -> String {
    let slug: String = task
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let words: Vec<&str> = slug.split('-').filter(|word| !word.is_empty()).collect();
    format!("bide/{}", words.join("-"))
}

/// Real repository access through the `git` CLI.
#[derive(Debug, Default)]
pub struct GitCli;

impl Git for GitCli {
    fn status(&mut self) -> GitStatus {
        parse_status(&run_git(&["status", "--porcelain"]).unwrap_or_default())
    }

    fn current_branch(&mut self) -> Option<String> {
        let name = run_git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        Some(name.to_string())
    }

    fn create_branch(&mut self, name: &str) -> bool {
        run_git(&["checkout", "-b", name]).is_some()
    }

    fn diff(&mut self) -> String {
        run_git(&["diff"]).unwrap_or_default()
    }
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}
