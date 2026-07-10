use bide::StepOutcome::*;
use bide::{Status, Step, Task, Workflow};

fn two_step_with_retry() -> Workflow {
    Workflow {
        steps: vec![Step::abort("implement"), Step::retry_from("verify", 0)],
        max_retries: 2,
    }
}

#[test]
fn success_advances_the_cursor_and_keeps_running() {
    let workflow = two_step_with_retry();
    let mut task = Task::new();

    let status = task.advance(&workflow, Success);

    assert_eq!(status, Status::Running);
    assert_eq!(task.cursor(), 1);
}

#[test]
fn success_on_the_last_step_accepts() {
    let workflow = two_step_with_retry();
    let mut task = Task::new();

    task.advance(&workflow, Success); // implement -> cursor 1
    let status = task.advance(&workflow, Success); // verify -> done

    assert_eq!(status, Status::Accepted);
}

#[test]
fn an_aborted_outcome_is_terminal() {
    let workflow = two_step_with_retry();
    let mut task = Task::new();
    assert_eq!(task.advance(&workflow, Aborted), Status::Aborted);
}

#[test]
fn abort_on_failure_fails_immediately() {
    let workflow = Workflow {
        steps: vec![Step::abort("only")],
        max_retries: 2,
    };
    let mut task = Task::new();

    assert_eq!(task.advance(&workflow, Failure), Status::Failed);
}

#[test]
fn retry_from_moves_the_cursor_back_and_counts_the_retry() {
    let workflow = two_step_with_retry();
    let mut task = Task::new();
    task.advance(&workflow, Success); // at verify

    let status = task.advance(&workflow, Failure);

    assert_eq!(status, Status::Running);
    assert_eq!(task.cursor(), 0);
}

#[test]
fn retries_are_capped_at_max_retries_by_default() {
    let workflow = Workflow {
        steps: vec![Step::retry_from("loop", 0)],
        max_retries: 2,
    };
    let mut task = Task::new();

    assert_eq!(task.advance(&workflow, Failure), Status::Running); // retry 1
    assert_eq!(task.advance(&workflow, Failure), Status::Running); // retry 2
    assert_eq!(task.advance(&workflow, Failure), Status::Failed); // over budget
}

#[test]
fn force_retry_grants_a_fresh_budget_and_jumps_back() {
    // At the limit an interactive run can keep going: force_retry resets the
    // budget and returns to the retry_from target.
    let workflow = Workflow {
        steps: vec![Step::retry_from("loop", 0)],
        max_retries: 2,
    };
    let mut task = Task::new();
    task.advance(&workflow, Failure); // retry 1
    task.advance(&workflow, Failure); // retry 2 (budget now spent)
    assert!(task.hit_retry_limit(&workflow, Failure));

    task.force_retry(&workflow);

    assert_eq!(task.cursor(), 0);
    assert!(!task.hit_retry_limit(&workflow, Failure)); // budget refreshed
}
