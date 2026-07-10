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
        "A user typed this into a coding tool: \"{input}\"\n\nIf it is a QUESTION \
         about the codebase, answer it clearly using the lexis tools to read the \
         real code. If it is a request to CHANGE, ADD or FIX code (a task to do), \
         reply with EXACTLY the single word TASK and nothing else."
    ));
    prompt
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
