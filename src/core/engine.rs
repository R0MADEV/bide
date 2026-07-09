use super::state::{Status, StepOutcome};
use super::task::Task;
use super::workflow::{Step, Workflow};

/// Executes the work for a single step and reports its outcome. The engine owns
/// the flow; the runner only does what bide asks, one step at a time.
pub trait StepRunner {
    fn run(&mut self, step: &Step) -> StepOutcome;
}

pub fn run<R: StepRunner>(workflow: &Workflow, runner: &mut R) -> Status {
    if workflow.steps.is_empty() {
        return Status::Accepted;
    }

    let mut task = Task::new();
    loop {
        let step = &workflow.steps[task.cursor()];
        let outcome = runner.run(step);
        let status = task.advance(workflow, outcome);
        if status != Status::Running {
            return status;
        }
    }
}
