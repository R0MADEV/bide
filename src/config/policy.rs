use super::error::ConfigError;
use serde::Deserialize;

/// Project-specific security rules from the `[policy]` section. These are added
/// on top of the built-in rules, never replacing them.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PolicySettings {
    #[serde(default)]
    pub deny_commands: Vec<String>,
    #[serde(default)]
    pub secret_paths: Vec<String>,
}

#[derive(Deserialize)]
struct Root {
    policy: Option<PolicySettings>,
}

pub(super) fn parse(input: &str) -> Result<PolicySettings, ConfigError> {
    let root: Root = toml::from_str(input)?;
    Ok(root.policy.unwrap_or_default())
}
