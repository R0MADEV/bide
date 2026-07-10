use super::state::{Status, StepOutcome};
use super::workflow::{OnFailure, Workflow};

/// The mutable run state: which step we are on and how many retries we spent.
/// bide always knows where it is without asking an agent.
pub struct Task {
    cursor: usize,
    retries: u32,
}

impl Task {
    pub fn new() -> Self {
        Task {
            cursor: 0,
            retries: 0,
        }
    }

    pub fn resumed(cursor: usize) -> Self {
        Task { cursor, retries: 0 }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// True when the just-failed step is a retry_from loop that has spent its
    /// budget. This is the moment an interactive run offers the human a choice
    /// (keep retrying or abort) instead of failing outright.
    pub fn hit_retry_limit(&self, workflow: &Workflow, outcome: StepOutcome) -> bool {
        if outcome != StepOutcome::Failure {
            return false;
        }
        let OnFailure::RetryFrom(_) = workflow.steps[self.cursor].on_failure else {
            return false;
        };
        self.retries == workflow.max_retries
    }

    /// Grant a fresh budget and jump back to the retry_from target, used when a
    /// human chooses to keep going at the limit. No-op on a non-retry step.
    pub fn force_retry(&mut self, workflow: &Workflow) {
        let OnFailure::RetryFrom(step) = workflow.steps[self.cursor].on_failure else {
            return;
        };
        self.retries = 0;
        self.cursor = step;
    }

    pub fn advance(&mut self, workflow: &Workflow, outcome: StepOutcome) -> Status {
        if outcome == StepOutcome::Aborted {
            return Status::Aborted;
        }
        if outcome == StepOutcome::Success {
            return self.on_success(workflow);
        }
        self.on_failure(workflow)
    }

    fn on_success(&mut self, workflow: &Workflow) -> Status {
        self.cursor += 1;
        let finished = self.cursor == workflow.steps.len();
        if finished {
            return Status::Accepted;
        }
        Status::Running
    }

    fn on_failure(&mut self, workflow: &Workflow) -> Status {
        let OnFailure::RetryFrom(step) = workflow.steps[self.cursor].on_failure else {
            return Status::Failed;
        };
        if self.retries == workflow.max_retries {
            return Status::Failed;
        }
        self.retries += 1;
        self.cursor = step;
        Status::Running
    }
}

impl Default for Task {
    fn default() -> Self {
        Self::new()
    }
}
