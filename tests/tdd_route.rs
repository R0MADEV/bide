use bide::route::{guess, is_hedged_task, is_task_reply, route_prompt, Guess, Turn};

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
fn only_an_exact_task_reply_starts_a_workflow() {
    assert!(is_task_reply("TASK"));
    assert!(is_task_reply("  task\n"));
    // The model hedges: blurts TASK then reconsiders. That is not a task.
    assert!(!is_task_reply(
        "TASK\n\nWait — this names no concrete target."
    ));
    assert!(!is_task_reply("This is a question, here is the answer…"));
}

#[test]
fn a_leaked_task_token_is_detected_as_a_hedge() {
    assert!(is_hedged_task("TASK\n\nWait — let me reconsider."));
    assert!(is_hedged_task("task, but actually this is vague"));
    // A clean task or a real answer is not a hedge.
    assert!(!is_hedged_task("TASK"));
    assert!(!is_hedged_task("This function lives in src/main.rs."));
}

#[test]
fn the_prompt_biases_toward_answering_not_tasking() {
    let prompt = route_prompt(&[], "what do you think about adding something?");
    assert!(prompt.contains("TASK"));
    assert!(prompt.to_lowercase().contains("when in doubt"));
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
