use super::{run_guarded, Approver, CommandOutcome, Shell};
use crate::board::Blackboard;
use crate::core::{Step, StepOutcome};
use crate::dispatch::{StepHandler, StepReport};
use crate::policy::Policy;

/// A step whose work is running a shell command behind the policy gate. Only a
/// successful run makes the step succeed; denied, rejected or failing commands
/// fail the step.
pub struct CommandStep {
    command: String,
    policy: Policy,
    shell: Box<dyn Shell>,
    approver: Box<dyn Approver>,
}

impl CommandStep {
    pub fn new(command: &str, shell: Box<dyn Shell>, approver: Box<dyn Approver>) -> Self {
        CommandStep {
            command: command.to_string(),
            policy: Policy,
            shell,
            approver,
        }
    }
}

impl StepHandler for CommandStep {
    fn handle(&mut self, _step: &Step, _board: &Blackboard) -> StepReport {
        let outcome = run_guarded(
            &self.policy,
            self.approver.as_mut(),
            self.shell.as_mut(),
            &self.command,
        );
        match outcome {
            CommandOutcome::Ran { success, output } => {
                let step_outcome = if success {
                    StepOutcome::Success
                } else {
                    StepOutcome::Failure
                };
                StepReport::new(step_outcome, format!("$ {}\n{}", self.command, output))
            }
            CommandOutcome::Denied(reason) => {
                StepReport::new(StepOutcome::Failure, format!("denied by policy: {reason}"))
            }
            CommandOutcome::Rejected(reason) => {
                StepReport::new(StepOutcome::Failure, format!("rejected: {reason}"))
            }
        }
    }
}
