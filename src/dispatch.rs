use crate::core::{Step, StepOutcome, StepRunner};
use std::collections::HashMap;

/// Does the actual work for one step. Handlers are the seam where real tools
/// (context, agents, verification) plug into the engine.
pub trait StepHandler {
    fn handle(&mut self, step: &Step) -> StepOutcome;
}

/// Routes each step to the handler registered under its name. This is the
/// concrete StepRunner the engine drives once real work exists.
#[derive(Default)]
pub struct Dispatcher {
    handlers: HashMap<String, Box<dyn StepHandler>>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher::default()
    }

    pub fn register(&mut self, name: &str, handler: Box<dyn StepHandler>) -> &mut Self {
        self.handlers.insert(name.to_string(), handler);
        self
    }
}

impl StepRunner for Dispatcher {
    fn run(&mut self, step: &Step) -> StepOutcome {
        let Some(handler) = self.handlers.get_mut(&step.name) else {
            return StepOutcome::Failure;
        };
        handler.handle(step)
    }
}
