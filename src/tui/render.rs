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
    let rows = Layout::vertical([Constraint::Min(0), Constraint::Length(input_height(app))])
        .split(frame.area());
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

/// The bottom box: a checkpoint's feedback prompt, a spinner while a task runs,
/// or the input prompt when idle. The editable ones grow to show multi-line text
/// as real lines, with the key hints on the last line.
fn input_line(app: &App, view: &View) -> Paragraph<'static> {
    if let Some(checkpoint) = &app.checkpoint {
        let body = format!(
            "feedback › {}\n[Enter] continue · [⇧↵] newline · [Esc] abort",
            app.feedback
        );
        let (_, scroll) = rows_and_scroll(&app.feedback);
        return bar(body, scroll)
            .block(Block::bordered().title(format!(" checkpoint: {} ", checkpoint.step)));
    }
    if app.mode == Mode::Running {
        let spin = SPINNER[view.tick % SPINNER.len()];
        let active = active_step(app).unwrap_or("working");
        return bar(
            format!("{spin} {active} · {}s    [↑/↓] scroll", view.elapsed_secs),
            0,
        );
    }
    let body = format!(
        "› {}\n[Enter] send · [⇧↵] newline · [Esc] quit",
        app.input
    );
    let (_, scroll) = rows_and_scroll(&app.input);
    bar(body, scroll).block(Block::bordered().title(" bide "))
}

/// The bottom box grows with the text up to this many lines; beyond that it
/// scrolls, anchored to the last (newest) line.
const MAX_INPUT_ROWS: u16 = 10;

/// How many rows the editable text occupies (capped) and how far to scroll so
/// its last line stays visible.
fn rows_and_scroll(text: &str) -> (u16, u16) {
    let total = u16::try_from(text.matches('\n').count()).unwrap_or(0) + 1;
    let visible = total.min(MAX_INPUT_ROWS);
    (visible, total - visible)
}

/// Height of the bottom box: one line while running, otherwise the (capped)
/// editable rows plus the hint line and borders.
fn input_height(app: &App) -> u16 {
    if app.mode == Mode::Running && app.checkpoint.is_none() {
        return 3;
    }
    let text = if app.checkpoint.is_some() {
        &app.feedback
    } else {
        &app.input
    };
    let (rows, _) = rows_and_scroll(text);
    // editable rows + hint line + top/bottom borders.
    rows + 3
}

fn bar(text: String, scroll: u16) -> Paragraph<'static> {
    Paragraph::new(text)
        .block(Block::bordered())
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
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
