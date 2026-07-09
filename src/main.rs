use bide::agents::{
    AgentRequest, AgentResponse, AgentRunner, AgentStep, AnthropicAgent, ClaudeCodeAgent,
    OpenAiAgent, Verdict,
};
use bide::cli::{parse, Command};
use bide::config::{AgentSettings, Provider};
use bide::doctor::{config_check, is_healthy, tool_check, ConfigState, Level};
use bide::context::{build_context, CodeContext, ContextPack, LexisAsk};
use bide::dispatch::{Dispatcher, StepHandler};
use bide::git::{branch_name, commit_message, Git, GitCli};
use bide::report::{save, RunRecord};
use bide::tools::{Approver, ClaudeCodeImplementer, CommandStep, ImplementStep, ProcessShell};
use bide::{run, Status, Step, Workflow};
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command as Process, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_PATH: &str = "bide.toml";
const RUNS_DIR: &str = ".bide/runs";
const IMPLEMENT_STEP: &str = "implement";

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
        Command::Doctor => doctor(),
    }
}

fn doctor() -> ExitCode {
    let checks = vec![
        tool_check("git", tool_present("git"), true),
        tool_check("claude", tool_present("claude"), false),
        tool_check("lexis", tool_present("lexis"), false),
        config_check(config_state()),
    ];

    for check in &checks {
        let mark = match check.level {
            Level::Ok => "ok  ",
            Level::Warn => "warn",
            Level::Fail => "FAIL",
        };
        println!("[{mark}] {} — {}", check.name, check.detail);
    }

    if is_healthy(&checks) {
        return ExitCode::SUCCESS;
    }
    ExitCode::from(1)
}

fn tool_present(name: &str) -> bool {
    Process::new(name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn config_state() -> ConfigState {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return ConfigState::Missing;
    }
    match bide::config::load(path) {
        Ok(_) => ConfigState::Valid,
        Err(error) => ConfigState::Invalid(error.to_string()),
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
    println!("agent: {}", agent.label());
    println!("git: {}\n", git_state());
    let clean_at_start = GitCli.status().clean;
    let context = context_pack(task);
    println!("context:\n{}\n", context.text);
    print_plan(&workflow);

    let mut dispatcher = build_dispatcher(&workflow, task, &context.text, &agent);
    let status = run(&workflow, &mut dispatcher);

    println!("\nfinished: {status:?}");
    let diff = GitCli.diff();
    finalize_branch(task, clean_at_start, &diff);
    let record = RunRecord {
        task: task.to_string(),
        steps: dispatcher.into_records(),
        status,
        diff,
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

/// After a run, move the changes it produced onto a task branch when opted in
/// (BIDE_BRANCH=1). Only when the tree was clean at the start (so the changes
/// are the run's) and the run actually produced a diff — no empty branches.
fn finalize_branch(task: &str, clean_at_start: bool, diff: &str) {
    let opted_in = matches!(std::env::var("BIDE_BRANCH").as_deref(), Ok("1"));
    if !opted_in {
        return;
    }
    if !clean_at_start {
        println!("branch: skipped (working tree was not clean at start)");
        return;
    }
    if diff.trim().is_empty() {
        println!("branch: skipped (run produced no changes)");
        return;
    }
    let name = branch_name(task);
    let mut git = GitCli;
    if !git.create_branch(&name) {
        println!("branch: could not create {name}");
        return;
    }
    if git.commit_all(&commit_message(task)) {
        println!("branch: created {name} and committed the run's changes");
        return;
    }
    println!("branch: created {name} (nothing committed)");
}

fn git_state() -> String {
    let mut git = GitCli;
    let Some(branch) = git.current_branch() else {
        return "not a git repository".to_string();
    };
    let status = git.status();
    if status.clean {
        return format!("{branch} (clean)");
    }
    format!("{branch} ({} changed)", status.changed_files.len())
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
    if let Some(command) = &step.command {
        return Box::new(CommandStep::new(
            command,
            Box::new(ProcessShell),
            Box::new(PromptApprover),
        ));
    }
    if is_implement_step(step, agent) {
        return Box::new(ImplementStep::new(task, Box::new(ClaudeCodeImplementer)));
    }
    let input = format!("{task}\n\nRepository context:\n{context}");
    Box::new(AgentStep::new(&step.name, &input, agent.build()))
}

/// The implement step edits the repo through Claude Code — only when real agents
/// are opted in, so a stub run never tries to change files.
fn is_implement_step(step: &Step, agent: &AgentKind) -> bool {
    step.name == IMPLEMENT_STEP && !agent.is_stub()
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

    fn is_stub(&self) -> bool {
        matches!(self, AgentKind::Stub)
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
