use bide::cli::{parse, Command};
use bide::dispatch::{Dispatcher, StepHandler};
use bide::tools::{Approver, CommandStep, ProcessShell};
use bide::{run, Status, Step, StepOutcome, Workflow};
use std::io::{self, Write};
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
    print_plan(&workflow);

    let mut dispatcher = build_dispatcher(&workflow);
    let status = run(&workflow, &mut dispatcher);

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

fn print_plan(workflow: &Workflow) {
    println!("recipe ({} steps):", workflow.steps.len());
    for step in &workflow.steps {
        match &step.command {
            Some(command) => println!("  · {} $ {command}", step.name),
            None => println!("  · {} (no command yet)", step.name),
        }
    }
    println!();
}

fn build_dispatcher(workflow: &Workflow) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    for step in &workflow.steps {
        dispatcher.register(&step.name, handler_for(step));
    }
    dispatcher
}

fn handler_for(step: &Step) -> Box<dyn StepHandler> {
    let Some(command) = &step.command else {
        return Box::new(Placeholder);
    };
    Box::new(CommandStep::new(
        command,
        Box::new(ProcessShell),
        Box::new(PromptApprover),
    ))
}

/// Stand-in for steps without a command (agent/context steps). Succeeds so the
/// walking skeleton runs end to end until real handlers exist.
struct Placeholder;

impl StepHandler for Placeholder {
    fn handle(&mut self, _step: &Step) -> StepOutcome {
        StepOutcome::Success
    }
}

/// Confirms a policy-flagged command on the terminal.
struct PromptApprover;

impl Approver for PromptApprover {
    fn approve(&mut self, reason: &str, command: &str) -> bool {
        print!("bide: {reason}\n      run `{command}`? [y/N] ");
        let _ = io::stdout().flush();

        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(answer.trim(), "y" | "Y" | "yes")
    }
}
