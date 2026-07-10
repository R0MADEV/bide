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
