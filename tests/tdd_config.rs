use bide::config::parse;
use bide::OnFailure;

#[test]
fn parses_a_valid_recipe_and_resolves_retry_targets_by_name() {
    let input = r#"
        [workflow]
        max_retries = 3

        [[workflow.step]]
        name = "implement"
        on_failure = "abort"

        [[workflow.step]]
        name = "verify"
        on_failure = { retry_from = "implement" }
    "#;

    let workflow = parse(input).expect("valid recipe");

    assert_eq!(workflow.max_retries, 3);
    assert_eq!(workflow.steps.len(), 2);
    assert_eq!(workflow.steps[0].name, "implement");
    assert_eq!(workflow.steps[1].on_failure, OnFailure::RetryFrom(0));
}

#[test]
fn rejects_retry_from_an_unknown_step() {
    let input = r#"
        [workflow]
        max_retries = 2

        [[workflow.step]]
        name = "verify"
        on_failure = { retry_from = "does_not_exist" }
    "#;

    assert!(parse(input).is_err());
}

#[test]
fn rejects_a_recipe_without_steps() {
    let input = r#"
        [workflow]
        max_retries = 2
    "#;

    assert!(parse(input).is_err());
}

#[test]
fn rejects_an_empty_step_name() {
    let input = r#"
        [workflow]
        max_retries = 2

        [[workflow.step]]
        name = ""
        on_failure = "abort"
    "#;

    assert!(parse(input).is_err());
}

#[test]
fn rejects_malformed_toml() {
    assert!(parse("this is not = = toml").is_err());
}
