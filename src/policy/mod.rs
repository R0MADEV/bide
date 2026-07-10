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
/// Built-in rules always apply; a project can add its own via bide.toml.
#[derive(Debug, Clone, Default)]
pub struct Policy {
    extra_denied: Vec<String>,
    extra_secrets: Vec<String>,
}

impl Policy {
    pub fn with_rules(extra_denied: Vec<String>, extra_secrets: Vec<String>) -> Self {
        Policy {
            extra_denied,
            extra_secrets,
        }
    }

    pub fn evaluate(&self, action: &Action) -> Decision {
        match action {
            Action::RunCommand(command) => self.evaluate_command(command),
            Action::AccessPath(path) => self.evaluate_path(path),
        }
    }

    fn evaluate_command(&self, command: &str) -> Decision {
        if let Some(reason) = command::denied_reason(command) {
            return Decision::Deny(format!("destructive command ({reason})"));
        }
        if let Some(pattern) = self.extra_denied.iter().find(|p| command.contains(p.as_str())) {
            return Decision::Deny(format!("denied by config ({pattern})"));
        }
        if let Some(reason) = command::approval_reason(command) {
            return Decision::RequireApproval(format!("needs confirmation ({reason})"));
        }
        Decision::Allow
    }

    fn evaluate_path(&self, path: &Path) -> Decision {
        if let Some(reason) = secret::secret_reason(path) {
            return Decision::Deny(reason);
        }
        let text = path.to_string_lossy();
        if let Some(marker) = self.extra_secrets.iter().find(|m| text.contains(m.as_str())) {
            return Decision::Deny(format!("secret path by config ({marker})"));
        }
        Decision::Allow
    }
}
