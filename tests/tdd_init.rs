use bide::config::{parse, scaffold, STARTER_CONFIG};
use std::fs;
use std::path::PathBuf;

fn temp_config(marker: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bide-init-{marker}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.join("bide.toml")
}

#[test]
fn creates_the_config_when_none_exists() {
    let path = temp_config("create");

    let created = scaffold(&path).unwrap();

    assert!(created);
    assert_eq!(fs::read_to_string(&path).unwrap(), STARTER_CONFIG);
}

#[test]
fn refuses_to_overwrite_an_existing_config() {
    let path = temp_config("refuse");
    fs::write(&path, "# hand-written, keep me").unwrap();

    let created = scaffold(&path).unwrap();

    assert!(!created);
    assert_eq!(fs::read_to_string(&path).unwrap(), "# hand-written, keep me");
}

#[test]
fn the_scaffolded_config_is_a_valid_workflow() {
    let workflow = parse(STARTER_CONFIG).unwrap();

    let names: Vec<&str> = workflow.steps.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["plan", "critic", "implement", "verify", "diff", "review"]);
}
