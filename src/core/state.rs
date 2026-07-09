#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Success,
    Failure,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Accepted,
    Failed,
    Aborted,
}
