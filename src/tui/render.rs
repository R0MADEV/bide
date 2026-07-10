//! Drawing the interactive workspace. Kept in the library (not the binary) so
//! the layout can be snapshot-tested against a `TestBackend` buffer.

use super::{App, Checkpoint, Mode, StepStatus};
use crate::core::StepOutcome;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

/// Braille frames for the "working…" spinner, one per redraw tick.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
/// Width of the step sidebar, in columns.
const SIDEBAR_WIDTH: u16 = 18;

/// The transient view state the pure `App` does not hold.
pub struct View {
    /// The panel title, e.g. "bide · gpt-4o · main".
    pub header: String,
    /// Redraw counter, picks the spinner frame.
    pub tick: usize,
    /// Seconds the current run has been in flight.
    pub elapsed_secs: u64,
}

/// Draw the whole workspace: the transcript, an optional step sidebar, and the
/// input/status line.
pub fn draw(frame: &mut Frame, app: &App, view: &View) {
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(frame.area());
    let has_steps = !app.steps.is_empty();
    let (main_area, sidebar_area) = if has_steps {
        let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(SIDEBAR_WIDTH)])
            .split(rows[0]);
        (cols[0], Some(cols[1]))
    } else {
        (rows[0], None)
    };

    frame.render_widget(transcript(app, view, main_area), main_area);
    if let Some(area) = sidebar_area {
        frame.render_widget(sidebar(app, view), area);
    }
    frame.render_widget(input_line(app, view), rows[1]);
}

/// The main panel: the conversation, anchored to the newest line, scrollable
/// back through history with ↑/PageUp.
fn transcript(app: &App, view: &View, area: Rect) -> Paragraph<'static> {
    let body = transcript_body(app);
    let inner_width = area.width.saturating_sub(2);
    let inner_height = area.height.saturating_sub(2);
    let used = wrapped_lines(&body, inner_width);
    let bottom = used.saturating_sub(usize::from(inner_height));
    let top_line = bottom.saturating_sub(usize::from(app.scroll));
    let top = u16::try_from(top_line).unwrap_or(u16::MAX);
    Paragraph::new(body)
        .block(Block::bordered().title(format!(" {} ", view.header)))
        .wrap(Wrap { trim: false })
        .scroll((top, 0))
}

fn transcript_body(app: &App) -> String {
    let mut body = app.log.join("\n");
    let Some(checkpoint) = &app.checkpoint else {
        return body;
    };
    if !body.is_empty() {
        body.push_str("\n\n");
    }
    body.push_str(&checkpoint_block(checkpoint));
    body
}

fn checkpoint_block(checkpoint: &Checkpoint) -> String {
    format!(
        "── checkpoint: {} — review below ──\n{}",
        checkpoint.step,
        checkpoint.output.trim()
    )
}

fn sidebar(app: &App, view: &View) -> Paragraph<'static> {
    let mut body = String::new();
    for step in &app.steps {
        body.push_str(&format!("{} {}\n", step_mark(&step.status), step.name));
    }
    if app.mode == Mode::Running {
        let spin = SPINNER[view.tick % SPINNER.len()];
        body.push_str(&format!("\n{spin} {}s", view.elapsed_secs));
    }
    Paragraph::new(body).block(Block::bordered().title(" steps "))
}

/// The bottom line: a checkpoint's feedback prompt, a spinner while a task runs,
/// or the input prompt when idle.
fn input_line(app: &App, view: &View) -> Paragraph<'static> {
    if let Some(checkpoint) = &app.checkpoint {
        return bar(format!(
            "feedback › {}    [Enter] continue · [Esc] abort",
            app.feedback
        ))
        .block(Block::bordered().title(format!(" checkpoint: {} ", checkpoint.step)));
    }
    if app.mode == Mode::Running {
        let spin = SPINNER[view.tick % SPINNER.len()];
        let active = active_step(app).unwrap_or("working");
        return bar(format!(
            "{spin} {active} · {}s    [↑/↓] scroll",
            view.elapsed_secs
        ));
    }
    bar(format!(
        "› {}    [Enter] send · [Esc] quit",
        app.input
    ))
    .block(Block::bordered().title(" bide "))
}

fn bar(text: String) -> Paragraph<'static> {
    Paragraph::new(text).block(Block::bordered())
}

fn active_step(app: &App) -> Option<&str> {
    app.steps
        .iter()
        .find(|step| step.status == StepStatus::Running)
        .map(|step| step.name.as_str())
}

fn step_mark(status: &StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "·",
        StepStatus::Running => "▶",
        StepStatus::Done(StepOutcome::Success) => "✓",
        StepStatus::Done(_) => "✗",
    }
}

/// Estimate how many terminal rows the text occupies once wrapped to `width`, so
/// the transcript can be anchored to its last line.
fn wrapped_lines(text: &str, width: u16) -> usize {
    let width = usize::from(width.max(1));
    text.lines()
        .map(|line| {
            let chars = line.chars().count().max(1);
            chars.div_ceil(width)
        })
        .sum()
}
