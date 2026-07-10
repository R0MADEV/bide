use crate::board::Blackboard;
use crate::core::{Step, StepOutcome, StepRunner};
use crate::report::StepRecord;
use std::collections::HashMap;

/// What a handler produced: the outcome the engine acts on, plus the output text
/// bide records (command result, agent reasoning).
pub struct StepReport {
    pub outcome: StepOutcome,
    pub output: String,
    /// The message sent to the AI, for agent steps (empty otherwise). Surfaced so
    /// the user can see exactly what bide asked.
    pub prompt: String,
}

impl StepReport {
    pub fn new(outcome: StepOutcome, output: impl Into<String>) -> Self {
        StepReport {
            outcome,
            output: output.into(),
            prompt: String::new(),
        }
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }
}

/// Does the actual work for one step. Handlers read shared state from the
/// blackboard and are the seam where real tools plug into the engine.
pub trait StepHandler {
    fn handle(&mut self, step: &Step, board: &Blackboard) -> StepReport;
}

/// What the user decides at a checkpoint. Retry carries optional feedback that is
/// fed back to the step (via the blackboard) so it can be re-run with guidance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Continue,
    Retry(String),
    Abort,
}

/// Consulted after a step that is a checkpoint (`pause = true`). The interactive
/// gate asks the user; the auto gate always continues.
pub trait Gate {
    fn checkpoint(&mut self, step: &Step, report: &StepReport) -> Control;

    /// A retry_from loop spent its budget. By default, offer the same choice as
    /// a checkpoint — continue keeps retrying, abort gives up — so an interactive
    /// gate asks the human for free. The auto gate overrides this to give up,
    /// keeping `--yes` (unattended) runs bounded.
    fn retry_limit(&mut self, step: &Step, report: &StepReport) -> bool {
        !matches!(self.checkpoint(step, report), Control::Abort)
    }
}

pub struct AutoGate;

impl Gate for AutoGate {
    fn checkpoint(&mut self, _step: &Step, _report: &StepReport) -> Control {
        Control::Continue
    }

    fn retry_limit(&mut self, _step: &Step, _report: &StepReport) -> bool {
        false
    }
}

/// Observes the run as it happens. The CLI prints progress; a desktop frontend
/// can forward these as UI events. Methods default to doing nothing.
pub trait Observer {
    fn step_started(&mut self, _step: &Step) {}
    fn step_finished(&mut self, _step: &Step, _report: &StepReport) {}
}

pub struct Silent;

impl Observer for Silent {}

/// Routes each step to its handler, feeds it the blackboard, records what every
/// step produced, and stops at checkpoints so the user can steer.
pub struct Dispatcher {
    handlers: HashMap<String, Box<dyn StepHandler>>,
    records: Vec<StepRecord>,
    board: Blackboard,
    gate: Box<dyn Gate>,
    observer: Box<dyn Observer>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Dispatcher {
            handlers: HashMap::new(),
            records: Vec::new(),
            board: Blackboard::new(),
            gate: Box::new(AutoGate),
            observer: Box::new(Silent),
        }
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher::default()
    }

    pub fn register(&mut self, name: &str, handler: Box<dyn StepHandler>) -> &mut Self {
        self.handlers.insert(name.to_string(), handler);
        self
    }

    pub fn set_gate(&mut self, gate: Box<dyn Gate>) -> &mut Self {
        self.gate = gate;
        self
    }

    pub fn set_observer(&mut self, observer: Box<dyn Observer>) -> &mut Self {
        self.observer = observer;
        self
    }

    pub fn into_records(self) -> Vec<StepRecord> {
        self.records
    }

    pub fn board_entries(&self) -> &[(String, String)] {
        self.board.entries()
    }

    /// Seed the blackboard with a previous run's step outputs, so a resumed run's
    /// remaining steps still see the context the skipped steps produced.
    pub fn preload_board(&mut self, entries: &[(String, String)]) {
        for (name, output) in entries {
            self.board.record(name, output);
        }
    }

    fn handle(&mut self, step: &Step) -> StepReport {
        match self.handlers.get_mut(&step.name) {
            Some(handler) => handler.handle(step, &self.board),
            None => StepReport::new(StepOutcome::Failure, "no handler registered"),
        }
    }

    fn record(&mut self, step: &Step, report: &StepReport) {
        self.board.record(&step.name, &report.output);
        self.records.push(StepRecord {
            name: step.name.clone(),
            outcome: report.outcome,
            output: report.output.clone(),
            prompt: report.prompt.clone(),
        });
    }
}

impl StepRunner for Dispatcher {
    fn run(&mut self, step: &Step) -> StepOutcome {
        loop {
            self.observer.step_started(step);
            let report = self.handle(step);
            let outcome = report.outcome;
            self.observer.step_finished(step, &report);
            self.record(step, &report);

            if !step.pause {
                return outcome;
            }
            match self.gate.checkpoint(step, &report) {
                Control::Continue => return outcome,
                Control::Abort => return StepOutcome::Aborted,
                Control::Retry(feedback) => {
                    if !feedback.trim().is_empty() {
                        self.board.record("feedback", &feedback);
                    }
                }
            }
        }
    }

    fn on_retry_limit(&mut self, step: &Step) -> bool {
        let last = self.records.last();
        let output = last.map(|record| record.output.as_str()).unwrap_or("");
        let report = StepReport::new(
            StepOutcome::Failure,
            format!("retry budget spent for '{}' — keep retrying, or abort?\n\n{output}", step.name),
        );
        self.gate.retry_limit(step, &report)
    }
}
