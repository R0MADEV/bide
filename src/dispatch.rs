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

/// Routes each step to the handler registered under its name, feeds it the
/// shared blackboard, and records what every step produced.
#[derive(Default)]
pub struct Dispatcher {
    handlers: HashMap<String, Box<dyn StepHandler>>,
    records: Vec<StepRecord>,
    board: Blackboard,
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher::default()
    }

    pub fn register(&mut self, name: &str, handler: Box<dyn StepHandler>) -> &mut Self {
        self.handlers.insert(name.to_string(), handler);
        self
    }

    pub fn into_records(self) -> Vec<StepRecord> {
        self.records
    }
}

impl StepRunner for Dispatcher {
    fn run(&mut self, step: &Step) -> StepOutcome {
        let report = match self.handlers.get_mut(&step.name) {
            Some(handler) => handler.handle(step, &self.board),
            None => StepReport::new(StepOutcome::Failure, "no handler registered"),
        };

        self.board.record(&step.name, &report.output);
        self.records.push(StepRecord {
            name: step.name.clone(),
            outcome: report.outcome,
            output: report.output,
        });
        report.outcome
    }
}
