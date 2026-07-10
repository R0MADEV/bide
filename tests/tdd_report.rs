use bide::report::{prune, render, save, worth_saving, RunRecord, StepRecord};
use bide::{Status, StepOutcome};

#[test]
fn a_successful_no_change_run_is_not_worth_saving() {
    assert!(!worth_saving(Status::Accepted, false)); // succeeded, changed nothing
    assert!(worth_saving(Status::Accepted, true)); // succeeded and changed the tree
    assert!(worth_saving(Status::Failed, false)); // a failure is worth inspecting
}

#[test]
fn prune_keeps_only_the_newest_runs() {
    let runs = std::env::temp_dir().join("bide-test-prune");
    let _ = std::fs::remove_dir_all(&runs);
    for id in ["run-1", "run-2", "run-3", "run-4"] {
        std::fs::create_dir_all(runs.join(id)).unwrap();
    }

    prune(&runs, 2).expect("prune");

    assert!(!runs.join("run-1").exists());
    assert!(!runs.join("run-2").exists());
    assert!(runs.join("run-3").exists());
    assert!(runs.join("run-4").exists());
    let _ = std::fs::remove_dir_all(&runs);
}

fn sample() -> RunRecord {
    RunRecord {
        task: "add jwt".to_string(),
        status: Status::Failed,
        diff: "diff --git a/src/auth.rs b/src/auth.rs".to_string(),
        context: "pub fn login() { /* relevant code */ }".to_string(),
        steps: vec![
            StepRecord {
                name: "plan".to_string(),
                outcome: StepOutcome::Success,
                output: "the plan body".to_string(),
                prompt: "please plan this task".to_string(),
            },
            StepRecord {
                name: "verify".to_string(),
                outcome: StepOutcome::Failure,
                output: "command failed".to_string(),
                prompt: String::new(),
            },
        ],
    }
}

#[test]
fn report_includes_task_status_steps_and_outputs() {
    let markdown = render(&sample());
    assert!(markdown.contains("add jwt"));
    assert!(markdown.contains("Failed"));
    assert!(markdown.contains("plan"));
    assert!(markdown.contains("verify"));
    assert!(markdown.contains("the plan body"));
    assert!(markdown.contains("please plan this task"));
}

#[test]
fn report_includes_the_diff_when_present() {
    let markdown = render(&sample());
    assert!(markdown.contains("## Diff"));
    assert!(markdown.contains("diff --git a/src/auth.rs"));
}

#[test]
fn save_writes_one_artifact_file_per_step() {
    let runs_dir = std::env::temp_dir().join("bide-test-runs-steps");
    let _ = std::fs::remove_dir_all(&runs_dir);

    save(&sample(), &runs_dir, "run-x").expect("save");
    let step_file = runs_dir.join("run-x/steps/01-plan.md");
    let content = std::fs::read_to_string(&step_file).expect("read step file");

    assert!(content.contains("the plan body"));
    let _ = std::fs::remove_dir_all(&runs_dir);
}

#[test]
fn save_writes_the_captured_context_to_its_own_file() {
    let runs_dir = std::env::temp_dir().join("bide-test-runs-ctx");
    let _ = std::fs::remove_dir_all(&runs_dir);

    save(&sample(), &runs_dir, "run-ctx").expect("save");
    let context = std::fs::read_to_string(runs_dir.join("run-ctx/context.md")).expect("read");

    assert!(context.contains("relevant code"));
    let _ = std::fs::remove_dir_all(&runs_dir);
}

#[test]
fn save_writes_a_report_file_under_the_run_id() {
    let runs_dir = std::env::temp_dir().join("bide-test-runs");
    let _ = std::fs::remove_dir_all(&runs_dir);

    let path = save(&sample(), &runs_dir, "run-test").expect("save");
    let written = std::fs::read_to_string(&path).expect("read");

    assert!(path.ends_with("run-test/report.md"));
    assert!(written.contains("add jwt"));
    let _ = std::fs::remove_dir_all(&runs_dir);
}
