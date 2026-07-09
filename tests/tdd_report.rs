use bide::report::{render, save, RunRecord, StepRecord};
use bide::{Status, StepOutcome};

fn sample() -> RunRecord {
    RunRecord {
        task: "add jwt".to_string(),
        status: Status::Failed,
        diff: "diff --git a/src/auth.rs b/src/auth.rs".to_string(),
        steps: vec![
            StepRecord {
                name: "plan".to_string(),
                outcome: StepOutcome::Success,
                output: "the plan body".to_string(),
            },
            StepRecord {
                name: "verify".to_string(),
                outcome: StepOutcome::Failure,
                output: "command failed".to_string(),
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
}

#[test]
fn report_includes_the_diff_when_present() {
    let markdown = render(&sample());
    assert!(markdown.contains("## Diff"));
    assert!(markdown.contains("diff --git a/src/auth.rs"));
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
