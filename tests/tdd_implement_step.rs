use bide::board::Blackboard;
use bide::dispatch::StepHandler;
use bide::tools::{build_implement_prompt, ImplementResult, ImplementStep, Implementer};
use bide::{Step, StepOutcome};

struct FakeImplementer {
    success: bool,
}

impl Implementer for FakeImplementer {
    fn implement(&mut self, _prompt: &str) -> ImplementResult {
        ImplementResult {
            success: self.success,
            summary: "edited 2 files".to_string(),
        }
    }
}

fn implement_step(success: bool) -> ImplementStep {
    ImplementStep::new("add jwt", Box::new(FakeImplementer { success }))
}

#[test]
fn a_successful_implementation_makes_the_step_succeed() {
    let mut handler = implement_step(true);
    let report = handler.handle(&Step::abort("implement"), &Blackboard::new());
    assert_eq!(report.outcome, StepOutcome::Success);
    assert!(report.output.contains("edited 2 files"));
}

#[test]
fn a_failed_implementation_makes_the_step_fail() {
    let mut handler = implement_step(false);
    let report = handler.handle(&Step::abort("implement"), &Blackboard::new());
    assert_eq!(report.outcome, StepOutcome::Failure);
}

#[test]
fn the_prompt_carries_the_task_and_the_prior_plan() {
    let mut board = Blackboard::new();
    board.record("plan", "step 1: add auth middleware");

    let prompt = build_implement_prompt("add jwt", &board);

    assert!(prompt.contains("add jwt"));
    assert!(prompt.contains("add auth middleware"));
}
