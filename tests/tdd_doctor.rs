use bide::doctor::{config_check, is_healthy, tool_check, ConfigState, Level};

#[test]
fn a_present_tool_is_ok() {
    assert!(matches!(tool_check("git", true, true).level, Level::Ok));
}

#[test]
fn a_required_missing_tool_fails_health() {
    let checks = vec![tool_check("git", false, true)];
    assert!(!is_healthy(&checks));
}

#[test]
fn an_optional_missing_tool_only_warns() {
    let check = tool_check("lexis", false, false);
    assert!(matches!(check.level, Level::Warn));
    assert!(is_healthy(&[check]));
}

#[test]
fn an_invalid_config_fails_health() {
    let check = config_check(ConfigState::Invalid("bad recipe".to_string()));
    assert!(matches!(check.level, Level::Fail));
    assert!(!is_healthy(&[check]));
}

#[test]
fn a_missing_config_only_warns() {
    assert!(matches!(config_check(ConfigState::Missing).level, Level::Warn));
}
