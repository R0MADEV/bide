//! Snapshot tests for the TUI layout. Rendering into a `TestBackend` buffer and
//! dumping it to text lets the layout be verified without a real terminal.

use bide::tui::render::{draw, View};
use bide::tui::{App, UiEvent};
use bide::StepOutcome;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn view() -> View {
    View {
        header: "bide · gpt-4o · main".to_string(),
        tick: 2,
        elapsed_secs: 14,
    }
}

/// Render the app into a fixed-size buffer and return it as text, one line per
/// terminal row.
fn snapshot(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw(frame, app, &view())).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn mid_run() -> App {
    let mut app = App::new();
    app.log.push("› add a greeting function".to_string());
    app.start_run(vec![
        "context".to_string(),
        "plan".to_string(),
        "implement".to_string(),
        "verify".to_string(),
        "review".to_string(),
    ]);
    app.apply(UiEvent::StepFinished {
        name: "context".to_string(),
        outcome: StepOutcome::Success,
        output: "found src/cli/mod.rs, src/main.rs".to_string(),
    });
    app.apply(UiEvent::StepStarted("plan".to_string()));
    app
}

#[test]
fn workflow_layout_has_transcript_and_step_sidebar() {
    let screen = snapshot(&mid_run(), 70, 16);
    println!("\n{screen}");

    assert!(screen.contains("bide · gpt-4o · main"), "header missing");
    assert!(screen.contains("steps"), "sidebar missing");
    assert!(screen.contains("add a greeting function"), "prompt missing");
    assert!(screen.contains("found src/cli/mod.rs"), "step output missing");
    assert!(screen.contains("plan"), "step name missing");
}

#[test]
fn a_checkpoint_shows_its_feedback_prompt() {
    let mut app = mid_run();
    app.apply(UiEvent::Checkpoint {
        step: "plan".to_string(),
        prompt: "the whole task".to_string(),
        output: "1. add greet()  2. wire the subcommand".to_string(),
    });
    let screen = snapshot(&app, 70, 16);
    println!("\n{screen}");

    assert!(screen.contains("checkpoint: plan"), "checkpoint title missing");
    assert!(screen.contains("feedback"), "feedback prompt missing");
    assert!(screen.contains("wire the subcommand"), "plan output missing");
}

#[test]
fn idle_layout_is_a_single_panel_without_a_sidebar() {
    let screen = snapshot(&App::new(), 70, 10);
    println!("\n{screen}");

    assert!(screen.contains("bide · gpt-4o · main"), "header missing");
    assert!(!screen.contains("steps"), "sidebar should be absent when idle");
    assert!(screen.contains("[Enter] send"), "input hint missing");
}

#[test]
fn multiline_input_grows_and_shows_real_lines() {
    let mut app = App::new();
    app.paste("Arregla el login de Microsoft\ny muestra el selector de cuenta");
    let screen = snapshot(&app, 70, 12);
    println!("\n{screen}");
    assert!(screen.contains("Arregla el login de Microsoft"));
    assert!(screen.contains("y muestra el selector de cuenta"));
}

#[test]
fn a_long_input_scrolls_to_the_last_lines() {
    let mut app = App::new();
    let many: Vec<String> = (1..=15).map(|n| format!("line {n}")).collect();
    app.paste(&many.join("\n"));
    let screen = snapshot(&app, 70, 20);
    println!("\n{screen}");
    assert!(screen.contains("line 15"), "newest line must be visible");
    assert!(!screen.contains("line 1\n") && !screen.contains("│line 1 "), "top lines scrolled off");
}
