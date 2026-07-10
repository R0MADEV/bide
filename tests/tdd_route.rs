use bide::route::{guess, Guess};

#[test]
fn obvious_tasks_are_tasks() {
    assert_eq!(guess("add jwt auth"), Some(Guess::Task));
    assert_eq!(guess("Fix the login bug"), Some(Guess::Task));
    assert_eq!(guess("implementa la validación"), Some(Guess::Task));
    assert_eq!(guess("refactor the engine"), Some(Guess::Task));
}

#[test]
fn obvious_questions_are_questions() {
    assert_eq!(guess("how does resume work"), Some(Guess::Question));
    assert_eq!(guess("what is the engine"), Some(Guess::Question));
    assert_eq!(guess("¿qué hace esto?"), Some(Guess::Question));
    assert_eq!(guess("que hace este proyecto ?"), Some(Guess::Question));
}

#[test]
fn ambiguous_input_is_not_guessed() {
    assert_eq!(guess("the login flow"), None);
    assert_eq!(guess("jwt tokens everywhere"), None);
    assert_eq!(guess(""), None);
}
