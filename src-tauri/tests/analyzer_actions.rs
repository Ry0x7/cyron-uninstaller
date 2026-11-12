//! Public API integration coverage for review-first Analyzer actions.
//! All destructive targets are uniquely named fixtures under the system temp directory.

use cyron_lib::analyzer_actions::{
    cancel_analyzer_action, execute_analyzer_action, prepare_analyzer_action, AnalyzerAction,
    AnalyzerActionState,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cyron-analyzer-integration-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create disposable Analyzer fixture");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn prepare(state: &AnalyzerActionState, paths: &[&Path], operation: AnalyzerAction) -> String {
    prepare_analyzer_action(
        state,
        paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        operation,
    )
    .expect("prepare fixture action")
    .plan_id
}

#[test]
fn permanent_action_removes_only_the_exact_file_and_empty_folder_selection() {
    let fixture = Fixture::new();
    let file = fixture.path().join("selected-marker.txt");
    let empty_dir = fixture.path().join("selected-empty");
    let sibling = fixture.path().join("unrelated-sibling.txt");
    fs::write(&file, b"disposable marker").unwrap();
    fs::create_dir(&empty_dir).unwrap();
    fs::write(&sibling, b"must remain").unwrap();

    let state = AnalyzerActionState::default();
    let plan_id = prepare(&state, &[&file, &empty_dir], AnalyzerAction::Permanent);
    let report = execute_analyzer_action(&state, &plan_id, |_| {}).unwrap();

    assert!(!report.cancelled);
    assert_eq!(report.completed_files, 1);
    assert_eq!(report.completed_dirs, 1);
    assert!(report.items.iter().all(|item| item.status == "completed"));
    assert!(!file.exists());
    assert!(!empty_dir.exists());
    assert_eq!(fs::read(&sibling).unwrap(), b"must remain");
}

#[test]
fn cancellation_after_first_item_reports_remaining_item_and_preserves_it() {
    let fixture = Fixture::new();
    let first = fixture.path().join("a-first.txt");
    let second = fixture.path().join("b-second.txt");
    fs::write(&first, b"first disposable marker").unwrap();
    fs::write(&second, b"second must remain").unwrap();

    let state = AnalyzerActionState::default();
    let plan_id = prepare(&state, &[&first, &second], AnalyzerAction::Permanent);
    let callback_state = &state;
    let callback_plan = plan_id.clone();
    let report = execute_analyzer_action(&state, &plan_id, move |progress| {
        if progress.completed == 1 {
            cancel_analyzer_action(callback_state, &callback_plan)
                .expect("cancel running fixture action");
        }
    })
    .unwrap();

    assert!(report.cancelled);
    assert_eq!(report.completed_files, 1);
    assert_eq!(
        report
            .items
            .iter()
            .filter(|item| item.status == "completed")
            .count(),
        1
    );
    assert_eq!(
        report
            .items
            .iter()
            .filter(|item| item.status == "cancelled")
            .count(),
        1
    );
    assert!(!first.exists());
    assert_eq!(fs::read(&second).unwrap(), b"second must remain");
}

#[test]
#[ignore = "requires interactive Windows Recycle Bin"]
fn recycle_action_recycles_only_its_disposable_marker_file() {
    let fixture = Fixture::new();
    let marker_name = format!(
        "{}-recycle-marker.txt",
        fixture.path().file_name().unwrap().to_string_lossy()
    );
    let file = fixture.path().join(&marker_name);
    fs::write(&file, b"disposable recycle marker").unwrap();

    let state = AnalyzerActionState::default();
    let plan_id = prepare(&state, &[&file], AnalyzerAction::Recycle);
    let report = execute_analyzer_action(&state, &plan_id, |_| {}).unwrap();

    if report.completed_files != 1 {
        let source_exists = file.exists();
        std::mem::forget(fixture);
        panic!("Recycle Bin action failed: {report:?}; source_exists={source_exists}");
    }
    assert!(!report.cancelled);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].status, "completed");
    assert!(
        !file.exists(),
        "recycled fixture should be absent at its source"
    );
    let script = format!(
        "$shell = New-Object -ComObject Shell.Application; $items = $shell.Namespace(10).Items(); if (@($items | Where-Object {{ $_.Name -eq '{marker_name}' }}).Count -gt 0) {{ exit 0 }}; exit 1"
    );
    assert!(
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .expect("PowerShell to inspect Recycle Bin for the unique marker")
            .success(),
        "the unique fixture marker should be present in the Windows Recycle Bin"
    );
}
