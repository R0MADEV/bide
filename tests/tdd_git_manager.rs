use bide::git::{branch_name, commit_message, parse_status};

#[test]
fn an_empty_status_is_a_clean_tree() {
    let status = parse_status("");
    assert!(status.clean);
    assert!(status.changed_files.is_empty());
}

#[test]
fn a_porcelain_status_lists_changed_files() {
    let status = parse_status(" M src/main.rs\n?? new.txt\nA  staged.rs\n");
    assert!(!status.clean);
    assert_eq!(status.changed_files, vec!["src/main.rs", "new.txt", "staged.rs"]);
}

#[test]
fn a_task_becomes_a_slugged_branch_name() {
    assert_eq!(branch_name("Add JWT auth!"), "bide/add-jwt-auth");
}

#[test]
fn branch_names_collapse_repeated_separators() {
    assert_eq!(branch_name("  fix:  login   redirect "), "bide/fix-login-redirect");
}

#[test]
fn commit_message_prefixes_the_task() {
    assert_eq!(commit_message("  add jwt auth  "), "bide: add jwt auth");
}
