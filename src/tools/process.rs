use super::{CommandResult, Shell};
use crate::exec;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(600);

/// Runs commands through the platform shell. Only reached after the Policy
/// Engine has allowed the command, and always under a timeout.
#[derive(Debug, Default)]
pub struct ProcessShell;

impl Shell for ProcessShell {
    fn run(&mut self, command: &str) -> CommandResult {
        let captured = exec::run(shell_command(command), TIMEOUT);
        CommandResult {
            success: captured.success,
            output: captured.merged(),
        }
    }
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    shell
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}
