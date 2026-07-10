//! Terminal UI state. The `App` here is pure and testable: it turns run events
//! into screen state and turns keys into checkpoint decisions. Rendering and the
//! terminal event loop live in the binary; the engine bridge is below.

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
    StepStarted(String),
    StepFinished(String, StepOutcome),
    Checkpoint {
        step: String,
        prompt: String,
        output: String,
    },
    Finished(Status),
}

/// The keys the UI understands, mapped from the terminal backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Esc,
    Backspace,
    Char(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub step: String,
    pub prompt: String,
    pub output: String,
}

pub struct App {
    pub steps: Vec<StepView>,
    pub checkpoint: Option<Checkpoint>,
    pub feedback: String,
    pub done: Option<Status>,
}

impl App {
    pub fn new(step_names: Vec<String>) -> Self {
        let steps = step_names
            .into_iter()
            .map(|name| StepView {
                name,
                status: StepStatus::Pending,
            })
            .collect();
        App {
            steps,
            checkpoint: None,
            feedback: String::new(),
            done: None,
        }
    }

    pub fn apply(&mut self, event: UiEvent) {
        match event {
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
            }
            UiEvent::Finished(status) => self.done = Some(status),
        }
    }

    /// Handle a key while a checkpoint is open. Enter proceeds (continue when the
    /// feedback is empty, otherwise re-run with it); Esc aborts; other keys edit
    /// the feedback. Returns a decision to send to the engine when one is made.
    pub fn on_key(&mut self, key: Key) -> Option<Control> {
        self.checkpoint.as_ref()?;
        match key {
            Key::Enter => Some(self.resolve()),
            Key::Esc => {
                self.close();
                Some(Control::Abort)
            }
            Key::Backspace => {
                self.feedback.pop();
                None
            }
            Key::Char(c) => {
                self.feedback.push(c);
                None
            }
        }
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
