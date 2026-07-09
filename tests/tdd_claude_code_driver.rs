use bide::agents::{
    build_prompt, AgentProcess, AgentProcessResult, AgentRunner, ClaudeCodeAgent, Verdict,
};

struct FakeProcess {
    stdout: String,
    success: bool,
}

impl AgentProcess for FakeProcess {
    fn run(&mut self, _prompt: &str) -> AgentProcessResult {
        AgentProcessResult {
            success: self.success,
            stdout: self.stdout.clone(),
        }
    }
}

fn agent(stdout: &str, success: bool) -> ClaudeCodeAgent {
    ClaudeCodeAgent::new(Box::new(FakeProcess {
        stdout: stdout.to_string(),
        success,
    }))
}

fn verdict(stdout: &str, success: bool) -> Verdict {
    let mut agent = agent(stdout, success);
    agent
        .run(&bide::agents::AgentRequest {
            role: "critic".to_string(),
            input: "add jwt".to_string(),
        })
        .verdict
}

#[test]
fn parses_a_proceed_verdict() {
    let out = "The plan looks sound.\nVERDICT: PROCEED\n";
    assert_eq!(verdict(out, true), Verdict::Proceed);
}

#[test]
fn parses_a_reject_verdict_with_reason() {
    let out = "This is over-engineered.\nVERDICT: REJECT: too many layers\n";
    assert_eq!(
        verdict(out, true),
        Verdict::Reject("too many layers".to_string())
    );
}

#[test]
fn missing_verdict_is_treated_as_failure() {
    assert!(matches!(verdict("no marker here", true), Verdict::Failed(_)));
}

#[test]
fn a_failed_process_is_a_failed_verdict() {
    assert!(matches!(verdict("", false), Verdict::Failed(_)));
}

#[test]
fn the_prompt_carries_the_role_task_and_verdict_contract() {
    let prompt = build_prompt("planner", "add jwt to the backend");
    assert!(prompt.contains("planner"));
    assert!(prompt.contains("add jwt to the backend"));
    assert!(prompt.contains("VERDICT:"));
}
