use bide::{run, Status, Step, StepOutcome, StepRunner, Workflow};
use std::collections::HashMap;

/// Records every step it runs and fails a step a configurable number of times
/// before letting it succeed.
struct ScriptedRunner {
    visited: Vec<String>,
    remaining_failures: HashMap<String, u32>,
}

impl ScriptedRunner {
    fn new() -> Self {
        ScriptedRunner {
            visited: Vec::new(),
            remaining_failures: HashMap::new(),
        }
    }

    fn failing(mut self, step: &str, times: u32) -> Self {
        self.remaining_failures.insert(step.to_string(), times);
        self
    }
}

impl StepRunner for ScriptedRunner {
    fn run(&mut self, step: &Step) -> StepOutcome {
        self.visited.push(step.name.clone());
        let pending = self.remaining_failures.get_mut(&step.name);
        match pending {
            Some(n) if *n > 0 => {
                *n -= 1;
                StepOutcome::Failure
            }
            _ => StepOutcome::Success,
        }
    }
}

fn pipeline(steps: Vec<Step>, max_retries: u32) -> Workflow {
    Workflow { steps, max_retries }
}

#[test]
fn runs_every_step_of_a_custom_pipeline_in_order() {
    let workflow = pipeline(
        vec![
            Step::abort("search_code"),
            Step::abort("analyze"),
            Step::abort("critique"),
        ],
        0,
    );
    let mut runner = ScriptedRunner::new();

    let status = run(&workflow, &mut runner);

    assert_eq!(status, Status::Accepted);
    assert_eq!(runner.visited, vec!["search_code", "analyze", "critique"]);
}

#[test]
fn default_recipe_reaches_accepted_when_every_step_succeeds() {
    let mut runner = ScriptedRunner::new();
    let status = run(&Workflow::default_recipe(), &mut runner);
    assert_eq!(status, Status::Accepted);
}

#[test]
fn an_abort_step_failure_fails_the_run() {
    let workflow = pipeline(vec![Step::abort("a"), Step::abort("b")], 2);
    let mut runner = ScriptedRunner::new().failing("b", 1);

    let status = run(&workflow, &mut runner);

    assert_eq!(status, Status::Failed);
    assert_eq!(runner.visited, vec!["a", "b"]);
}

#[test]
fn retry_from_recovers_when_the_step_later_succeeds() {
    let workflow = pipeline(
        vec![Step::abort("implement"), Step::retry_from("verify", 0)],
        2,
    );
    let mut runner = ScriptedRunner::new().failing("verify", 1);

    let status = run(&workflow, &mut runner);

    assert_eq!(status, Status::Accepted);
    assert_eq!(
        runner.visited,
        vec!["implement", "verify", "implement", "verify"]
    );
}

#[test]
fn retry_from_fails_once_the_limit_is_exhausted() {
    let workflow = pipeline(
        vec![Step::abort("implement"), Step::retry_from("verify", 0)],
        2,
    );
    let mut runner = ScriptedRunner::new().failing("verify", 99);

    let status = run(&workflow, &mut runner);

    assert_eq!(status, Status::Failed);
    let verify_attempts = runner.visited.iter().filter(|s| *s == "verify").count();
    assert_eq!(verify_attempts, 3); // first attempt + 2 retries
}
