use bide::detect::verify_command;
use std::fs;

fn temp_project(marker: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bide-detect-{}", marker.replace('.', "-")));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(marker), "").unwrap();
    dir
}

#[test]
fn detects_a_rust_project() {
    let dir = temp_project("Cargo.toml");
    assert_eq!(verify_command(&dir).as_deref(), Some("cargo test"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn detects_a_node_project() {
    let dir = temp_project("package.json");
    assert_eq!(verify_command(&dir).as_deref(), Some("npm test"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_project_has_no_default() {
    let dir = std::env::temp_dir().join("bide-detect-empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(verify_command(&dir), None);
    let _ = fs::remove_dir_all(&dir);
}
