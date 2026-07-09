use super::{CommandResult, Shell};
use std::process::Command;

/// Runs commands through the platform shell. Only reached after the Policy
/// Engine has allowed the command.
#[derive(Debug, Default)]
pub struct ProcessShell;

impl Shell for ProcessShell {
    fn run(&mut self, command: &str) -> CommandResult {
        let Ok(output) = shell_command(command).output() else {
            return CommandResult {
                success: false,
                output: "failed to spawn command".to_string(),
            };
        };

        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            text.push_str(&stderr);
        }
        CommandResult {
            success: output.status.success(),
            output: text,
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
