use super::error::ConfigError;
use serde::Deserialize;

/// The external tool binaries bide drives, from the `[tools]` section. Each
/// defaults to the plain command name on PATH.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolSettings {
    #[serde(default = "claude_default")]
    pub claude: String,
    #[serde(default = "lexis_default")]
    pub lexis: String,
    #[serde(default = "gh_default")]
    pub gh: String,
}

fn claude_default() -> String {
    "claude".to_string()
}

fn lexis_default() -> String {
    "lexis".to_string()
}

fn gh_default() -> String {
    "gh".to_string()
}

impl Default for ToolSettings {
    fn default() -> Self {
        ToolSettings {
            claude: claude_default(),
            lexis: lexis_default(),
            gh: gh_default(),
        }
    }
}

#[derive(Deserialize)]
struct Root {
    tools: Option<ToolSettings>,
}

pub(super) fn parse(input: &str) -> Result<ToolSettings, ConfigError> {
    let root: Root = toml::from_str(input)?;
    Ok(root.tools.unwrap_or_default())
}
