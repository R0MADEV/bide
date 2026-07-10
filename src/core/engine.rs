use super::state::{Status, StepOutcome};
use super::task::Task;
use super::workflow::{Step, Workflow};

/// Executes the work for a single step and reports its outcome. The engine owns
/// the flow; the runner only does what bide asks, one step at a time.
pub trait StepRunner {
    fn run(&mut self, step: &Step) -> StepOutcome;
}

pub fn run<R: StepRunner>(workflow: &Workflow, runner: &mut R) -> Status {
    run_from(workflow, runner, &mut Task::new())
}

/// Drives the workflow from the given task's position. `run` starts fresh; a
/// resumed run passes a task seeded at the step where a previous run stopped.
pub fn run_from<R: StepRunner>(workflow: &Workflow, runner: &mut R, task: &mut Task) -> Status {
    loop {
        if task.cursor() >= workflow.steps.len() {
            return Status::Accepted;
        }
        let step = &workflow.steps[task.cursor()];
        let outcome = runner.run(step);
        let status = task.advance(workflow, outcome);
        if status != Status::Running {
            return status;
        }
    }
}
