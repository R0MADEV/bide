use bide::route::{guess, route_prompt, Guess, Turn};

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

#[test]
fn prompt_without_history_just_asks_about_the_input() {
    let prompt = route_prompt(&[], "how does resume work");
    assert!(prompt.contains("how does resume work"));
    assert!(!prompt.contains("Earlier in this conversation"));
}

#[test]
fn prompt_replays_recent_turns_for_follow_ups() {
    let history = vec![Turn {
        question: "what is the engine".to_string(),
        answer: "it runs steps".to_string(),
    }];
    let prompt = route_prompt(&history, "and where is that used?");
    assert!(prompt.contains("Earlier in this conversation"));
    assert!(prompt.contains("what is the engine"));
    assert!(prompt.contains("it runs steps"));
    assert!(prompt.contains("and where is that used?"));
}

#[test]
fn prompt_keeps_only_the_last_few_turns() {
    let history: Vec<Turn> = (0..6)
        .map(|i| Turn {
            question: format!("q{i}"),
            answer: format!("a{i}"),
        })
        .collect();
    let prompt = route_prompt(&history, "next");
    assert!(!prompt.contains("q0"));
    assert!(!prompt.contains("q1"));
    assert!(prompt.contains("q2"));
    assert!(prompt.contains("q5"));
}
