use crate::board::Blackboard;
use crate::context::{stream_claude, streaming_command};
use crate::core::{Step, StepOutcome};
use crate::dispatch::{StepHandler, StepReport};
use std::fmt::Write as _;
use std::rc::Rc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(900);

/// A sink for live progress lines (a tool an agent just used), so a long step
/// shows what it is doing. Used within a single run's thread, so `Rc` suffices.
pub type Progress = Rc<dyn Fn(&str)>;

/// A progress sink that discards everything, for non-interactive callers.
pub fn no_progress() -> Progress {
    Rc::new(|_line: &str| {})
}

/// Carries out an implementation by editing the repository. The port isolates
/// the Claude Code driver so the step logic can be tested without editing files.
pub trait Implementer {
    fn implement(&mut self, prompt: &str) -> ImplementResult;
}

pub struct ImplementResult {
    pub success: bool,
    pub summary: String,
}

/// The step where bide actually changes code: it asks the implementer to edit
/// the repo from the task and the plan on the blackboard. The user reviews the
/// changes at the interactive checkpoint after this step.
pub struct ImplementStep {
    task: String,
    implementer: Box<dyn Implementer>,
}

impl ImplementStep {
    pub fn new(task: &str, implementer: Box<dyn Implementer>) -> Self {
        ImplementStep {
            task: task.to_string(),
            implementer,
        }
    }
}

impl StepHandler for ImplementStep {
    fn handle(&mut self, _step: &Step, board: &Blackboard) -> StepReport {
        let prompt = build_implement_prompt(&self.task, board);
        let result = self.implementer.implement(&prompt);
        let outcome = if result.success {
            StepOutcome::Success
        } else {
            StepOutcome::Failure
        };
        StepReport::new(outcome, result.summary)
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
/// so it changes files in the repo. Streams each edit/read it makes to the
/// progress sink. Only reached when real agents are opted in.
pub struct ClaudeCodeImplementer {
    program: String,
    progress: Progress,
}

impl ClaudeCodeImplementer {
    pub fn new(program: &str, progress: Progress) -> Self {
        ClaudeCodeImplementer {
            program: program.to_string(),
            progress,
        }
    }
}

impl Implementer for ClaudeCodeImplementer {
    fn implement(&mut self, prompt: &str) -> ImplementResult {
        let command = streaming_command(
            &self.program,
            prompt,
            &["--permission-mode", "acceptEdits"],
        );
        let progress = &self.progress;
        let (success, summary) = stream_claude(command, TIMEOUT, |line| progress(line));
        ImplementResult {
            success,
            summary: summary.trim().to_string(),
        }
    }
}
