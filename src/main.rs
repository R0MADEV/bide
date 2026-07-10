use bide::agents::{
    AgentRequest, AgentResponse, AgentRunner, AgentStep, AnthropicAgent, ClaudeCodeAgent,
    OpenAiAgent, Verdict,
};
use bide::cli::{parse, Command, RunOptions};
use bide::config::{AgentSettings, Provider, ToolSettings};
use bide::doctor::{config_check, is_healthy, tool_check, ConfigState, Level};
use bide::context::{build_context, ClaudeContext, CodeContext, ContextPack, LexisAsk};
use bide::dispatch::{AutoGate, Control, Dispatcher, Gate, Observer, StepHandler, StepReport};
use bide::tui::{
    App, ChannelGate, ChannelObserver, Key as TuiKey, Mode, Reaction, StepStatus, UiEvent,
};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use bide::git::{branch_name, commit_message, pr_title, Git, GitCli};
use bide::policy::Policy;
use bide::report::{save, worth_saving, RunRecord};
use bide::tools::{Approver, ClaudeCodeImplementer, CommandStep, ImplementStep, ProcessShell};
use bide::{run_from, Status, Step, StepOutcome, Task, Workflow};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as Process, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use bide::route::Turn;

const CONFIG_PATH: &str = "bide.toml";
const RUNS_DIR: &str = ".bide/runs";
const IMPLEMENT_STEP: &str = "implement";
const KEEP_RUNS: usize = 20;
/// Braille frames for the "working…" spinner, one per redraw tick.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

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
        Command::Tui(options) => {
            let start = options.task.clone();
            repl(&options, Some(start))
        }
        Command::Repl => repl(&RunOptions::default(), None),
        Command::Init => init(),
        Command::Doctor => doctor(),
        Command::Help => help(),
    }
}

/// A run in flight: the events it emits and the channel to send it decisions.
struct ActiveRun {
    events: Receiver<UiEvent>,
    decisions: Sender<Control>,
    handle: thread::JoinHandle<()>,
}

/// The interactive workspace: type a task to run the workflow, or `?question` to
/// ask Claude+lexis about the code. Runs happen in a worker thread; the UI only
/// observes and decides through the ports.
fn repl(options: &RunOptions, autostart: Option<String>) -> ExitCode {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    let mut active: Option<ActiveRun> = None;
    // Past exchanges, so a follow-up question carries context.
    let mut history: Vec<Turn> = Vec::new();
    // The input awaiting an answer, and when it started (for the elapsed timer).
    let mut pending: Option<(String, Instant)> = None;
    let mut tick: usize = 0;

    if let Some(task) = autostart {
        if !task.trim().is_empty() {
            pending = Some((task.clone(), Instant::now()));
            active = Some(spawn_route(&task, options, &history, &mut app));
        }
    }

    loop {
        if let Some(run) = &active {
            while let Ok(event) = run.events.try_recv() {
                app.apply(event);
            }
            if app.mode == Mode::Input {
                if let Some(run) = active.take() {
                    let _ = run.handle.join();
                }
                remember(&mut history, &mut pending, &app);
            }
        }
        tick = tick.wrapping_add(1);
        let elapsed = pending.as_ref().map(|(_, started)| started.elapsed());
        let _ = terminal.draw(|frame| render(frame, &app, tick, elapsed));

        if !event::poll(Duration::from_millis(80)).unwrap_or(false) {
            continue;
        }
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(mapped) = map_key(key.code) else {
            continue;
        };
        match app.on_key(mapped) {
            Reaction::Quit => break,
            Reaction::Submit(text) => {
                pending = Some((text.clone(), Instant::now()));
                active = Some(spawn_route(&text, options, &history, &mut app));
            }
            Reaction::Decide(control) => {
                if let Some(run) = &active {
                    let _ = run.decisions.send(control);
                }
            }
            Reaction::None => {}
        }
    }

    ratatui::restore();
    ExitCode::SUCCESS
}

/// When a run ends, record it as a turn if it produced an answer (a question),
/// so the next follow-up has context. Tasks produce no answer and are skipped.
fn remember(history: &mut Vec<Turn>, pending: &mut Option<(String, Instant)>, app: &App) {
    let Some((question, _)) = pending.take() else {
        return;
    };
    let Some(answer) = &app.answer else {
        return;
    };
    history.push(Turn {
        question,
        answer: answer.clone(),
    });
}

/// Spawn a worker that first lets the AI decide whether the input is a QUESTION
/// (answer it with Claude+lexis) or a TASK (run the workflow) — no `?` needed.
fn spawn_route(input: &str, options: &RunOptions, history: &[Turn], app: &mut App) -> ActiveRun {
    app.start_question();

    let (events_tx, events) = mpsc::channel::<UiEvent>();
    let (decisions, decisions_rx) = mpsc::channel::<Control>();
    let input = input.to_string();
    let history = history.to_vec();
    let agent_flag = options.agent.clone();
    let context_flag = context_choice(options);
    let handle = thread::spawn(move || {
        let tools = tools_from_config();
        // An obvious task skips the AI classifier and runs straight away.
        if matches!(bide::route::guess(&input), Some(bide::route::Guess::Task)) {
            run_workflow_worker(
                &input,
                agent_flag.as_deref(),
                context_flag.as_deref(),
                &tools,
                &events_tx,
                decisions_rx,
            );
            return;
        }
        // Otherwise ask Claude+lexis: it answers a question, or replies TASK.
        let reply = bide::context::ask_claude(&tools.claude, &bide::route::route_prompt(&history, &input));
        let is_task = reply.trim().eq_ignore_ascii_case("TASK") || reply.trim().starts_with("TASK");
        if is_task {
            run_workflow_worker(
                &input,
                agent_flag.as_deref(),
                context_flag.as_deref(),
                &tools,
                &events_tx,
                decisions_rx,
            );
            return;
        }
        let answer = if reply.trim().is_empty() {
            "(no answer — is claude available?)".to_string()
        } else {
            reply
        };
        let _ = events_tx.send(UiEvent::Answer(answer));
        let _ = events_tx.send(UiEvent::Finished(Status::Accepted));
    });

    ActiveRun {
        events,
        decisions,
        handle,
    }
}

/// Resolve config and run the workflow inside a worker, streaming UI events and
/// persisting the run. Errors are surfaced as an answer.
fn run_workflow_worker(
    task: &str,
    agent_flag: Option<&str>,
    context_flag: Option<&str>,
    tools: &ToolSettings,
    events_tx: &Sender<UiEvent>,
    decisions_rx: Receiver<Control>,
) {
    let workflow = match resolve_workflow() {
        Ok(workflow) => workflow,
        Err(message) => return finish_with_error(events_tx, &message),
    };
    let agent = match resolve_agent(agent_flag, &tools.claude) {
        Ok(agent) => agent,
        Err(message) => return finish_with_error(events_tx, &message),
    };
    let policy = policy_from_config();
    let context = context_pack(task, context_flag, tools);
    let diff_before = GitCli.diff();

    let _ = events_tx.send(UiEvent::Steps(
        workflow.steps.iter().map(|s| s.name.clone()).collect(),
    ));

    let mut dispatcher =
        build_dispatcher(&workflow, task, &context.text, &agent, &policy, &tools.claude);
    dispatcher.set_observer(Box::new(ChannelObserver::new(events_tx.clone())));
    dispatcher.set_gate(Box::new(ChannelGate::new(events_tx.clone(), decisions_rx)));
    let mut state = Task::new();
    let status = run_from(&workflow, &mut dispatcher, &mut state);

    let diff = GitCli.diff();
    let changed = diff != diff_before;
    let board = dispatcher.board_entries().to_vec();
    let cursor = state.cursor();
    let record = RunRecord {
        task: task.to_string(),
        steps: dispatcher.into_records(),
        status,
        diff,
        context: context.text,
    };
    persist_run(&run_id(), &record, cursor, board, changed);
    let _ = events_tx.send(UiEvent::Finished(status));
}

fn finish_with_error(events_tx: &Sender<UiEvent>, message: &str) {
    let _ = events_tx.send(UiEvent::Answer(format!("error: {message}")));
    let _ = events_tx.send(UiEvent::Finished(Status::Failed));
}

fn map_key(code: KeyCode) -> Option<TuiKey> {
    match code {
        KeyCode::Enter => Some(TuiKey::Enter),
        KeyCode::Esc => Some(TuiKey::Esc),
        KeyCode::Backspace => Some(TuiKey::Backspace),
        KeyCode::Up => Some(TuiKey::Up),
        KeyCode::Down => Some(TuiKey::Down),
        KeyCode::Char(c) => Some(TuiKey::Char(c)),
        _ => None,
    }
}

fn render(frame: &mut Frame, app: &App, tick: usize, elapsed: Option<Duration>) {
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
    frame.render_widget(bottom_panel(app, tick, elapsed), areas[1]);
}

fn bottom_panel(app: &App, tick: usize, elapsed: Option<Duration>) -> Paragraph<'static> {
    if let Some(checkpoint) = &app.checkpoint {
        return Paragraph::new(format!(
            "SENT TO AI:\n{}\n\nRESPONSE:\n{}\n\n> feedback: {}\n\n[↑/↓] scroll   [Enter] continue (or re-plan with feedback)   [Esc] abort",
            checkpoint.prompt.trim(),
            checkpoint.output.trim(),
            app.feedback
        ))
        .block(Block::bordered().title(format!(" checkpoint: {} ", checkpoint.step)))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));
    }
    if app.mode == Mode::Running {
        let spin = SPINNER[tick % SPINNER.len()];
        let secs = elapsed.map(|e| e.as_secs()).unwrap_or(0);
        return Paragraph::new(format!("{spin} working… {secs}s"))
            .block(Block::bordered().title(" status "));
    }
    // Input mode: show the last answer/result and the prompt line.
    let mut body = String::new();
    if let Some(answer) = &app.answer {
        body.push_str(answer.trim());
        body.push_str("\n\n");
    } else if let Some(status) = app.done {
        body.push_str(&format!("finished: {status:?}\n\n"));
    }
    body.push_str(&format!(
        "> {}\n\n[↑/↓] scroll   [Enter] send — bide decides: question → answered, task → workflow   [Esc] quit",
        app.input
    ));
    Paragraph::new(body)
        .block(Block::bordered().title(" bide "))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0))
}

fn help() -> ExitCode {
    println!(
        "bide — a deterministic workflow engine.\n\n\
         Usage:\n  \
           bide                        interactive workspace (type tasks or ?questions)\n  \
           bide run \"<task>\" [flags]   run once in the terminal (line-based)\n  \
           bide tui \"<task>\" [flags]   run a task in the interactive UI\n  \
           bide init                   scaffold a starter bide.toml here\n  \
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

/// Scaffold a starter bide.toml in the current directory, refusing to overwrite
/// an existing one.
fn init() -> ExitCode {
    let path = Path::new(CONFIG_PATH);
    match bide::config::scaffold(path) {
        Ok(true) => {
            println!("created {CONFIG_PATH} — edit it, then run `bide run \"<task>\"`");
            ExitCode::SUCCESS
        }
        Ok(false) => {
            eprintln!("{CONFIG_PATH} already exists — not overwriting");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("error: could not write {CONFIG_PATH}: {error}");
            ExitCode::from(2)
        }
    }
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
    let diff_before = GitCli.diff();
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
    let changed = diff != diff_before;
    let board = dispatcher.board_entries().to_vec();
    let cursor = work.cursor();
    let record = RunRecord {
        task: task.to_string(),
        steps: dispatcher.into_records(),
        status,
        diff,
        context: context.text,
    };
    let report_path = persist_run(&id, &record, cursor, board, changed);
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

/// Persist a run only when it is worth keeping (it changed something, or failed),
/// then prune old runs so `.bide/runs` does not fill up with noise.
fn persist_run(
    id: &str,
    record: &RunRecord,
    cursor: usize,
    board: Vec<(String, String)>,
    changed: bool,
) -> Option<PathBuf> {
    if !worth_saving(record.status, changed) {
        return None;
    }
    save_state(
        id,
        &RunState {
            task: record.task.clone(),
            cursor,
            board,
        },
    );
    let path = record_run(record, id);
    let _ = bide::report::prune(Path::new(RUNS_DIR), KEEP_RUNS);
    path
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
