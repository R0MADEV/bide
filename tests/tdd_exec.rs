use bide::exec::run;
use std::process::Command;
use std::time::Duration;

fn sh(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[test]
fn captures_the_output_of_a_fast_command() {
    let captured = run(sh("echo hello"), Duration::from_secs(5));
    assert!(captured.success);
    assert!(!captured.timed_out);
    assert!(captured.stdout.contains("hello"));
}

#[test]
fn kills_a_command_that_exceeds_the_timeout() {
    let captured = run(sh("sleep 5"), Duration::from_millis(300));
    assert!(captured.timed_out);
    assert!(!captured.success);
}

#[test]
fn captures_stderr_separately() {
    let captured = run(sh("echo oops 1>&2"), Duration::from_secs(5));
    assert!(captured.stderr.contains("oops"));
    assert!(captured.merged().contains("oops"));
}

#[test]
fn reports_the_exit_status() {
    assert!(!run(sh("exit 1"), Duration::from_secs(5)).success);
    assert!(run(sh("true"), Duration::from_secs(5)).success);
}
