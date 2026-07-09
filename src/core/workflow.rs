/// What the engine does when a step reports failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnFailure {
    Abort,
    RetryFrom(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub on_failure: OnFailure,
    pub command: Option<String>,
    pub pause: bool,
}

impl Step {
    pub fn abort(name: &str) -> Self {
        Step {
            name: name.to_string(),
            on_failure: OnFailure::Abort,
            command: None,
            pause: false,
        }
    }

    pub fn retry_from(name: &str, step: usize) -> Self {
        Step {
            name: name.to_string(),
            on_failure: OnFailure::RetryFrom(step),
            command: None,
            pause: false,
        }
    }

    pub fn with_command(mut self, command: &str) -> Self {
        self.command = Some(command.to_string());
        self
    }
}

/// An ordered, composable list of steps. This is the recipe bide drives; it can
/// be built in code or loaded from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workflow {
    pub steps: Vec<Step>,
    pub max_retries: u32,
}

impl Workflow {
    pub fn default_recipe() -> Self {
        let implement = 2;
        Workflow {
            max_retries: 2,
            steps: vec![
                Step::abort("build_context"),
                Step::abort("plan"),
                Step::abort("implement"),
                Step::retry_from("verify", implement),
                Step::abort("review"),
            ],
        }
    }
}
