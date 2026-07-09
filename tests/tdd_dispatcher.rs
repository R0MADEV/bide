use bide::dispatch::{Dispatcher, StepHandler, StepReport};
use bide::{run, Status, Step, StepOutcome, StepRunner, Workflow};

struct Fixed(StepOutcome);

impl StepHandler for Fixed {
    fn handle(&mut self, _step: &Step) -> StepReport {
        StepReport::new(self.0, "")
    }
}

#[test]
fn routes_a_step_to_its_registered_handler() {
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("search", Box::new(Fixed(StepOutcome::Success)));
    dispatcher.register("verify", Box::new(Fixed(StepOutcome::Failure)));

    assert_eq!(dispatcher.run(&Step::abort("search")), StepOutcome::Success);
    assert_eq!(dispatcher.run(&Step::abort("verify")), StepOutcome::Failure);
}

#[test]
fn an_unregistered_step_fails() {
    let mut dispatcher = Dispatcher::new();
    assert_eq!(dispatcher.run(&Step::abort("ghost")), StepOutcome::Failure);
}

#[test]
fn drives_a_workflow_through_registered_handlers() {
    let workflow = Workflow {
        steps: vec![Step::abort("plan"), Step::abort("implement")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("plan", Box::new(Fixed(StepOutcome::Success)));
    dispatcher.register("implement", Box::new(Fixed(StepOutcome::Success)));

    assert_eq!(run(&workflow, &mut dispatcher), Status::Accepted);
}
