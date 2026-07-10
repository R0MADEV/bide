use bide::agents::{
    AgentRequest, AgentResponse, AgentRunner, AgentStep, AnthropicAgent, ClaudeCodeAgent,
    OpenAiAgent, Verdict,
};
use bide::cli::{parse, Command, RunOptions};
use bide::config::{AgentSettings, Provider, ToolSettings};
use bide::doctor::{config_check, is_healthy, tool_check, ConfigState, Level};
use bide::context::{build_context, ClaudeContext, CodeContext, ContextPack, LexisAsk};
use bide::dispatch::{AutoGate, Control, Dispatcher, Gate, Observer, StepHandler, StepReport};
use bide::tui::{App, ChannelGate, ChannelObserver, Key as TuiKey, StepStatus, UiEvent};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use bide::git::{branch_name, commit_message, pr_title, Git, GitCli};
use bide::policy::Policy;
use bide::report::{save, RunRecord};
use bide::tools::{Approver, ClaudeCodeImplementer, CommandStep, ImplementStep, ProcessShell};
use bide::{run_from, Status, Step, StepOutcome, Task, Workflow};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as Process, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        Command::Run(options) => run_task(&options),
        Command::Tui(options) => tui_command(&options),
        Command::Doctor => doctor(),
        Command::Help => help(),
    }
}

/// Runs the workflow in a worker thread and drives a terminal UI: the steps with
/// live status, and a checkpoint panel where you continue, re-plan with feedback
/// or abort. The engine talks to the UI only through the Observer/Gate ports.
fn tui_command(options: &RunOptions) -> ExitCode {
    let workflow = match resolve_workflow() {
        Ok(workflow) => workflow,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };
    let tools = tools_from_config();
    let agent = match resolve_agent(options.agent.as_deref(), &tools.claude) {
        Ok(agent) => agent,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };
    let policy = policy_from_config();
    let context = context_pack(&options.task, context_choice(options).as_deref(), &tools);
    let step_names: Vec<String> = workflow.steps.iter().map(|s| s.name.clone()).collect();

    let (events_tx, events_rx) = mpsc::channel::<UiEvent>();
    let (decisions_tx, decisions_rx) = mpsc::channel::<Control>();

    let task = options.task.clone();
    let claude = tools.claude.clone();
    let context_text = context.text;
    let worker = thread::spawn(move || {
        let mut dispatcher =
            build_dispatcher(&workflow, &task, &context_text, &agent, &policy, &claude);
        dispatcher.set_observer(Box::new(ChannelObserver::new(events_tx.clone())));
        dispatcher.set_gate(Box::new(ChannelGate::new(events_tx.clone(), decisions_rx)));
        let mut state = Task::new();
        let status = run_from(&workflow, &mut dispatcher, &mut state);
        let _ = events_tx.send(UiEvent::Finished(status));
    });

    let exit = run_ui(step_names, &events_rx, &decisions_tx);
    let _ = worker.join();
    exit
}

fn run_ui(step_names: Vec<String>, events: &Receiver<UiEvent>, decisions: &Sender<Control>) -> ExitCode {
    let mut terminal = ratatui::init();
    let mut app = App::new(step_names);

    let final_status = loop {
        while let Ok(event) = events.try_recv() {
            app.apply(event);
        }
        let _ = terminal.draw(|frame| render(frame, &app));

        if !event::poll(Duration::from_millis(100)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if let Some(mapped) = map_key(key.code) {
            if let Some(control) = app.on_key(mapped) {
                let _ = decisions.send(control);
            }
        }
        let quit = matches!(key.code, KeyCode::Char('q') | KeyCode::Enter | KeyCode::Esc);
        if app.done.is_some() && quit {
            break app.done;
        }
    };

    ratatui::restore();
    match final_status {
        Some(Status::Accepted) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

fn map_key(code: KeyCode) -> Option<TuiKey> {
    match code {
        KeyCode::Enter => Some(TuiKey::Enter),
        KeyCode::Esc => Some(TuiKey::Esc),
        KeyCode::Backspace => Some(TuiKey::Backspace),
        KeyCode::Char(c) => Some(TuiKey::Char(c)),
        _ => None,
    }
}

fn render(frame: &mut Frame, app: &App) {
    let areas =
        Layout::vertical([Constraint::Percentage(45), Constraint::Min(0)]).split(frame.area());

    let items: Vec<ListItem> = app
        .steps
        .iter()
        .map(|step| {
            let mark = match &step.status {
                StepStatus::Pending => "·",
                StepStatus::Running => "▶",
                StepStatus::Done(StepOutcome::Success) => "✓",
                StepStatus::Done(_) => "✗",
            };
            ListItem::new(format!(" {mark} {}", step.name))
        })
        .collect();
    frame.render_widget(List::new(items).block(Block::bordered().title(" bide ")), areas[0]);

    let panel = match (&app.checkpoint, app.done) {
        (Some(checkpoint), _) => Paragraph::new(format!(
            "SENT TO AI:\n{}\n\nRESPONSE:\n{}\n\n> feedback: {}\n\n[Enter] continue (or re-plan with feedback)   [Esc] abort",
            checkpoint.prompt.trim(),
            checkpoint.output.trim(),
            app.feedback
        ))
        .block(Block::bordered().title(format!(" checkpoint: {} ", checkpoint.step)))
        .wrap(Wrap { trim: false }),
        (None, Some(status)) => Paragraph::new(format!("finished: {status:?}\n\n[q] quit"))
            .block(Block::bordered().title(" done ")),
        (None, None) => {
            Paragraph::new("running…").block(Block::bordered().title(" status "))
        }
    };
    frame.render_widget(panel, areas[1]);
}

fn help() -> ExitCode {
    println!(
        "bide — a deterministic workflow engine.\n\n\
         Usage:\n  \
           bide run \"<task>\" [flags]   run in the terminal (line-based)\n  \
           bide tui \"<task>\" [flags]   run in an interactive terminal UI\n  \
           bide doctor\n  \
           bide help\n\n\
         Run flags (each also has a BIDE_* env var):\n  \
           --yes, -y           run straight through, no interactive checkpoints\n  \
           --branch            put the run's changes on a bide/<slug> branch\n  \
           --pr                push the branch and open a pull request\n  \
           --agent <name>      reasoning backend: claude | stub (else [agent] in bide.toml)\n  \
           --context <name>    context source: claude (Claude Code + lexis) | lexis\n  \
           --resume <id>       continue a previous run from where it stopped"
    );
    ExitCode::SUCCESS
}

fn opt_in(flag: bool, env: &str) -> bool {
    flag || matches!(std::env::var(env).as_deref(), Ok("1"))
}

fn doctor() -> ExitCode {
    let tools = tools_from_config();
    let checks = vec![
        tool_check("git", tool_present("git"), true),
        tool_check(&tools.claude, tool_present(&tools.claude), false),
        tool_check(&tools.lexis, tool_present(&tools.lexis), false),
        tool_check(&tools.gh, tool_present(&tools.gh), false),
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

fn run_task(options: &RunOptions) -> ExitCode {
    let (task_desc, id, mut work, preload) = match &options.resume {
        Some(id) => match load_state(id) {
            Ok(state) => (
                state.task,
                id.clone(),
                Task::resumed(state.cursor),
                Some(state.board),
            ),
            Err(message) => {
                eprintln!("error: {message}");
                return ExitCode::from(2);
            }
        },
        None => (options.task.clone(), run_id(), Task::new(), None),
    };
    let task = task_desc.as_str();

    let workflow = match resolve_workflow() {
        Ok(workflow) => workflow,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };
    let tools = tools_from_config();
    let agent = match resolve_agent(options.agent.as_deref(), &tools.claude) {
        Ok(agent) => agent,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };

    println!("bide run: {task}");
    if options.resume.is_some() {
        println!("resuming {id} from step {}", work.cursor());
    }
    println!("agent: {}", agent.label());
    println!("git: {}\n", git_state());
    let clean_at_start = GitCli.status().clean;
    let context = context_pack(task, context_choice(options).as_deref(), &tools);
    println!("context:\n{}\n", context.text);
    print_plan(&workflow);

    let policy = policy_from_config();
    let mut dispatcher =
        build_dispatcher(&workflow, task, &context.text, &agent, &policy, &tools.claude);
    if let Some(board) = &preload {
        dispatcher.preload_board(board);
    }
    dispatcher.set_gate(make_gate(opt_in(options.yes, "BIDE_YES")));
    dispatcher.set_observer(Box::new(PrintObserver));
    let status = run_from(&workflow, &mut dispatcher, &mut work);

    println!("\nfinished: {status:?}");
    let diff = GitCli.diff();
    let branch = finalize_branch(task, clean_at_start, &diff, opt_in(options.branch, "BIDE_BRANCH"));

    save_state(
        &id,
        &RunState {
            task: task.to_string(),
            cursor: work.cursor(),
            board: dispatcher.board_entries().to_vec(),
        },
    );

    let record = RunRecord {
        task: task.to_string(),
        steps: dispatcher.into_records(),
        status,
        diff,
    };
    let report_path = record_run(&record, &id);
    maybe_open_pr(
        branch.as_deref(),
        task,
        report_path.as_deref(),
        opt_in(options.pr, "BIDE_PR"),
        &tools.gh,
    );

    match status {
        Status::Accepted => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RunState {
    task: String,
    cursor: usize,
    board: Vec<(String, String)>,
}

fn state_path(id: &str) -> PathBuf {
    Path::new(RUNS_DIR).join(id).join("state.json")
}

fn save_state(id: &str, state: &RunState) {
    let path = state_path(id);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, json);
    }
}

fn load_state(id: &str) -> Result<RunState, String> {
    let path = state_path(id);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot resume {id}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("invalid state for {id}: {error}"))
}

fn context_choice(options: &RunOptions) -> Option<String> {
    options
        .context
        .clone()
        .or_else(|| std::env::var("BIDE_CONTEXT").ok())
}

fn record_run(record: &RunRecord, id: &str) -> Option<PathBuf> {
    match save(record, Path::new(RUNS_DIR), id) {
        Ok(path) => {
            println!("report: {}", path.display());
            Some(path)
        }
        Err(error) => {
            eprintln!("warning: could not write run report: {error}");
            None
        }
    }
}

/// After a run, move the changes it produced onto a task branch when opted in
/// (BIDE_BRANCH=1). Only when the tree was clean at the start (so the changes
/// are the run's) and the run actually produced a diff — no empty branches.
fn finalize_branch(
    task: &str,
    clean_at_start: bool,
    diff: &str,
    opted_in: bool,
) -> Option<String> {
    if !opted_in {
        return None;
    }
    if !clean_at_start {
        println!("branch: skipped (working tree was not clean at start)");
        return None;
    }
    if diff.trim().is_empty() {
        println!("branch: skipped (run produced no changes)");
        return None;
    }
    let name = branch_name(task);
    let mut git = GitCli;
    if !git.create_branch(&name) {
        println!("branch: could not create {name}");
        return None;
    }
    if !git.commit_all(&commit_message(task)) {
        println!("branch: created {name} (nothing committed)");
        return None;
    }
    println!("branch: created {name} and committed the run's changes");
    Some(name)
}

/// Push the task branch and open a PR when opted in (BIDE_PR=1). Uses the run
/// report as the PR body.
fn maybe_open_pr(
    branch: Option<&str>,
    task: &str,
    report: Option<&Path>,
    opted_in: bool,
    gh: &str,
) {
    if !opted_in {
        return;
    }
    let Some(branch) = branch else {
        println!("pr: skipped (no committed branch)");
        return;
    };
    if !GitCli.push(branch) {
        println!("pr: could not push {branch}");
        return;
    }
    if open_pr(&pr_title(task), report, gh) {
        println!("pr: opened for {branch}");
        return;
    }
    println!("pr: could not open PR (is gh installed and authenticated?)");
}

fn open_pr(title: &str, report: Option<&Path>, gh: &str) -> bool {
    let mut command = Process::new(gh);
    command.arg("pr").arg("create").arg("--title").arg(title);
    match report {
        Some(path) => command.arg("--body-file").arg(path),
        None => command.arg("--body").arg("Opened by bide."),
    };
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
        return Ok(default_workflow());
    }
    bide::config::load(path).map_err(|error| format!("invalid {CONFIG_PATH}: {error}"))
}

/// The default recipe, with its verify step wired to the detected project test
/// command (cargo test, npm test, ...) so `bide run` does something real even
/// without a bide.toml.
fn default_workflow() -> Workflow {
    let mut workflow = Workflow::default_recipe();
    let Some(command) = bide::detect::verify_command(Path::new(".")) else {
        return workflow;
    };
    for step in &mut workflow.steps {
        if step.name == "verify" {
            step.command = Some(command.clone());
        }
    }
    workflow
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
    policy: &Policy,
    claude: &str,
) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    for step in &workflow.steps {
        let handler = handler_for(step, task, context, agent, policy, claude);
        dispatcher.register(&step.name, handler);
    }
    dispatcher
}

fn handler_for(
    step: &Step,
    task: &str,
    context: &str,
    agent: &AgentKind,
    policy: &Policy,
    claude: &str,
) -> Box<dyn StepHandler> {
    if let Some(command) = &step.command {
        return Box::new(CommandStep::new(
            command,
            Box::new(ProcessShell),
            Box::new(PromptApprover),
            policy.clone(),
        ));
    }
    if is_implement_step(step, agent) {
        return Box::new(ImplementStep::new(
            task,
            Box::new(ClaudeCodeImplementer::new(claude)),
        ));
    }
    let input = format!("{task}\n\nRepository context:\n{context}");
    Box::new(AgentStep::new(&step.name, &input, agent.build()))
}

/// The implement step edits the repo through Claude Code — only when real agents
/// are opted in, so a stub run never tries to change files.
fn is_implement_step(step: &Step, agent: &AgentKind) -> bool {
    step.name == IMPLEMENT_STEP && !agent.is_stub()
}

/// Built-in policy plus any extra rules from the [policy] section of bide.toml.
fn policy_from_config() -> Policy {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Policy::default();
    }
    match bide::config::load_policy(path) {
        Ok(settings) => Policy::with_rules(settings.deny_commands, settings.secret_paths),
        Err(_) => Policy::default(),
    }
}

fn context_pack(task: &str, choice: Option<&str>, tools: &ToolSettings) -> ContextPack {
    let mut provider = context_provider(choice, tools);
    build_context(provider.as_mut(), task)
}

/// Pick the context source. `claude` runs Claude Code with the lexis tools to
/// fetch real code; `lexis` runs `lexis ask`; anything else gives no context.
fn context_provider(choice: Option<&str>, tools: &ToolSettings) -> Box<dyn CodeContext> {
    match choice {
        Some("claude") => Box::new(ClaudeContext::new(&tools.claude)),
        Some("lexis") => {
            let cwd = std::env::current_dir().unwrap_or_default();
            Box::new(LexisAsk::new(cwd, &tools.lexis))
        }
        _ => Box::new(NoContext),
    }
}

/// Tool binaries from the [tools] section of bide.toml, defaulting to PATH names.
fn tools_from_config() -> ToolSettings {
    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return ToolSettings::default();
    }
    bide::config::load_tools(path).unwrap_or_default()
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
    ClaudeCli(String),
    Api { settings: AgentSettings, api_key: String },
}

impl AgentKind {
    fn build(&self) -> Box<dyn AgentRunner> {
        match self {
            AgentKind::Stub => Box::new(StubAgent),
            AgentKind::ClaudeCli(program) => Box::new(ClaudeCodeAgent::with_cli(program)),
            AgentKind::Api { settings, api_key } => match settings.provider {
                Provider::OpenAi => Box::new(OpenAiAgent::new(
                    api_key.clone(),
                    settings.model.clone(),
                    settings.max_tokens,
                )),
                Provider::Anthropic => Box::new(AnthropicAgent::new(
                    api_key.clone(),
                    settings.model.clone(),
                    settings.max_tokens,
                )),
            },
        }
    }

    fn is_stub(&self) -> bool {
        matches!(self, AgentKind::Stub)
    }

    fn label(&self) -> String {
        match self {
            AgentKind::Stub => "stub".to_string(),
            AgentKind::ClaudeCli(_) => "claude (cli)".to_string(),
            AgentKind::Api { settings, .. } => format!("{:?} {}", settings.provider, settings.model),
        }
    }
}

/// Resolve the agent: an explicit env override wins, then the [agent] section of
/// bide.toml, otherwise the stub. The API key is read from the named env var.
fn resolve_agent(flag: Option<&str>, claude: &str) -> Result<AgentKind, String> {
    let choice = flag
        .map(str::to_string)
        .or_else(|| std::env::var("BIDE_AGENT").ok());
    match choice.as_deref() {
        Some("claude") => return Ok(AgentKind::ClaudeCli(claude.to_string())),
        Some("stub") => return Ok(AgentKind::Stub),
        Some(other) => return Err(format!("unknown agent: {other} (use claude or stub)")),
        None => {}
    }

    let path = Path::new(CONFIG_PATH);
    if !path.exists() {
        return Ok(AgentKind::Stub);
    }
    let settings = bide::config::load_agent(path).map_err(|error| format!("invalid {CONFIG_PATH}: {error}"))?;
    let Some(settings) = settings else {
        return Ok(AgentKind::Stub);
    };
    let api_key = resolve_key(&settings)?;
    Ok(AgentKind::Api { settings, api_key })
}

/// Prefer a direct api_key; otherwise read it from the named env var.
fn resolve_key(settings: &AgentSettings) -> Result<String, String> {
    if let Some(key) = &settings.api_key {
        if !key.trim().is_empty() {
            return Ok(key.clone());
        }
    }
    if let Some(var) = &settings.api_key_env {
        return read_key(var);
    }
    Err("[agent] needs api_key or api_key_env".to_string())
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

/// Prints live progress as each step runs, so long steps (an agent thinking,
/// tests running) do not look frozen.
struct PrintObserver;

impl Observer for PrintObserver {
    fn step_started(&mut self, step: &Step) {
        print!("→ {} … ", step.name);
        let _ = io::stdout().flush();
    }

    fn step_finished(&mut self, _step: &Step, outcome: StepOutcome) {
        println!("{outcome:?}");
    }
}

/// Interactive by default; `--yes` or BIDE_YES=1 runs straight through.
fn make_gate(auto: bool) -> Box<dyn Gate> {
    if auto {
        return Box::new(AutoGate);
    }
    Box::new(PromptGate)
}

/// Stops at a checkpoint step, shows what it produced, and lets the user
/// continue, retry the step, or abort the run.
struct PromptGate;

impl Gate for PromptGate {
    fn checkpoint(&mut self, step: &Step, report: &StepReport) -> Control {
        println!("\n── checkpoint: {} [{:?}] ──", step.name, report.outcome);
        println!("{}", report.output.trim());
        print!("continue / retry / abort? [C/r/a] ");
        let _ = io::stdout().flush();

        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_err() {
            return Control::Abort;
        }
        match answer.trim() {
            "r" | "retry" => Control::Retry(prompt_feedback()),
            "a" | "abort" => Control::Abort,
            _ => Control::Continue,
        }
    }
}

fn prompt_feedback() -> String {
    print!("      feedback (optional, enter to skip): ");
    let _ = io::stdout().flush();
    let mut feedback = String::new();
    if io::stdin().read_line(&mut feedback).is_err() {
        return String::new();
    }
    feedback.trim().to_string()
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
