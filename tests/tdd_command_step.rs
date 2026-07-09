use bide::dispatch::StepHandler;
use bide::tools::{CommandResult, CommandStep, Shell};
use bide::{Step, StepOutcome};

struct FakeShell {
    calls: Vec<String>,
    succeed: bool,
    output: String,
}

impl Shell for FakeShell {
    fn run(&mut self, command: &str) -> CommandResult {
        self.calls.push(command.to_string());
        CommandResult {
            success: self.succeed,
            output: self.output.clone(),
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
    command_step_with_output(command, succeed, "")
}

fn command_step_with_output(command: &str, succeed: bool, output: &str) -> CommandStep {
    CommandStep::new(
        command,
        Box::new(FakeShell {
            calls: Vec::new(),
            succeed,
            output: output.to_string(),
        }),
        Box::new(AlwaysApprove),
    )
}

#[test]
fn a_succeeding_command_makes_the_step_succeed() {
    let mut handler = command_step("cargo test", true);
    let report = handler.handle(&Step::abort("verify"));
    assert_eq!(report.outcome, StepOutcome::Success);
}

#[test]
fn a_failing_command_makes_the_step_fail() {
    let mut handler = command_step("cargo test", false);
    let report = handler.handle(&Step::abort("verify"));
    assert_eq!(report.outcome, StepOutcome::Failure);
}

#[test]
fn a_denied_command_makes_the_step_fail() {
    let mut handler = command_step("rm -rf /", true);
    let report = handler.handle(&Step::abort("verify"));
    assert_eq!(report.outcome, StepOutcome::Failure);
}

#[test]
fn the_command_output_is_captured_in_the_report() {
    let mut handler = command_step_with_output("cargo test", false, "error[E0382]: borrow of moved value");
    let report = handler.handle(&Step::abort("verify"));
    assert!(report.output.contains("E0382"));
}
