mod command_step;
mod implement;
mod process;

pub use command_step::CommandStep;
pub use implement::{
    build_implement_prompt, no_progress, ClaudeCodeImplementer, ImplementResult, ImplementStep,
    Implementer, Progress,
};
pub use process::ProcessShell;

use crate::policy::{Action, Decision, Policy};

/// Runs a command and reports whether it succeeded. The port that isolates real
/// process execution from the gating logic.
pub trait Shell {
    fn run(&mut self, command: &str) -> CommandResult;
}

/// Asks the user to confirm an action the policy flagged for approval.
pub trait Approver {
    fn approve(&mut self, reason: &str, command: &str) -> bool;
}

pub struct CommandResult {
    pub success: bool,
    pub output: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    Denied(String),
    Rejected(String),
    Ran { success: bool, output: String },
}

/// Runs a command only after the Policy Engine allows it. Denied commands never
/// reach the shell; approval-required commands run only if the approver agrees.
pub fn run_guarded(
    policy: &Policy,
    approver: &mut dyn Approver,
    shell: &mut dyn Shell,
    command: &str,
) -> CommandOutcome {
    let decision = policy.evaluate(&Action::RunCommand(command.to_string()));

    if let Decision::Deny(reason) = decision {
        return CommandOutcome::Denied(reason);
    }
    if let Decision::RequireApproval(reason) = decision {
        let approved = approver.approve(&reason, command);
        if !approved {
            return CommandOutcome::Rejected(reason);
        }
    }

    let result = shell.run(command);
    CommandOutcome::Ran {
        success: result.success,
        output: result.output,
    }
}
