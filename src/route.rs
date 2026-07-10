//! A cheap guess of whether the user's input is a task or a question, so the
//! obvious cases skip the AI classifier. Only ambiguous input needs the model.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guess {
    Task,
    Question,
}

const TASK_VERBS: &[&str] = &[
    "add", "fix", "implement", "create", "remove", "update", "refactor", "rename", "delete",
    "write", "make", "change", "build", "wire", "extract", "move", "replace", "improve",
    "añade", "agrega", "arregla", "implementa", "crea", "elimina", "borra", "cambia",
    "refactoriza", "escribe", "mueve", "renombra", "haz", "mejora",
];

const QUESTION_WORDS: &[&str] = &[
    "what", "how", "why", "where", "when", "which", "who", "does", "do", "is", "are", "can",
    "should", "explain", "list", "show", "qué", "que", "cómo", "como", "por", "dónde", "donde",
    "cuándo", "cuando", "cuál", "cual", "quién", "quien", "explica", "muestra",
];

/// One past REPL exchange, so a follow-up question can carry context.
#[derive(Debug, Clone)]
pub struct Turn {
    pub question: String,
    pub answer: String,
}

/// How many recent turns to replay, so the prompt stays small.
const MEMORY_TURNS: usize = 4;

/// The prompt that classifies the input (answer a question with lexis, or reply
/// TASK), prefixed with the recent conversation so a follow-up like "and where
/// is that used?" has the context it needs.
pub fn route_prompt(history: &[Turn], input: &str) -> String {
    let mut prompt = String::new();
    let recent = &history[history.len().saturating_sub(MEMORY_TURNS)..];
    if !recent.is_empty() {
        prompt.push_str("Earlier in this conversation:\n");
        for turn in recent {
            prompt.push_str(&format!("Q: {}\nA: {}\n", turn.question.trim(), turn.answer.trim()));
        }
        prompt.push('\n');
    }
    prompt.push_str(&format!(
        "A user typed this into a coding tool: \"{input}\"\n\n\
         Reply with EXACTLY the single word TASK (and nothing else) ONLY if the \
         message names a concrete, specific change to the code — a clear goal you \
         could start editing right now (e.g. \"add a --json flag to the export \
         command\", \"fix the panic in parse_config\").\n\n\
         Do NOT reply TASK for anything vague or without a concrete target. An \
         invitation to build \"something\" with no specifics — like \"vamos a \
         implementar algo\", \"let's build something\", \"add a new feature\", \
         \"implement something new\" — is NOT a task: it names no target. For those, \
         and for questions, opinions, brainstorming or small talk, answer directly \
         instead (use the lexis tools to read the real code when relevant). If it \
         sounds like a task but is too vague to start, ask what specifically they \
         want. When in doubt, answer — never start a task on a guess."
    ));
    prompt
}

/// Whether the classifier's reply is a clean TASK signal. The prompt asks for
/// EXACTLY "TASK"; a reply with any extra text is the model hedging (it often
/// blurts "TASK" then reconsiders), so we treat that as an answer and never
/// start a workflow on it.
pub fn is_task_reply(reply: &str) -> bool {
    reply.trim().eq_ignore_ascii_case("TASK")
}

/// Whether the reply is a hedge: it leaked the TASK token as its first word then
/// trailed off (typical on vague, task-shaped input with no target). We show a
/// clean clarification instead of the garbled text.
pub fn is_hedged_task(reply: &str) -> bool {
    let trimmed = reply.trim();
    if trimmed.eq_ignore_ascii_case("TASK") {
        return false;
    }
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    let cleaned = first_word.trim_matches(|c: char| !c.is_alphanumeric());
    cleaned.eq_ignore_ascii_case("task")
}

pub fn guess(input: &str) -> Option<Guess> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    if text.starts_with('¿') || text.ends_with('?') {
        return Some(Guess::Question);
    }
    let first_word = text.split_whitespace().next()?;
    let first = first_word
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    if TASK_VERBS.contains(&first.as_str()) {
        return Some(Guess::Task);
    }
    if QUESTION_WORDS.contains(&first.as_str()) {
        return Some(Guess::Question);
    }
    None
}
