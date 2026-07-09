use bide::cli::{parse, Command};
use bide::{run, Status, Step, StepOutcome, StepRunner, Workflow};
use std::path::Path;
use std::process::ExitCode;

const CONFIG_PATH: &str = "bide.toml";

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
    let workflow = match resolve_workflow() {
        Ok(workflow) => workflow,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };

    println!("bide run: {task}\n");
    let status = run(&workflow, &mut StubRunner);

    println!("\nfinished: {status:?}");
    match status {
        Status::Accepted => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// Load the recipe from bide.toml when present, otherwise use the default. A
/// present-but-invalid config is a hard error, not a silent fallback.
fn resolve_workflow() -> Result<Workflow, String> {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Ok(Workflow::default_recipe());
    }
    bide::config::load(path).map_err(|error| format!("invalid {CONFIG_PATH}: {error}"))
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
