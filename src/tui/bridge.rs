use super::UiEvent;
use crate::core::Step;
use crate::dispatch::{Control, Gate, Observer, StepReport};
use std::sync::mpsc::{Receiver, Sender};

/// Forwards run events to the UI thread.
pub struct ChannelObserver {
    events: Sender<UiEvent>,
}

impl ChannelObserver {
    pub fn new(events: Sender<UiEvent>) -> Self {
        ChannelObserver { events }
    }
}

impl Observer for ChannelObserver {
    fn step_started(&mut self, step: &Step) {
        let _ = self.events.send(UiEvent::StepStarted(step.name.clone()));
    }

    fn step_finished(&mut self, step: &Step, report: &StepReport) {
        let _ = self.events.send(UiEvent::StepFinished {
            name: step.name.clone(),
            outcome: report.outcome,
            output: report.output.clone(),
        });
    }
}

/// At a checkpoint, tells the UI and blocks until the user's decision arrives.
pub struct ChannelGate {
    events: Sender<UiEvent>,
    decisions: Receiver<Control>,
}

impl ChannelGate {
    pub fn new(events: Sender<UiEvent>, decisions: Receiver<Control>) -> Self {
        ChannelGate { events, decisions }
    }
}

impl Gate for ChannelGate {
    fn checkpoint(&mut self, step: &Step, report: &StepReport) -> Control {
        let _ = self.events.send(UiEvent::Checkpoint {
            step: step.name.clone(),
            prompt: report.prompt.clone(),
            output: report.output.clone(),
        });
        // If the UI is gone, abort rather than hang.
        self.decisions.recv().unwrap_or(Control::Abort)
    }
}
