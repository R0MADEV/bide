#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("recipe has no steps")]
    EmptyRecipe,
    #[error("a step has an empty name")]
    EmptyStepName,
    #[error("retry_from points to unknown step: {0}")]
    UnknownRetryTarget(String),
}
