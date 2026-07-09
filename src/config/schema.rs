use serde::Deserialize;

use super::error::ConfigError;
use crate::{OnFailure, Step, Workflow};

#[derive(Deserialize)]
pub(super) struct Root {
    workflow: WorkflowConfig,
}

#[derive(Deserialize)]
struct WorkflowConfig {
    max_retries: u32,
    #[serde(default)]
    step: Vec<StepConfig>,
}

#[derive(Deserialize)]
struct StepConfig {
    name: String,
    on_failure: OnFailureConfig,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    pause: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum OnFailureConfig {
    Abort,
    RetryFrom(String),
}

pub(super) fn to_workflow(root: Root) -> Result<Workflow, ConfigError> {
    let WorkflowConfig { max_retries, step } = root.workflow;

    if step.is_empty() {
        return Err(ConfigError::EmptyRecipe);
    }
    let has_empty_name = step.iter().any(|s| s.name.trim().is_empty());
    if has_empty_name {
        return Err(ConfigError::EmptyStepName);
    }

    let on_failures = step
        .iter()
        .map(|s| resolve(&s.on_failure, &step))
        .collect::<Result<Vec<_>, _>>()?;

    let steps = step
        .into_iter()
        .zip(on_failures)
        .map(|(s, on_failure)| Step {
            name: s.name,
            on_failure,
            command: s.command,
            pause: s.pause,
        })
        .collect();

    Ok(Workflow { steps, max_retries })
}

fn resolve(on_failure: &OnFailureConfig, steps: &[StepConfig]) -> Result<OnFailure, ConfigError> {
    match on_failure {
        OnFailureConfig::Abort => Ok(OnFailure::Abort),
        OnFailureConfig::RetryFrom(target) => {
            let Some(index) = steps.iter().position(|s| &s.name == target) else {
                return Err(ConfigError::UnknownRetryTarget(target.clone()));
            };
            Ok(OnFailure::RetryFrom(index))
        }
    }
}
