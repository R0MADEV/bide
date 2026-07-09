mod agent;
mod error;
mod schema;

pub use agent::{AgentSettings, Provider};
pub use error::ConfigError;

use crate::Workflow;
use std::path::Path;

pub fn parse(input: &str) -> Result<Workflow, ConfigError> {
    let root: schema::Root = toml::from_str(input)?;
    schema::to_workflow(root)
}

pub fn load(path: &Path) -> Result<Workflow, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    parse(&text)
}

pub fn parse_agent(input: &str) -> Result<Option<AgentSettings>, ConfigError> {
    agent::parse(input)
}

pub fn load_agent(path: &Path) -> Result<Option<AgentSettings>, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    parse_agent(&text)
}
