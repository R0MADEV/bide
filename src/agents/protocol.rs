use super::{AgentResponse, Verdict};

const VERDICT_CONTRACT: &str = "End your reply with exactly one line: \
    `VERDICT: PROCEED` if the workflow should continue, or \
    `VERDICT: REJECT: <reason>` if it should not.";

/// The shared prompt + verdict contract used by every AgentRunner, whatever the
/// backend (Claude CLI, OpenAI, Anthropic API). The instructions are tailored to
/// the step's role.
pub fn build_prompt(role: &str, input: &str) -> String {
    format!(
        "You are the {role} agent in the bide workflow. {}\n\n{VERDICT_CONTRACT}\n\nTask: {input}\n",
        role_instruction(role)
    )
}

fn role_instruction(role: &str) -> &'static str {
    match role {
        "plan" | "planner" => {
            "Produce a concrete, minimal implementation plan as a numbered list. For each step \
             give the files likely touched and why. End with a `Checks:` line (commands to run) \
             and a `Risks:` line. Prefer the simplest approach that works; do not over-engineer. \
             Always PROCEED unless the task is impossible or unsafe."
        }
        "critic" => {
            "Critique the plan shown under 'Prior steps'. Look for correctness gaps, missing \
             steps, over-engineering, and architecture or convention violations. If the plan is \
             sound, PROCEED. If it is not, REJECT with a one-line reason naming the biggest flaw \
             so the planner can redo it."
        }
        "review" | "reviewer" => {
            "Review the code changes in the `diff` output under 'Prior steps' against the plan. \
             Judge correctness, quality, architecture and maintainability. If the changes are \
             good and match the plan, PROCEED. Otherwise REJECT with a one-line reason naming \
             what must change, so the implementer can fix it."
        }
        "fix_plan" | "fix_planner" => {
            "Read the failure output under 'Prior steps' and propose a concrete, minimal repair: \
             the likely cause and the smallest change that fixes it. Recommend only; do not \
             implement. PROCEED with the recommendation."
        }
        _ => "Do your job for the task below.",
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_call_keeps_the_diagnostic_as_output() {
        let response = response_from(false, "HTTP 401: invalid api key".to_string());
        assert!(matches!(response.verdict, Verdict::Failed(_)));
        assert_eq!(response.output, "HTTP 401: invalid api key");
    }

    #[test]
    fn a_successful_call_parses_the_verdict() {
        let response = response_from(true, "looks good\nVERDICT: PROCEED".to_string());
        assert_eq!(response.verdict, Verdict::Proceed);
    }
}
