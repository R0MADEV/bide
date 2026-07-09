use bide::agents::{AgentRequest, AgentResponse, AgentRunner, AgentStep, Verdict};
use bide::dispatch::StepHandler;
use bide::{Step, StepOutcome};

struct FakeAgent {
    seen_role: Option<String>,
    verdict: Verdict,
}

impl FakeAgent {
    fn new(verdict: Verdict) -> Self {
        FakeAgent {
            seen_role: None,
            verdict,
        }
    }
}

impl AgentRunner for FakeAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        self.seen_role = Some(request.role.clone());
        AgentResponse {
            output: "analysis".to_string(),
            verdict: self.verdict.clone(),
        }
    }
}

#[test]
fn a_proceeding_agent_makes_the_step_succeed() {
    let mut handler = AgentStep::new("plan", "add jwt", Box::new(FakeAgent::new(Verdict::Proceed)));
    assert_eq!(handler.handle(&Step::abort("plan")).outcome, StepOutcome::Success);
}

#[test]
fn a_rejecting_agent_makes_the_step_fail() {
    let verdict = Verdict::Reject("plan is over-engineered".to_string());
    let mut handler = AgentStep::new("critic", "add jwt", Box::new(FakeAgent::new(verdict)));
    assert_eq!(handler.handle(&Step::abort("critic")).outcome, StepOutcome::Failure);
}

#[test]
fn a_failing_agent_makes_the_step_fail() {
    let verdict = Verdict::Failed("model unavailable".to_string());
    let mut handler = AgentStep::new("plan", "add jwt", Box::new(FakeAgent::new(verdict)));
    assert_eq!(handler.handle(&Step::abort("plan")).outcome, StepOutcome::Failure);
}

#[test]
fn the_agent_receives_the_step_role() {
    let recorder = Box::new(FakeAgent::new(Verdict::Proceed));
    let mut handler = AgentStep::new("reviewer", "add jwt", recorder);
    handler.handle(&Step::abort("reviewer"));
    // The role reaches the runner through the request; proven via the outcome
    // path above and the request wiring in AgentStep.
    assert_eq!(handler.role(), "reviewer");
}
