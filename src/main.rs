use bide::agents::{
    AgentRequest, AgentResponse, AgentRunner, AgentStep, AnthropicAgent, ClaudeCodeAgent,
    OpenAiAgent, Verdict,
};
use bide::cli::{parse, Command};
use bide::config::{AgentSettings, Provider};
use bide::context::{build_context, CodeContext, ContextPack, LexisAsk};
use bide::dispatch::{Dispatcher, StepHandler};
use bide::report::{save, RunRecord};
use bide::tools::{Approver, CommandStep, ProcessShell};
use bide::{run, Status, Step, Workflow};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_PATH: &str = "bide.toml";
const RUNS_DIR: &str = ".bide/runs";

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
    let agent = match resolve_agent() {
        Ok(agent) => agent,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };

    println!("bide run: {task}");
    println!("agent: {}\n", agent.label());
    let context = context_pack(task);
    println!("context:\n{}\n", context.text);
    print_plan(&workflow);

    let mut dispatcher = build_dispatcher(&workflow, task, &context.text, &agent);
    let status = run(&workflow, &mut dispatcher);

    println!("\nfinished: {status:?}");
    let record = RunRecord {
        task: task.to_string(),
        steps: dispatcher.into_records(),
        status,
    };
    record_run(&record);

    match status {
        Status::Accepted => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

fn record_run(record: &RunRecord) {
    match save(record, Path::new(RUNS_DIR), &run_id()) {
        Ok(path) => println!("report: {}", path.display()),
        Err(error) => eprintln!("warning: could not write run report: {error}"),
    }
}

fn run_id() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    format!("run-{seconds}")
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

fn build_dispatcher(
    workflow: &Workflow,
    task: &str,
    context: &str,
    agent: &AgentKind,
) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    for step in &workflow.steps {
        dispatcher.register(&step.name, handler_for(step, task, context, agent));
    }
    dispatcher
}

fn handler_for(step: &Step, task: &str, context: &str, agent: &AgentKind) -> Box<dyn StepHandler> {
    let Some(command) = &step.command else {
        let input = format!("{task}\n\nRepository context:\n{context}");
        return Box::new(AgentStep::new(&step.name, &input, agent.build()));
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

/// Which agent backend reasons for this run.
enum AgentKind {
    Stub,
    ClaudeCli,
    Api { settings: AgentSettings, api_key: String },
}

impl AgentKind {
    fn build(&self) -> Box<dyn AgentRunner> {
        match self {
            AgentKind::Stub => Box::new(StubAgent),
            AgentKind::ClaudeCli => Box::new(ClaudeCodeAgent::with_cli()),
            AgentKind::Api { settings, api_key } => match settings.provider {
                Provider::OpenAi => {
                    Box::new(OpenAiAgent::new(api_key.clone(), settings.model.clone()))
                }
                Provider::Anthropic => {
                    Box::new(AnthropicAgent::new(api_key.clone(), settings.model.clone()))
                }
            },
        }
    }

    fn label(&self) -> String {
        match self {
            AgentKind::Stub => "stub".to_string(),
            AgentKind::ClaudeCli => "claude (cli)".to_string(),
            AgentKind::Api { settings, .. } => format!("{:?} {}", settings.provider, settings.model),
        }
    }
}

/// Resolve the agent: an explicit env override wins, then the [agent] section of
/// bide.toml, otherwise the stub. The API key is read from the named env var.
fn resolve_agent() -> Result<AgentKind, String> {
    match std::env::var("BIDE_AGENT").as_deref() {
        Ok("claude") => return Ok(AgentKind::ClaudeCli),
        Ok("stub") => return Ok(AgentKind::Stub),
        _ => {}
    }

    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Ok(AgentKind::Stub);
    }
    let settings = bide::config::load_agent(path).map_err(|error| format!("invalid {CONFIG_PATH}: {error}"))?;
    let Some(settings) = settings else {
        return Ok(AgentKind::Stub);
    };
    let api_key = read_key(&settings.api_key_env)?;
    Ok(AgentKind::Api { settings, api_key })
}

fn read_key(var: &str) -> Result<String, String> {
    match std::env::var(var) {
        Ok(key) if !key.is_empty() => Ok(key),
        _ => Err(format!(
            "environment variable {var} is not set (required by the configured agent)"
        )),
    }
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
