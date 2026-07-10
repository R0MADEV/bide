use bide::context::{build_context, retrieval_prompt, CodeContext};

#[test]
fn the_retrieval_prompt_asks_lexis_for_the_task_code() {
    let prompt = retrieval_prompt("add jwt auth");
    assert!(prompt.contains("add jwt auth"));
    assert!(prompt.contains("lexis"));
    assert!(prompt.to_lowercase().contains("do not edit") || prompt.contains("not implement"));
}

struct FakeContext(String);

impl CodeContext for FakeContext {
    fn lookup(&mut self, _task: &str) -> String {
        self.0.clone()
    }
}

#[test]
fn wraps_the_context_found_for_the_task() {
    let mut provider = FakeContext("Framework: Axum\nRoutes: src/routes.rs".to_string());
    let pack = build_context(&mut provider, "add jwt");
    assert!(pack.text.contains("Axum"));
    assert!(pack.text.contains("src/routes.rs"));
}

#[test]
fn reports_when_no_context_is_found() {
    let mut provider = FakeContext("   ".to_string());
    let pack = build_context(&mut provider, "add jwt");
    assert!(pack.text.contains("No repository context"));
}

#[test]
fn the_task_is_used_as_the_query() {
    struct Recorder {
        seen: Option<String>,
    }
    impl CodeContext for Recorder {
        fn lookup(&mut self, task: &str) -> String {
            self.seen = Some(task.to_string());
            String::new()
        }
    }

    let mut recorder = Recorder { seen: None };
    build_context(&mut recorder, "add jwt to the backend");
    assert_eq!(recorder.seen.as_deref(), Some("add jwt to the backend"));
}
