//! Terminal UI state. The `App` here is pure and testable: it turns run events
//! into screen state and turns keys into reactions (submit a task/question,
//! decide a checkpoint, quit). Rendering and the terminal loop live in the
//! binary; the engine bridge is below.

mod bridge;
pub mod render;

pub use bridge::{ChannelGate, ChannelObserver};

use crate::core::{Status, StepOutcome};
use crate::dispatch::Control;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done(StepOutcome),
}

/// One step in the sidebar: just its name and status. The output it produced is
/// appended to the transcript, not kept here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepView {
    pub name: String,
    pub status: StepStatus,
}

/// What the running engine tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    Steps(Vec<String>),
    StepStarted(String),
    StepFinished {
        name: String,
        outcome: StepOutcome,
        output: String,
    },
    Checkpoint {
        step: String,
        prompt: String,
        output: String,
    },
    /// A live progress line streamed from a running agent (a tool it just used).
    Chunk(String),
    Answer(String),
    Finished(Status),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    /// Shift/Alt+Enter: insert a line break instead of submitting.
    Newline,
    Esc,
    Backspace,
    Up,
    Down,
    PageUp,
    PageDown,
    Char(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub step: String,
    pub prompt: String,
    pub output: String,
}

/// Whether the UI is waiting for the user to type (Input) or a run is in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Input,
    Running,
}

/// What a key press asks the binary to do. The binary decides (with the AI)
/// whether a submission is a task or a question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reaction {
    None,
    Submit(String),
    Decide(Control),
    Quit,
}

pub struct App {
    pub mode: Mode,
    pub input: String,
    /// The conversation transcript: your prompts and each run's step outputs and
    /// answers, oldest first. The main panel shows it, anchored to the newest.
    pub log: Vec<String>,
    /// The current run's steps, shown in the sidebar.
    pub steps: Vec<StepView>,
    pub checkpoint: Option<Checkpoint>,
    pub feedback: String,
    pub answer: Option<String>,
    pub done: Option<Status>,
    /// How many lines the transcript is scrolled back from the newest. 0 shows
    /// the bottom (the latest); ↑/PageUp move back through history.
    pub scroll: u16,
}

impl Default for App {
    fn default() -> Self {
        App {
            mode: Mode::Input,
            input: String::new(),
            log: Vec::new(),
            steps: Vec::new(),
            checkpoint: None,
            feedback: String::new(),
            answer: None,
            done: None,
            scroll: 0,
        }
    }
}

/// How many lines a PageUp/PageDown moves the transcript.
const PAGE: u16 = 10;

/// A step that has not run yet.
fn pending(name: String) -> StepView {
    StepView {
        name,
        status: StepStatus::Pending,
    }
}

/// A one-line transcript summary of a finished step: its mark, name and the
/// first line of its output.
fn step_line(mark: &str, name: &str, output: &str) -> String {
    let summary = output.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    if summary.trim().is_empty() {
        return format!("{mark} {name}");
    }
    format!("{mark} {name}  {}", summary.trim())
}

fn outcome_mark(outcome: StepOutcome) -> &'static str {
    match outcome {
        StepOutcome::Success => "✓",
        _ => "✗",
    }
}

impl App {
    pub fn new() -> Self {
        App::default()
    }

    /// Begin a workflow run with these step names. The transcript is kept; only
    /// the run state (sidebar steps, checkpoint) is reset.
    pub fn start_run(&mut self, step_names: Vec<String>) {
        self.mode = Mode::Running;
        self.steps = step_names.into_iter().map(pending).collect();
        self.checkpoint = None;
        self.feedback.clear();
        self.answer = None;
        self.done = None;
        self.scroll = 0;
    }

    /// Begin a question (Claude + lexis); no workflow steps.
    pub fn start_question(&mut self) {
        self.mode = Mode::Running;
        self.steps = Vec::new();
        self.checkpoint = None;
        self.answer = None;
        self.done = None;
        self.scroll = 0;
    }

    pub fn apply(&mut self, event: UiEvent) {
        match event {
            UiEvent::Steps(names) => {
                self.steps = names.into_iter().map(pending).collect();
            }
            UiEvent::StepStarted(name) => self.set_status(&name, StepStatus::Running),
            UiEvent::StepFinished {
                name,
                outcome,
                output,
            } => {
                self.set_status(&name, StepStatus::Done(outcome));
                self.log.push(step_line(outcome_mark(outcome), &name, &output));
                self.scroll = 0;
            }
            UiEvent::Checkpoint {
                step,
                prompt,
                output,
            } => {
                self.checkpoint = Some(Checkpoint {
                    step,
                    prompt,
                    output,
                });
                self.feedback.clear();
                self.scroll = 0;
            }
            UiEvent::Chunk(text) => {
                for line in text.lines() {
                    self.log.push(line.to_string());
                }
                self.scroll = 0;
            }
            UiEvent::Answer(text) => {
                for line in text.lines() {
                    self.log.push(line.to_string());
                }
                self.answer = Some(text);
                self.scroll = 0;
            }
            UiEvent::Finished(status) => {
                self.done = Some(status);
                self.mode = Mode::Input;
            }
        }
    }

    pub fn on_key(&mut self, key: Key) -> Reaction {
        // Navigation keys scroll the transcript back through history; they never
        // type. 0 is the newest (bottom).
        match key {
            Key::Up => return self.scroll_by(1),
            Key::Down => return self.scroll_by(-1),
            Key::PageUp => return self.scroll_by(i32::from(PAGE)),
            Key::PageDown => return self.scroll_by(-i32::from(PAGE)),
            _ => {}
        }
        match self.mode {
            Mode::Running => self.on_key_running(key),
            Mode::Input => self.on_key_input(key),
        }
    }

    fn scroll_by(&mut self, delta: i32) -> Reaction {
        let target = (i32::from(self.scroll) + delta).max(0);
        self.scroll = u16::try_from(target).unwrap_or(u16::MAX);
        Reaction::None
    }

    fn on_key_running(&mut self, key: Key) -> Reaction {
        if self.checkpoint.is_none() {
            return Reaction::None;
        }
        match key {
            Key::Enter => Reaction::Decide(self.resolve()),
            Key::Newline => {
                self.feedback.push('\n');
                Reaction::None
            }
            Key::Esc => {
                self.close();
                Reaction::Decide(Control::Abort)
            }
            Key::Backspace => {
                self.feedback.pop();
                Reaction::None
            }
            Key::Char(c) => {
                self.feedback.push(c);
                Reaction::None
            }
            _ => Reaction::None,
        }
    }

    /// Insert pasted text (which may span multiple lines) without submitting.
    /// Goes to the checkpoint feedback when one is open, otherwise to the input.
    pub fn paste(&mut self, text: &str) {
        if self.checkpoint.is_some() {
            self.feedback.push_str(text);
        } else {
            self.input.push_str(text);
        }
    }

    fn on_key_input(&mut self, key: Key) -> Reaction {
        match key {
            Key::Esc => Reaction::Quit,
            Key::Enter => self.submit(),
            Key::Newline => {
                self.input.push('\n');
                Reaction::None
            }
            Key::Backspace => {
                self.input.pop();
                Reaction::None
            }
            Key::Char(c) => {
                self.input.push(c);
                Reaction::None
            }
            _ => Reaction::None,
        }
    }

    fn submit(&mut self) -> Reaction {
        let text = self.input.trim().to_string();
        self.input.clear();
        if text.is_empty() {
            return Reaction::None;
        }
        self.log.push(format!("› {text}"));
        self.scroll = 0;
        Reaction::Submit(text)
    }

    fn resolve(&mut self) -> Control {
        let decision = if self.feedback.trim().is_empty() {
            Control::Continue
        } else {
            Control::Retry(self.feedback.clone())
        };
        self.close();
        decision
    }

    fn close(&mut self) {
        self.checkpoint = None;
        self.feedback.clear();
    }

    fn set_status(&mut self, name: &str, status: StepStatus) {
        if let Some(view) = self.steps.iter_mut().find(|view| view.name == name) {
            view.status = status;
        }
    }
}
