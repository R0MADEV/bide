use crate::board::Blackboard;
use crate::core::{Step, StepOutcome, StepRunner};
use crate::report::StepRecord;
use std::collections::HashMap;

/// What a handler produced: the outcome the engine acts on, plus the output text
/// bide records (command result, agent reasoning).
pub struct StepReport {
    pub outcome: StepOutcome,
    pub output: String,
}

impl StepReport {
    pub fn new(outcome: StepOutcome, output: impl Into<String>) -> Self {
        StepReport {
            outcome,
            output: output.into(),
        }
    }
}

/// Does the actual work for one step. Handlers read shared state from the
/// blackboard and are the seam where real tools plug into the engine.
pub trait StepHandler {
    fn handle(&mut self, step: &Step, board: &Blackboard) -> StepReport;
}

/// What the user decides at a checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Retry,
    Abort,
}

/// Consulted after a step that is a checkpoint (`pause = true`). The interactive
/// gate asks the user; the auto gate always continues.
pub trait Gate {
    fn checkpoint(&mut self, step: &Step, report: &StepReport) -> Control;
}

pub struct AutoGate;

impl Gate for AutoGate {
    fn checkpoint(&mut self, _step: &Step, _report: &StepReport) -> Control {
        Control::Continue
    }
}

/// Routes each step to its handler, feeds it the blackboard, records what every
/// step produced, and stops at checkpoints so the user can steer.
pub struct Dispatcher {
    handlers: HashMap<String, Box<dyn StepHandler>>,
    records: Vec<StepRecord>,
    board: Blackboard,
    gate: Box<dyn Gate>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Dispatcher {
            handlers: HashMap::new(),
            records: Vec::new(),
            board: Blackboard::new(),
            gate: Box::new(AutoGate),
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

    pub fn into_records(self) -> Vec<StepRecord> {
        self.records
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
        });
    }
}

impl StepRunner for Dispatcher {
    fn run(&mut self, step: &Step) -> StepOutcome {
        loop {
            let report = self.handle(step);
            let outcome = report.outcome;
            self.record(step, &report);

            if !step.pause {
                return outcome;
            }
            match self.gate.checkpoint(step, &report) {
                Control::Continue => return outcome,
                Control::Abort => return StepOutcome::Aborted,
                Control::Retry => continue,
            }
        }
    }
}
