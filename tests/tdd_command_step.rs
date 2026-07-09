use bide::dispatch::StepHandler;
use bide::tools::{CommandResult, CommandStep, Shell};
use bide::{Step, StepOutcome};

struct FakeShell {
    calls: Vec<String>,
    succeed: bool,
}

impl Shell for FakeShell {
    fn run(&mut self, command: &str) -> CommandResult {
        self.calls.push(command.to_string());
        CommandResult {
            success: self.succeed,
        }
    }
}

struct AlwaysApprove;

impl bide::tools::Approver for AlwaysApprove {
    fn approve(&mut self, _reason: &str, _command: &str) -> bool {
        true
    }
}

fn command_step(command: &str, succeed: bool) -> CommandStep {
    CommandStep::new(
        command,
        Box::new(FakeShell {
            calls: Vec::new(),
            succeed,
        }),
        Box::new(AlwaysApprove),
    )
}

#[test]
fn a_succeeding_command_makes_the_step_succeed() {
    let mut handler = command_step("cargo test", true);
    let outcome = handler.handle(&Step::abort("verify"));
    assert_eq!(outcome, StepOutcome::Success);
}

#[test]
fn a_failing_command_makes_the_step_fail() {
    let mut handler = command_step("cargo test", false);
    let outcome = handler.handle(&Step::abort("verify"));
    assert_eq!(outcome, StepOutcome::Failure);
}

#[test]
fn a_denied_command_makes_the_step_fail() {
    let mut handler = command_step("rm -rf /", true);
    let outcome = handler.handle(&Step::abort("verify"));
    assert_eq!(outcome, StepOutcome::Failure);
}
