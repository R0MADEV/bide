use bide::agents::{AgentRequest, AgentResponse, AgentRunner, AgentStep, Verdict};
use bide::board::Blackboard;
use bide::dispatch::Dispatcher;
use bide::{run, Step, Workflow};

#[test]
fn records_outputs_and_summarises_them() {
    let mut board = Blackboard::new();
    assert!(board.is_empty());

    board.record("plan", "the plan body");
    board.record("critic", "looks good");

    let summary = board.summary();
    assert!(!board.is_empty());
    assert!(summary.contains("plan"));
    assert!(summary.contains("the plan body"));
    assert!(summary.contains("critic"));
}

/// Echoes back the exact input it was given, so a later step's recorded output
/// reveals what the blackboard fed into it.
struct EchoAgent;

impl AgentRunner for EchoAgent {
    fn run(&mut self, request: &AgentRequest) -> AgentResponse {
        AgentResponse {
            output: request.input.clone(),
            verdict: Verdict::Proceed,
        }
    }
}

#[test]
fn a_later_agent_step_sees_earlier_step_output() {
    let workflow = Workflow {
        steps: vec![Step::abort("first"), Step::abort("second")],
        max_retries: 0,
    };
    let mut dispatcher = Dispatcher::new();
    dispatcher.register("first", Box::new(AgentStep::new("first", "FIRST-INPUT", Box::new(EchoAgent))));
    dispatcher.register("second", Box::new(AgentStep::new("second", "SECOND-INPUT", Box::new(EchoAgent))));

    run(&workflow, &mut dispatcher);
    let records = dispatcher.into_records();

    // The second step's output echoes its input, which now carries the first
    // step's output through the blackboard.
    assert!(records[1].output.contains("FIRST-INPUT"));
}
