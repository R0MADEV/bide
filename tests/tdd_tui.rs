use bide::dispatch::Control;
use bide::tui::{App, Key, StepStatus, UiEvent};
use bide::{Status, StepOutcome};

fn app() -> App {
    App::new(vec!["plan".to_string(), "implement".to_string()])
}

#[test]
fn starts_with_every_step_pending() {
    let app = app();
    assert_eq!(app.steps.len(), 2);
    assert!(app.steps.iter().all(|s| s.status == StepStatus::Pending));
}

#[test]
fn events_update_step_status() {
    let mut app = app();
    app.apply(UiEvent::StepStarted("plan".to_string()));
    assert_eq!(app.steps[0].status, StepStatus::Running);

    app.apply(UiEvent::StepFinished("plan".to_string(), StepOutcome::Success));
    assert_eq!(app.steps[0].status, StepStatus::Done(StepOutcome::Success));
}

#[test]
fn a_checkpoint_event_opens_the_panel() {
    let mut app = app();
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: "please plan this".to_string(),
        output: "the plan body".to_string(),
    });
    let checkpoint = app.checkpoint.as_ref().expect("checkpoint open");
    assert_eq!(checkpoint.output, "the plan body");
    assert_eq!(checkpoint.prompt, "please plan this");
}

#[test]
fn keys_do_nothing_without_an_open_checkpoint() {
    let mut app = app();
    assert_eq!(app.on_key(Key::Enter), None);
}

#[test]
fn typing_edits_the_feedback_field() {
    let mut app = app();
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    app.on_key(Key::Char('h'));
    app.on_key(Key::Char('i'));
    assert_eq!(app.feedback, "hi");
    app.on_key(Key::Backspace);
    assert_eq!(app.feedback, "h");
}

#[test]
fn enter_without_feedback_continues() {
    let mut app = app();
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    assert_eq!(app.on_key(Key::Enter), Some(Control::Continue));
    assert!(app.checkpoint.is_none());
}

#[test]
fn enter_with_feedback_retries_with_it() {
    let mut app = app();
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    app.on_key(Key::Char('x'));
    assert_eq!(app.on_key(Key::Enter), Some(Control::Retry("x".to_string())));
}

#[test]
fn esc_aborts() {
    let mut app = app();
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    assert_eq!(app.on_key(Key::Esc), Some(Control::Abort));
}

#[test]
fn a_finished_event_records_the_status() {
    let mut app = app();
    app.apply(UiEvent::Finished(Status::Accepted));
    assert_eq!(app.done, Some(Status::Accepted));
}
