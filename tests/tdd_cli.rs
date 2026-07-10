use bide::cli::{confirmed, parse, Command, RunOptions};

fn args(items: &[&str]) -> std::vec::IntoIter<String> {
    items
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
}

fn run(items: &[&str]) -> RunOptions {
    match parse(args(items)).unwrap() {
        Command::Run(options) => options,
        other => panic!("expected Run, got {other:?}"),
    }
}

#[test]
fn parses_run_with_task_description() {
    let options = run(&["run", "add jwt to the backend"]);
    assert_eq!(options.task, "add jwt to the backend");
    assert!(!options.yes);
}

#[test]
fn parses_the_yes_flag_before_or_after_the_task() {
    assert!(run(&["run", "add jwt", "--yes"]).yes);
    assert!(run(&["run", "-y", "add jwt"]).yes);
}

#[test]
fn parses_the_opt_in_flags() {
    let options = run(&["run", "add jwt", "--branch", "--pr"]);
    assert!(options.branch);
    assert!(options.pr);
}

#[test]
fn parses_flags_that_take_a_value() {
    let options = run(&["run", "add jwt", "--agent", "claude", "--context", "lexis"]);
    assert_eq!(options.agent.as_deref(), Some("claude"));
    assert_eq!(options.context.as_deref(), Some("lexis"));
}

#[test]
fn a_value_flag_without_a_value_is_an_error() {
    assert!(parse(args(&["run", "add jwt", "--agent"])).is_err());
}

#[test]
fn parses_the_resume_flag_without_a_task() {
    let options = run(&["run", "--resume", "run-123"]);
    assert_eq!(options.resume.as_deref(), Some("run-123"));
    assert!(options.task.is_empty());
}

#[test]
fn parses_the_tui_command() {
    assert!(matches!(
        parse(args(&["tui", "add jwt"])).unwrap(),
        Command::Tui(_)
    ));
}

#[test]
fn parses_the_doctor_command() {
    assert_eq!(parse(args(&["doctor"])).unwrap(), Command::Doctor);
}

#[test]
fn parses_the_clean_command() {
    assert_eq!(parse(args(&["clean"])).unwrap(), Command::Clean);
}

#[test]
fn parses_the_init_command() {
    assert_eq!(parse(args(&["init"])).unwrap(), Command::Init);
}

#[test]
fn parses_the_help_command() {
    assert_eq!(parse(args(&["help"])).unwrap(), Command::Help);
    assert_eq!(parse(args(&["--help"])).unwrap(), Command::Help);
}

#[test]
fn run_requires_a_task_description() {
    assert!(parse(args(&["run"])).is_err());
}

#[test]
fn no_args_launches_the_repl() {
    assert_eq!(parse(args(&[])).unwrap(), Command::Repl);
}

#[test]
fn an_unknown_flag_is_an_error() {
    assert!(parse(args(&["run", "add jwt", "--nope"])).is_err());
}

#[test]
fn unknown_command_is_an_error() {
    assert!(parse(args(&["frobnicate"])).is_err());
}

#[test]
fn confirmation_accepts_only_yes() {
    assert!(confirmed("y"));
    assert!(confirmed("Y"));
    assert!(confirmed("y\n"));
    assert!(confirmed("  y  "));
}

#[test]
fn confirmation_rejects_anything_else() {
    assert!(!confirmed("n"));
    assert!(!confirmed("N"));
    assert!(!confirmed(""));
    assert!(!confirmed("yes"));
    assert!(!confirmed("nope"));
}
