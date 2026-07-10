use bide::dispatch::Control;
use bide::tui::{App, Key, Mode, Reaction, StepStatus, UiEvent};
use bide::{Status, StepOutcome};

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        app.on_key(Key::Char(c));
    }
}

#[test]
fn starts_in_input_mode() {
    let app = App::new();
    assert_eq!(app.mode, Mode::Input);
    assert!(app.steps.is_empty());
}

#[test]
fn typing_a_task_and_enter_runs_it() {
    let mut app = App::new();
    typed(&mut app, "add jwt");
    assert_eq!(app.input, "add jwt");
    assert_eq!(app.on_key(Key::Enter), Reaction::RunTask("add jwt".to_string()));
    assert!(app.input.is_empty());
}

#[test]
fn a_question_prefix_asks_instead_of_running() {
    let mut app = App::new();
    typed(&mut app, "? how does resume work");
    assert_eq!(
        app.on_key(Key::Enter),
        Reaction::AskQuestion("how does resume work".to_string())
    );
}

#[test]
fn esc_in_input_quits() {
    let mut app = App::new();
    assert_eq!(app.on_key(Key::Esc), Reaction::Quit);
}

#[test]
fn empty_input_does_nothing() {
    let mut app = App::new();
    assert_eq!(app.on_key(Key::Enter), Reaction::None);
}

#[test]
fn start_run_sets_steps_pending_and_running_mode() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string(), "implement".to_string()]);
    assert_eq!(app.mode, Mode::Running);
    assert!(app.steps.iter().all(|s| s.status == StepStatus::Pending));
}

#[test]
fn events_update_step_status() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string()]);
    app.apply(UiEvent::StepStarted("plan".to_string()));
    assert_eq!(app.steps[0].status, StepStatus::Running);
    app.apply(UiEvent::StepFinished("plan".to_string(), StepOutcome::Success));
    assert_eq!(app.steps[0].status, StepStatus::Done(StepOutcome::Success));
}

#[test]
fn a_checkpoint_lets_the_user_continue_or_retry_with_feedback() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string()]);
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: "prompt".to_string(),
        output: "the plan".to_string(),
    });
    assert_eq!(app.checkpoint.as_ref().unwrap().output, "the plan");

    // Enter with no feedback continues.
    assert_eq!(app.on_key(Key::Enter), Reaction::Decide(Control::Continue));

    // With feedback it retries.
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    typed(&mut app, "simpler");
    assert_eq!(
        app.on_key(Key::Enter),
        Reaction::Decide(Control::Retry("simpler".to_string()))
    );
}

#[test]
fn esc_at_a_checkpoint_aborts() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string()]);
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    assert_eq!(app.on_key(Key::Esc), Reaction::Decide(Control::Abort));
}

#[test]
fn an_answer_event_is_stored() {
    let mut app = App::new();
    app.start_question();
    app.apply(UiEvent::Answer("here is the code".to_string()));
    assert_eq!(app.answer.as_deref(), Some("here is the code"));
}

#[test]
fn finishing_returns_to_input_mode() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string()]);
    app.apply(UiEvent::Finished(Status::Accepted));
    assert_eq!(app.done, Some(Status::Accepted));
    assert_eq!(app.mode, Mode::Input);
}
