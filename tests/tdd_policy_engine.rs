use bide::policy::{Action, Policy};
use std::path::PathBuf;

fn command(input: &str) -> Action {
    Action::RunCommand(input.to_string())
}

fn access(path: &str) -> Action {
    Action::AccessPath(PathBuf::from(path))
}

#[test]
fn denies_destructive_commands() {
    let policy = Policy::default();
    assert!(policy.evaluate(&command("rm -rf /")).is_denied());
    assert!(policy.evaluate(&command("git reset --hard HEAD~5")).is_denied());
}

#[test]
fn requires_approval_for_a_plain_delete() {
    let policy = Policy::default();
    assert!(policy.evaluate(&command("rm old_notes.txt")).needs_approval());
}

#[test]
fn allows_safe_commands() {
    let policy = Policy::default();
    assert!(policy.evaluate(&command("cargo test")).is_allowed());
}

#[test]
fn denies_access_to_secrets() {
    let policy = Policy::default();
    assert!(policy.evaluate(&access(".env")).is_denied());
    assert!(policy.evaluate(&access("config/.env.local")).is_denied());
    assert!(policy.evaluate(&access("/home/dev/.ssh/id_rsa")).is_denied());
}

#[test]
fn allows_access_to_source_files() {
    let policy = Policy::default();
    assert!(policy.evaluate(&access("src/core/engine.rs")).is_allowed());
}
