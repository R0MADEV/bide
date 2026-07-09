use super::{AgentResponse, Verdict};

/// The shared prompt + verdict contract used by every AgentRunner, whatever the
/// backend (Claude CLI, OpenAI, Anthropic API).
pub fn build_prompt(role: &str, input: &str) -> String {
    format!(
        "You are the {role} agent in the bide workflow. \
         Do the {role} job for the task below, then end your reply with exactly \
         one line: `VERDICT: PROCEED` if the workflow should continue, or \
         `VERDICT: REJECT: <reason>` if it should not.\n\nTask: {input}\n"
    )
}

pub(crate) fn response_from(success: bool, output: String) -> AgentResponse {
    if !success {
        return AgentResponse {
            verdict: Verdict::Failed("agent call failed".to_string()),
            output,
        };
    }
    AgentResponse {
        verdict: parse_verdict(&output),
        output,
    }
}

fn parse_verdict(output: &str) -> Verdict {
    let marker = output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("VERDICT:"));
    let Some(line) = marker else {
        return Verdict::Failed("agent did not emit a verdict".to_string());
    };

    let body = line["VERDICT:".len()..].trim();
    if body.eq_ignore_ascii_case("PROCEED") {
        return Verdict::Proceed;
    }
    if let Some(reason) = reject_reason(body) {
        return Verdict::Reject(reason);
    }
    Verdict::Failed(format!("unknown verdict: {body}"))
}

fn reject_reason(body: &str) -> Option<String> {
    let rest = body.strip_prefix("REJECT")?;
    Some(rest.trim_start_matches(':').trim().to_string())
}
