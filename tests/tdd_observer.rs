use bide::dispatch::{Dispatcher, Observer, StepHandler, StepReport};
use bide::{board::Blackboard, run, Step, StepOutcome, Workflow};
use std::cell::RefCell;
use std::rc::Rc;

struct AlwaysOk;

impl StepHandler for AlwaysOk {
    fn handle(&mut self, _step: &Step, _board: &Blackboard) -> StepReport {
        StepReport::new(StepOutcome::Success, "")
    }
}

struct Recorder {
    log: Rc<RefCell<Vec<String>>>,
}

impl Observer for Recorder {
    fn step_started(&mut self, step: &Step) {
        self.log.borrow_mut().push(format!("start:{}", step.name));
    }

    fn step_finished(&mut self, step: &Step, outcome: StepOutcome) {
        self.log
            .borrow_mut()
            .push(format!("done:{}:{outcome:?}", step.name));
    }
}

#[test]
fn the_observer_is_notified_as_each_step_starts_and_finishes() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let workflow = Workflow {
        steps: vec![Step::abort("plan"), Step::abort("implement")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("plan", Box::new(AlwaysOk));
    dispatcher.register("implement", Box::new(AlwaysOk));
    dispatcher.set_observer(Box::new(Recorder { log: log.clone() }));

    run(&workflow, &mut dispatcher);

    assert_eq!(
        *log.borrow(),
        vec![
            "start:plan",
            "done:plan:Success",
            "start:implement",
            "done:implement:Success",
        ]
    );
}
