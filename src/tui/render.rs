//! Drawing the interactive workspace. Kept in the library (not the binary) so
//! the layout can be snapshot-tested against a `TestBackend` buffer.

use super::{App, Checkpoint, Mode, StepStatus};
use crate::core::StepOutcome;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

/// Braille frames for the "working…" spinner, one per redraw tick.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
/// Width of the step sidebar, in columns.
const SIDEBAR_WIDTH: u16 = 18;

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const OK: Color = Color::Green;
const BAD: Color = Color::Red;
const RUNNING: Color = Color::Yellow;

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

/// A rounded panel with a muted border and the given title line.
fn panel(title: Line<'static>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(MUTED))
        .title(title)
}

fn plain_title(text: &str) -> Line<'static> {
    Line::styled(format!(" {text} "), Style::default().fg(MUTED))
}

/// The header " bide · agent · branch ": the name in the accent colour, the rest
/// muted.
fn header_title(header: &str) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (index, part) in header.split(" · ").enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(MUTED)));
        }
        let style = if index == 0 {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED)
        };
        spans.push(Span::styled(part.to_string(), style));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
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
    Paragraph::new(Text::from(transcript_lines(app)))
        .block(panel(header_title(&view.header)))
        .wrap(Wrap { trim: false })
        .scroll((top, 0))
}

/// The transcript as styled lines: your prompts stand out, tool-use dims, step
/// results are coloured by outcome, a checkpoint header is highlighted.
fn transcript_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = app.log.iter().map(|line| log_line(line)).collect();
    if let Some(checkpoint) = &app.checkpoint {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("── checkpoint: {} — review below ──", checkpoint.step),
            Style::default().fg(RUNNING).add_modifier(Modifier::BOLD),
        ));
        for line in checkpoint.output.trim().lines() {
            lines.push(Line::raw(line.to_string()));
        }
    }
    lines
}

fn log_line(line: &str) -> Line<'static> {
    let owned = line.to_string();
    let style = if line.starts_with("› ") {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else if line.starts_with("→ ") {
        Style::default().fg(MUTED)
    } else if line.starts_with("✓ ") {
        Style::default().fg(OK)
    } else if line.starts_with("✗ ") {
        Style::default().fg(BAD)
    } else {
        Style::default()
    };
    Line::styled(owned, style)
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
    let mut lines: Vec<Line> = app.steps.iter().map(step_row).collect();
    if app.mode == Mode::Running {
        let spin = SPINNER[view.tick % SPINNER.len()];
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(spin.to_string(), Style::default().fg(ACCENT)),
            Span::styled(format!(" {}s", view.elapsed_secs), Style::default().fg(MUTED)),
        ]));
    }
    Paragraph::new(Text::from(lines)).block(panel(plain_title("steps")))
}

/// A sidebar row: a coloured status mark and the step name (bold while running).
fn step_row(step: &super::StepView) -> Line<'static> {
    let (mark, colour) = mark_and_colour(&step.status);
    let name_style = match step.status {
        StepStatus::Running => Style::default().add_modifier(Modifier::BOLD),
        StepStatus::Pending => Style::default().fg(MUTED),
        StepStatus::Done(_) => Style::default(),
    };
    Line::from(vec![
        Span::styled(mark, Style::default().fg(colour)),
        Span::raw(" "),
        Span::styled(step.name.clone(), name_style),
    ])
}

fn mark_and_colour(status: &StepStatus) -> (&'static str, Color) {
    match status {
        StepStatus::Pending => ("·", MUTED),
        StepStatus::Running => ("▶", RUNNING),
        StepStatus::Done(StepOutcome::Success) => ("✓", OK),
        StepStatus::Done(_) => ("✗", BAD),
    }
}

/// The bottom box: a checkpoint's feedback prompt, a spinner while a task runs,
/// or the input prompt when idle. Editable ones grow to show multi-line text as
/// real lines, scrolling past a cap, with the key hints on the last line.
fn input_line(app: &App, view: &View) -> Paragraph<'static> {
    if let Some(checkpoint) = &app.checkpoint {
        let lines = editable_lines(
            "feedback › ",
            &app.feedback,
            "[Enter] continue · [⇧↵] newline · [Esc] abort",
        );
        let (_, scroll) = rows_and_scroll(&app.feedback);
        return box_of(lines, scroll)
            .block(panel(plain_title(&format!("checkpoint: {}", checkpoint.step))));
    }
    if app.mode == Mode::Running {
        let spin = SPINNER[view.tick % SPINNER.len()];
        let active = active_step(app).unwrap_or("working");
        let line = Line::from(vec![
            Span::styled(spin.to_string(), Style::default().fg(ACCENT)),
            Span::raw(format!(" {active} · {}s", view.elapsed_secs)),
            Span::styled("    [↑/↓] scroll", Style::default().fg(MUTED)),
        ]);
        return box_of(vec![line], 0).block(panel(plain_title("bide")));
    }
    let lines = editable_lines("› ", &app.input, "[Enter] send · [⇧↵] newline · [Esc] quit");
    let (_, scroll) = rows_and_scroll(&app.input);
    box_of(lines, scroll).block(panel(plain_title("bide")))
}

/// The editable text as styled lines: an accented prompt on the first line, the
/// text as real lines, and a muted hint on the last line.
fn editable_lines(prompt: &str, text: &str, hint: &'static str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    for (index, part) in text.split('\n').enumerate() {
        if index == 0 {
            lines.push(Line::from(vec![
                Span::styled(prompt.to_string(), Style::default().fg(ACCENT)),
                Span::raw(part.to_string()),
            ]));
        } else {
            lines.push(Line::raw(part.to_string()));
        }
    }
    lines.push(Line::styled(hint, Style::default().fg(MUTED)));
    lines
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
    rows + 3
}

fn box_of(lines: Vec<Line<'static>>, scroll: u16) -> Paragraph<'static> {
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}

fn active_step(app: &App) -> Option<&str> {
    app.steps
        .iter()
        .find(|step| step.status == StepStatus::Running)
        .map(|step| step.name.as_str())
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
