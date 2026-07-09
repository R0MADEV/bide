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
            task: "add jwt to the backend".to_string()
        }
    );
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
