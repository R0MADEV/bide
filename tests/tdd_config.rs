use bide::config::{parse, parse_agent, Provider};
use bide::OnFailure;

#[test]
fn parses_an_openai_agent_section() {
    let input = r#"
        [agent]
        provider = "openai"
        model = "gpt-4o"
        api_key_env = "OPENAI_API_KEY"
    "#;

    let settings = parse_agent(input).expect("valid").expect("agent present");

    assert_eq!(settings.provider, Provider::OpenAi);
    assert_eq!(settings.model, "gpt-4o");
    assert_eq!(settings.api_key_env, "OPENAI_API_KEY");
}

#[test]
fn parses_an_anthropic_agent_section() {
    let input = r#"
        [agent]
        provider = "anthropic"
        model = "claude-sonnet-4-6"
        api_key_env = "ANTHROPIC_API_KEY"
    "#;

    let settings = parse_agent(input).expect("valid").expect("agent present");
    assert_eq!(settings.provider, Provider::Anthropic);
}

#[test]
fn no_agent_section_yields_none() {
    let input = "[workflow]\nmax_retries = 0\n";
    assert!(parse_agent(input).expect("valid").is_none());
}

#[test]
fn an_unknown_provider_is_rejected() {
    let input = r#"
        [agent]
        provider = "gemini"
        model = "x"
        api_key_env = "Y"
    "#;
    assert!(parse_agent(input).is_err());
}

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
fn parses_an_optional_command_on_a_step() {
    let input = r#"
        [workflow]
        max_retries = 0

        [[workflow.step]]
        name = "verify"
        on_failure = "abort"
        command = "cargo test"
    "#;

    let workflow = parse(input).expect("valid recipe");

    assert_eq!(workflow.steps[0].command.as_deref(), Some("cargo test"));
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
