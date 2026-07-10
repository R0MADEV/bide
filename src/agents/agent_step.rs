use super::{AgentRequest, AgentRunner, Verdict};
use crate::board::Blackboard;
use crate::core::{Step, StepOutcome};
use crate::dispatch::{StepHandler, StepReport};

/// A step whose work is asking an agent to reason. Only a Proceed verdict makes
/// the step succeed; a rejection or failure fails the step so the workflow's
/// on_failure policy decides what happens next.
pub struct AgentStep {
    role: String,
    input: String,
    runner: Box<dyn AgentRunner>,
}

impl AgentStep {
    pub fn new(role: &str, input: &str, runner: Box<dyn AgentRunner>) -> Self {
        AgentStep {
            role: role.to_string(),
            input: input.to_string(),
            runner,
        }
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    fn input_with(&self, board: &Blackboard) -> String {
        if board.is_empty() {
            return self.input.clone();
        }
        format!("{}\n\nPrior steps:\n{}", self.input, board.summary())
    }
}

impl StepHandler for AgentStep {
    fn handle(&mut self, _step: &Step, board: &Blackboard) -> StepReport {
        let request = AgentRequest {
            role: self.role.clone(),
            input: self.input_with(board),
        };
        let response = self.runner.run(&request);
        let outcome = match response.verdict {
            Verdict::Proceed => StepOutcome::Success,
            Verdict::Reject(_) | Verdict::Failed(_) => StepOutcome::Failure,
        };
        StepReport::new(outcome, response.output).with_prompt(request.input)
    }
}
