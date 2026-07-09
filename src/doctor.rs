/// Preflight checks so a first real run fails clearly, not cryptically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

pub struct Check {
    pub name: String,
    pub level: Level,
    pub detail: String,
}

pub enum ConfigState {
    Missing,
    Valid,
    Invalid(String),
}

pub fn tool_check(name: &str, present: bool, required: bool) -> Check {
    let level = match (present, required) {
        (true, _) => Level::Ok,
        (false, true) => Level::Fail,
        (false, false) => Level::Warn,
    };
    let detail = if present {
        "found on PATH".to_string()
    } else {
        "not found on PATH".to_string()
    };
    Check {
        name: name.to_string(),
        level,
        detail,
    }
}

pub fn config_check(state: ConfigState) -> Check {
    let (level, detail) = match state {
        ConfigState::Valid => (Level::Ok, "valid".to_string()),
        ConfigState::Missing => (Level::Warn, "not found — the default recipe will be used".to_string()),
        ConfigState::Invalid(reason) => (Level::Fail, reason),
    };
    Check {
        name: "bide.toml".to_string(),
        level,
        detail,
    }
}

pub fn is_healthy(checks: &[Check]) -> bool {
    checks.iter().all(|check| check.level != Level::Fail)
}
