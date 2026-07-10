use bide::dispatch::{Control, Dispatcher, Gate, StepHandler, StepReport};
use bide::{board::Blackboard, run, Status, Step, StepOutcome, Workflow};

struct AlwaysOk;

impl StepHandler for AlwaysOk {
    fn handle(&mut self, _step: &Step, _board: &Blackboard) -> StepReport {
        StepReport::new(StepOutcome::Success, "done")
    }
}

struct ScriptedGate {
    decisions: Vec<Control>,
}

impl Gate for ScriptedGate {
    fn checkpoint(&mut self, _step: &Step, _report: &StepReport) -> Control {
        if self.decisions.is_empty() {
            return Control::Continue;
        }
        self.decisions.remove(0)
    }
}

fn checkpoint(name: &str) -> Step {
    let mut step = Step::abort(name);
    step.pause = true;
    step
}

#[test]
fn a_retry_at_a_checkpoint_re_runs_the_step() {
    let workflow = Workflow {
        steps: vec![checkpoint("implement")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("implement", Box::new(AlwaysOk));
    dispatcher.set_gate(Box::new(ScriptedGate {
        decisions: vec![Control::Retry(String::new()), Control::Continue],
    }));

    let status = run(&workflow, &mut dispatcher);

    assert_eq!(status, Status::Accepted);
    let ran = dispatcher
        .into_records()
        .iter()
        .filter(|record| record.name == "implement")
        .count();
    assert_eq!(ran, 2); // ran once, retried once
}

#[test]
fn retry_feedback_is_recorded_so_the_re_run_sees_it() {
    let workflow = Workflow {
        steps: vec![checkpoint("plan")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("plan", Box::new(AlwaysOk));
    dispatcher.set_gate(Box::new(ScriptedGate {
        decisions: vec![Control::Retry("make it simpler".to_string()), Control::Continue],
    }));

    run(&workflow, &mut dispatcher);

    let recorded = dispatcher
        .board_entries()
        .iter()
        .any(|(name, output)| name == "feedback" && output == "make it simpler");
    assert!(recorded);
}

#[test]
fn an_abort_at_a_checkpoint_stops_the_run() {
    let workflow = Workflow {
        steps: vec![checkpoint("plan"), Step::abort("next")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("plan", Box::new(AlwaysOk));
    dispatcher.register("next", Box::new(AlwaysOk));
    dispatcher.set_gate(Box::new(ScriptedGate {
        decisions: vec![Control::Abort],
    }));

    let status = run(&workflow, &mut dispatcher);

    assert_eq!(status, Status::Aborted);
    let ran_next = dispatcher
        .into_records()
        .iter()
        .any(|record| record.name == "next");
    assert!(!ran_next);
}

#[test]
fn a_step_without_pause_never_consults_the_gate() {
    let workflow = Workflow {
        steps: vec![Step::abort("a")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("a", Box::new(AlwaysOk));
    // The gate would abort if consulted — but the step is not a checkpoint.
    dispatcher.set_gate(Box::new(ScriptedGate {
        decisions: vec![Control::Abort],
    }));

    assert_eq!(run(&workflow, &mut dispatcher), Status::Accepted);
}
