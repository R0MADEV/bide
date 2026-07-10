use crate::board::Blackboard;
use crate::core::{Step, StepOutcome};
use crate::dispatch::{StepHandler, StepReport};
use crate::exec;
use crate::policy::{Action, Policy};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(900);

/// Carries out an implementation by editing the repository. The port isolates
/// the Claude Code driver so the step logic can be tested without editing files.
pub trait Implementer {
    fn implement(&mut self, prompt: &str) -> ImplementResult;
}

pub struct ImplementResult {
    pub success: bool,
    pub summary: String,
}

/// Reports which files an implementation changed, so the policy can vet them.
pub trait ChangeSet {
    fn changed_files(&mut self) -> Vec<String>;
}

impl ChangeSet for crate::git::GitCli {
    fn changed_files(&mut self) -> Vec<String> {
        use crate::git::Git;
        self.status().changed_files
    }
}

/// The step where bide actually changes code: it asks the implementer to edit
/// the repo, then vets the changed files against the Policy Engine so an agent
/// cannot touch secrets even with edit permission.
pub struct ImplementStep {
    task: String,
    implementer: Box<dyn Implementer>,
    changes: Box<dyn ChangeSet>,
    policy: Policy,
}

impl ImplementStep {
    pub fn new(task: &str, implementer: Box<dyn Implementer>, changes: Box<dyn ChangeSet>) -> Self {
        ImplementStep {
            task: task.to_string(),
            implementer,
            changes,
            policy: Policy,
        }
    }

    fn forbidden_changes(&mut self) -> Vec<String> {
        self.changes
            .changed_files()
            .into_iter()
            .filter(|file| {
                let action = Action::AccessPath(PathBuf::from(file));
                self.policy.evaluate(&action).is_denied()
            })
            .collect()
    }
}

impl StepHandler for ImplementStep {
    fn handle(&mut self, _step: &Step, board: &Blackboard) -> StepReport {
        let prompt = build_implement_prompt(&self.task, board);
        let result = self.implementer.implement(&prompt);
        if !result.success {
            return StepReport::new(StepOutcome::Failure, result.summary);
        }

        let forbidden = self.forbidden_changes();
        if !forbidden.is_empty() {
            return StepReport::new(
                StepOutcome::Failure,
                format!("policy blocked edits to: {}", forbidden.join(", ")),
            );
        }
        StepReport::new(StepOutcome::Success, result.summary)
    }
}

pub fn build_implement_prompt(task: &str, board: &Blackboard) -> String {
    let mut prompt =
        format!("Implement the following task by editing the repository.\n\nTask: {task}\n");
    if !board.is_empty() {
        let _ = write!(prompt, "\nPlan and prior analysis:\n{}", board.summary());
    }
    prompt
}

/// Real driver: runs Claude Code headlessly under a timeout, accepting its edits,
/// so it changes files in the repo. Only reached when real agents are opted in.
#[derive(Debug, Default)]
pub struct ClaudeCodeImplementer;

impl Implementer for ClaudeCodeImplementer {
    fn implement(&mut self, prompt: &str) -> ImplementResult {
        let mut command = Command::new("claude");
        command
            .arg("-p")
            .arg(prompt)
            .arg("--permission-mode")
            .arg("acceptEdits");
        let captured = exec::run(command, TIMEOUT);
        ImplementResult {
            success: captured.success,
            summary: captured.merged().trim().to_string(),
        }
    }
}
