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
    assert!(app.log.is_empty());
}

#[test]
fn typing_and_enter_submits_the_text() {
    let mut app = App::new();
    typed(&mut app, "add jwt");
    assert_eq!(app.input, "add jwt");
    assert_eq!(app.on_key(Key::Enter), Reaction::Submit("add jwt".to_string()));
    assert!(app.input.is_empty());
}

#[test]
fn pasting_multiline_text_fills_the_input_without_submitting() {
    let mut app = App::new();
    app.paste("first line\nsecond line");
    assert_eq!(app.input, "first line\nsecond line");
    assert_eq!(app.mode, Mode::Input);
}

#[test]
fn pasting_at_a_checkpoint_goes_to_feedback() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string()]);
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: String::new(),
        output: String::new(),
    });
    app.paste("make it simpler");
    assert_eq!(app.feedback, "make it simpler");
}

#[test]
fn submitting_appends_the_prompt_to_the_transcript() {
    let mut app = App::new();
    typed(&mut app, "add jwt");
    app.on_key(Key::Enter);
    assert_eq!(app.log.last().unwrap(), "› add jwt");
}

#[test]
fn a_steps_event_populates_the_step_list() {
    let mut app = App::new();
    app.start_question();
    app.apply(UiEvent::Steps(vec!["plan".to_string(), "implement".to_string()]));
    assert_eq!(app.steps.len(), 2);
    assert!(app.steps.iter().all(|s| s.status == StepStatus::Pending));
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
    app.apply(UiEvent::StepFinished {
        name: "plan".to_string(),
        outcome: StepOutcome::Success,
        output: String::new(),
    });
    assert_eq!(app.steps[0].status, StepStatus::Done(StepOutcome::Success));
}

#[test]
fn a_finished_step_appends_its_output_to_the_transcript() {
    let mut app = App::new();
    app.start_run(vec!["context".to_string()]);
    app.apply(UiEvent::StepFinished {
        name: "context".to_string(),
        outcome: StepOutcome::Success,
        output: "found src/auth.rs".to_string(),
    });
    let line = app.log.last().unwrap();
    assert!(line.contains("context"));
    assert!(line.contains("found src/auth.rs"));
}

#[test]
fn a_chunk_streams_a_progress_line_into_the_transcript() {
    let mut app = App::new();
    app.start_question();
    app.apply(UiEvent::Chunk("→ Read src/main.rs".to_string()));
    assert_eq!(app.log.last().unwrap(), "→ Read src/main.rs");
}

#[test]
fn an_answer_is_stored_and_shown_in_the_transcript() {
    let mut app = App::new();
    app.start_question();
    app.apply(UiEvent::Answer("here is the code".to_string()));
    assert_eq!(app.answer.as_deref(), Some("here is the code"));
    assert!(app.log.iter().any(|line| line.contains("here is the code")));
}

#[test]
fn arrows_scroll_the_transcript_back_and_forward() {
    let mut app = App::new();
    assert_eq!(app.scroll, 0);
    app.on_key(Key::Up); // back through history
    app.on_key(Key::Up);
    assert_eq!(app.scroll, 2);
    app.on_key(Key::Down);
    assert_eq!(app.scroll, 1);
    // Cannot go past the newest (bottom).
    app.on_key(Key::Down);
    app.on_key(Key::Down);
    assert_eq!(app.scroll, 0);
}

#[test]
fn page_keys_scroll_a_page_at_a_time() {
    let mut app = App::new();
    app.on_key(Key::PageUp);
    assert!(app.scroll >= 10);
    app.on_key(Key::PageDown);
    assert_eq!(app.scroll, 0);
}

#[test]
fn new_content_jumps_back_to_the_newest() {
    let mut app = App::new();
    app.start_question();
    app.on_key(Key::Up);
    app.on_key(Key::Up);
    assert_eq!(app.scroll, 2);
    app.apply(UiEvent::Answer("a long answer".to_string()));
    assert_eq!(app.scroll, 0);
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
fn finishing_returns_to_input_mode() {
    let mut app = App::new();
    app.start_run(vec!["plan".to_string()]);
    app.apply(UiEvent::Finished(Status::Accepted));
    assert_eq!(app.done, Some(Status::Accepted));
    assert_eq!(app.mode, Mode::Input);
}
