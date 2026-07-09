mod command;
mod secret;

use std::path::{Path, PathBuf};

/// Something bide is about to do that must be checked before it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    RunCommand(String),
    AccessPath(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(String),
    RequireApproval(String),
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allow)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Decision::Deny(_))
    }

    pub fn needs_approval(&self) -> bool {
        matches!(self, Decision::RequireApproval(_))
    }
}

/// Security rules applied before any tool runs. Lives outside the agents: an
/// agent may recommend a command, but the Policy Engine decides if it is allowed.
#[derive(Debug, Default)]
pub struct Policy;

impl Policy {
    pub fn evaluate(&self, action: &Action) -> Decision {
        match action {
            Action::RunCommand(command) => evaluate_command(command),
            Action::AccessPath(path) => evaluate_path(path),
        }
    }
}

fn evaluate_command(command: &str) -> Decision {
    if let Some(reason) = command::denied_reason(command) {
        return Decision::Deny(format!("destructive command ({reason})"));
    }
    if let Some(reason) = command::approval_reason(command) {
        return Decision::RequireApproval(format!("needs confirmation ({reason})"));
    }
    Decision::Allow
}

fn evaluate_path(path: &Path) -> Decision {
    match secret::secret_reason(path) {
        Some(reason) => Decision::Deny(reason),
        None => Decision::Allow,
    }
}
