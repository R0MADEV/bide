//! Terminal UI state. The `App` here is pure and testable: it turns run events
//! into screen state and turns keys into reactions (submit a task/question,
//! decide a checkpoint, quit). Rendering and the terminal loop live in the
//! binary; the engine bridge is below.

mod bridge;

pub use bridge::{ChannelGate, ChannelObserver};

use crate::core::{Status, StepOutcome};
use crate::dispatch::Control;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done(StepOutcome),
}

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
    StepFinished(String, StepOutcome),
    Checkpoint {
        step: String,
        prompt: String,
        output: String,
    },
    Answer(String),
    Finished(Status),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Esc,
    Backspace,
    Up,
    Down,
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
    pub steps: Vec<StepView>,
    pub checkpoint: Option<Checkpoint>,
    pub feedback: String,
    pub answer: Option<String>,
    pub done: Option<Status>,
    /// How far the bottom panel is scrolled, in lines. Reset when new content
    /// (an answer, a checkpoint, a fresh run) arrives.
    pub scroll: u16,
}

impl Default for App {
    fn default() -> Self {
        App {
            mode: Mode::Input,
            input: String::new(),
            steps: Vec::new(),
            checkpoint: None,
            feedback: String::new(),
            answer: None,
            done: None,
            scroll: 0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        App::default()
    }

    /// Begin a workflow run with these step names; clears the previous run.
    pub fn start_run(&mut self, step_names: Vec<String>) {
        self.mode = Mode::Running;
        self.steps = step_names
            .into_iter()
            .map(|name| StepView {
                name,
                status: StepStatus::Pending,
            })
            .collect();
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
                self.steps = names
                    .into_iter()
                    .map(|name| StepView {
                        name,
                        status: StepStatus::Pending,
                    })
                    .collect();
            }
            UiEvent::StepStarted(name) => self.set_status(&name, StepStatus::Running),
            UiEvent::StepFinished(name, outcome) => self.set_status(&name, StepStatus::Done(outcome)),
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
            UiEvent::Answer(text) => {
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
        // Arrows scroll the bottom panel in any mode; they never type.
        match key {
            Key::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                return Reaction::None;
            }
            Key::Down => {
                self.scroll = self.scroll.saturating_add(1);
                return Reaction::None;
            }
            _ => {}
        }
        match self.mode {
            Mode::Running => self.on_key_running(key),
            Mode::Input => self.on_key_input(key),
        }
    }

    fn on_key_running(&mut self, key: Key) -> Reaction {
        if self.checkpoint.is_none() {
            return Reaction::None;
        }
        match key {
            Key::Enter => Reaction::Decide(self.resolve()),
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

    fn on_key_input(&mut self, key: Key) -> Reaction {
        match key {
            Key::Esc => Reaction::Quit,
            Key::Enter => self.submit(),
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
