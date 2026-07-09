use super::{CommandResult, Shell};
use std::process::Command;

/// Runs commands through the platform shell. Only reached after the Policy
/// Engine has allowed the command.
#[derive(Debug, Default)]
pub struct ProcessShell;

impl Shell for ProcessShell {
    fn run(&mut self, command: &str) -> CommandResult {
        let status = shell_command(command).status();
        CommandResult {
            success: matches!(status, Ok(status) if status.success()),
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
