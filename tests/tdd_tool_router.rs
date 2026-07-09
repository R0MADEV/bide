use bide::policy::Policy;
use bide::tools::{run_guarded, Approver, CommandOutcome, CommandResult, Shell};

struct FakeShell {
    calls: Vec<String>,
    succeed: bool,
}

impl FakeShell {
    fn new(succeed: bool) -> Self {
        FakeShell {
            calls: Vec::new(),
            succeed,
        }
    }
}

impl Shell for FakeShell {
    fn run(&mut self, command: &str) -> CommandResult {
        self.calls.push(command.to_string());
        CommandResult {
            success: self.succeed,
        }
    }
}

struct FakeApprover {
    answer: bool,
    asked: bool,
}

impl Approver for FakeApprover {
    fn approve(&mut self, _reason: &str, _command: &str) -> bool {
        self.asked = true;
        self.answer
    }
}

#[test]
fn a_denied_command_never_reaches_the_shell() {
    let policy = Policy::default();
    let mut approver = FakeApprover {
        answer: true,
        asked: false,
    };
    let mut shell = FakeShell::new(true);

    let outcome = run_guarded(&policy, &mut approver, &mut shell, "rm -rf /");

    assert!(matches!(outcome, CommandOutcome::Denied(_)));
    assert!(shell.calls.is_empty());
}

#[test]
fn an_unapproved_command_never_reaches_the_shell() {
    let policy = Policy::default();
    let mut approver = FakeApprover {
        answer: false,
        asked: false,
    };
    let mut shell = FakeShell::new(true);

    let outcome = run_guarded(&policy, &mut approver, &mut shell, "rm notes.txt");

    assert!(matches!(outcome, CommandOutcome::Rejected(_)));
    assert!(approver.asked);
    assert!(shell.calls.is_empty());
}

#[test]
fn an_approved_command_runs() {
    let policy = Policy::default();
    let mut approver = FakeApprover {
        answer: true,
        asked: false,
    };
    let mut shell = FakeShell::new(true);

    let outcome = run_guarded(&policy, &mut approver, &mut shell, "rm notes.txt");

    assert_eq!(outcome, CommandOutcome::Ran { success: true });
    assert_eq!(shell.calls, vec!["rm notes.txt"]);
}

#[test]
fn an_allowed_command_runs_and_reports_the_exit_result() {
    let policy = Policy::default();
    let mut approver = FakeApprover {
        answer: true,
        asked: false,
    };
    let mut shell = FakeShell::new(false);

    let outcome = run_guarded(&policy, &mut approver, &mut shell, "cargo test");

    assert_eq!(outcome, CommandOutcome::Ran { success: false });
    assert!(!approver.asked);
}
