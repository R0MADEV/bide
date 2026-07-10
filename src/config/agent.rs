use super::error::ConfigError;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    OpenAi,
    Anthropic,
}

/// The agent backend to reason with, from the `[agent]` section. The API key is
/// never stored here: `api_key_env` names the environment variable that holds it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentSettings {
    pub provider: Provider,
    pub model: String,
    pub api_key_env: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    4096
}

#[derive(Deserialize)]
struct Root {
    agent: Option<AgentSettings>,
}

pub(super) fn parse(input: &str) -> Result<Option<AgentSettings>, ConfigError> {
    let root: Root = toml::from_str(input)?;
    Ok(root.agent)
}
