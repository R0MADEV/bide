use bide::cli::{parse, Command};

fn args(items: &[&str]) -> std::vec::IntoIter<String> {
    items
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
}

#[test]
fn parses_run_with_task_description() {
    let command = parse(args(&["run", "add jwt to the backend"])).unwrap();
    assert_eq!(
        command,
        Command::Run {
            task: "add jwt to the backend".to_string(),
            yes: false,
        }
    );
}

#[test]
fn parses_the_yes_flag_before_or_after_the_task() {
    assert_eq!(
        parse(args(&["run", "add jwt", "--yes"])).unwrap(),
        Command::Run {
            task: "add jwt".to_string(),
            yes: true,
        }
    );
    assert_eq!(
        parse(args(&["run", "-y", "add jwt"])).unwrap(),
        Command::Run {
            task: "add jwt".to_string(),
            yes: true,
        }
    );
}

#[test]
fn an_unknown_flag_is_an_error() {
    assert!(parse(args(&["run", "add jwt", "--nope"])).is_err());
}

#[test]
fn parses_the_doctor_command() {
    assert_eq!(parse(args(&["doctor"])).unwrap(), Command::Doctor);
}

#[test]
fn run_requires_a_task_description() {
    assert!(parse(args(&["run"])).is_err());
}

#[test]
fn empty_input_is_an_error() {
    assert!(parse(args(&[])).is_err());
}

#[test]
fn unknown_command_is_an_error() {
    assert!(parse(args(&["frobnicate"])).is_err());
}
