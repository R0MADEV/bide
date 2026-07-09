use bide::cli::{parse, Command};
use bide::{run, Status, Step, StepOutcome, StepRunner, Workflow};
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = match parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!("usage: bide run \"<task description>\"");
            return ExitCode::from(2);
        }
    };

    match command {
        Command::Run { task } => run_task(&task),
    }
}

fn run_task(task: &str) -> ExitCode {
    println!("bide run: {task}\n");

    let workflow = Workflow::default_recipe();
    let status = run(&workflow, &mut StubRunner);

    println!("\nfinished: {status:?}");
    match status {
        Status::Accepted => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// Placeholder runner: reports every step as done so the end-to-end loop is
/// observable. Real runners (context, agents, tools) replace it later.
struct StubRunner;

impl StepRunner for StubRunner {
    fn run(&mut self, step: &Step) -> StepOutcome {
        println!("  · {}", step.name);
        StepOutcome::Success
    }
}
