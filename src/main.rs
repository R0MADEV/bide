use bide::agents::{
    AgentRequest, AgentResponse, AgentRunner, AgentStep, ClaudeCodeAgent, Verdict,
};
use bide::cli::{parse, Command};
use bide::context::{build_context, CodeContext, ContextPack, LexisAsk};
use bide::dispatch::{Dispatcher, StepHandler};
use bide::tools::{Approver, CommandStep, ProcessShell};
use bide::{run, Status, Step, Workflow};
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
    let context = context_pack(task);
    println!("context:\n{}\n", context.text);
    print_plan(&workflow);

    let mut dispatcher = build_dispatcher(&workflow, task, &context.text);
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

fn build_dispatcher(workflow: &Workflow, task: &str, context: &str) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    for step in &workflow.steps {
        dispatcher.register(&step.name, handler_for(step, task, context));
    }
    dispatcher
}

fn handler_for(step: &Step, task: &str, context: &str) -> Box<dyn StepHandler> {
    let Some(command) = &step.command else {
        let input = format!("{task}\n\nRepository context:\n{context}");
        return Box::new(AgentStep::new(&step.name, &input, make_agent()));
    };
    Box::new(CommandStep::new(
        command,
        Box::new(ProcessShell),
        Box::new(PromptApprover),
    ))
}

fn context_pack(task: &str) -> ContextPack {
    let mut provider = context_provider();
    build_context(provider.as_mut(), task)
}

/// Use Lexis for context when opted in, otherwise none. Keeps `bide run` working
/// without Lexis installed.
fn context_provider() -> Box<dyn CodeContext> {
    let use_lexis = matches!(std::env::var("BIDE_CONTEXT").as_deref(), Ok("lexis"));
    if use_lexis {
        let cwd = std::env::current_dir().unwrap_or_default();
        return Box::new(LexisAsk::new(cwd));
    }
    Box::new(NoContext)
}

struct NoContext;

impl CodeContext for NoContext {
    fn lookup(&mut self, _task: &str) -> String {
        String::new()
    }
}

/// Use the real Claude Code driver when opted in, otherwise the stub. The
/// default keeps `bide run` working without `claude` installed.
fn make_agent() -> Box<dyn AgentRunner> {
    let use_claude = matches!(std::env::var("BIDE_AGENT").as_deref(), Ok("claude"));
    if use_claude {
        return Box::new(ClaudeCodeAgent::with_cli());
    }
    Box::new(StubAgent)
}

/// Stand-in agent until the Claude Code driver exists: it proceeds without doing
/// real reasoning so the workflow runs end to end.
struct StubAgent;

impl AgentRunner for StubAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        AgentResponse {
            output: format!("(stub {} for: {})", request.role, request.input),
            verdict: Verdict::Proceed,
        }
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
