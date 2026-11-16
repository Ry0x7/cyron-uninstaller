//! Disposable-project round trip for the Phase 3 developer storage engine.
//!
//! The test never touches a real project or user cache. It creates a temporary
//! project with source, Git metadata, credentials and two regenerable folders,
//! then proves that the cleanup plan only quarantines the artifact folders and
//! that restore returns them.

use cyron_lib::{cleanup, devscan};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cyron-phase3-project-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture parent");
        }
        std::fs::write(path, contents).expect("fixture file");
    }

    fn project(&self) -> PathBuf {
        self.root.join("app")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn assert_present(path: &Path) {
    assert!(path.exists(), "{} should still exist", path.display());
}

#[test]
fn disposable_project_cleanup_is_scoped_and_restorable() {
    let fixture = Fixture::new();
    let app = fixture.project();
    fixture.write(
        "app/package.json",
        br#"{"name":"phase3-fixture","dependencies":{"vite":"7.0.0"}}"#,
    );
    fixture.write("app/package-lock.json", b"{}");
    fixture.write("app/src/index.ts", b"export const keep = true;");
    fixture.write("app/.env", b"SECRET=keep-me");
    fixture.write("app/.git/config", b"[core]\nrepositoryformatversion = 0\n");
    fixture.write("app/node_modules/pkg/index.js", b"module.exports = 1;");
    fixture.write("app/dist/index.html", b"<html>built</html>");

    let scan = devscan::scan_selected(vec![app.to_string_lossy().to_string()]).expect("scan");
    let project = scan
        .projects
        .iter()
        .find(|project| project.name == "phase3-fixture")
        .expect("fixture project");
    assert_eq!(project.name, "phase3-fixture");
    assert!(project.source_protected);
    assert!(
        project
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "node-modules"),
        "node_modules must be detected"
    );
    assert!(
        project
            .artifacts
            .iter()
            .any(|artifact| artifact.kind == "dist"),
        "dist must be detected"
    );
    assert!(
        !project
            .artifacts
            .iter()
            .any(|artifact| artifact.path.to_ascii_lowercase().contains("\\.git")),
        "Git data must never be an artifact"
    );
    assert!(
        project
            .protected
            .iter()
            .any(|component| component.path.to_ascii_lowercase().contains("\\.git")),
        "Git data must be reported as protected"
    );
    assert!(
        project
            .protected
            .iter()
            .any(|component| component.path.to_ascii_lowercase().ends_with("\\.env")),
        "credentials must be reported as protected"
    );

    let ids: Vec<String> = project
        .artifacts
        .iter()
        .filter(|artifact| matches!(artifact.kind.as_str(), "node-modules" | "dist"))
        .map(|artifact| artifact.id.clone())
        .collect();
    let specs = devscan::selected_specs(&scan, &ids).expect("selected specs");
    assert_eq!(specs.len(), 2);

    let (plan, preview) = cleanup::plan_specs(specs, false).expect("preview");
    assert_eq!(preview.targets.len(), 2);
    assert!(preview
        .targets
        .iter()
        .all(|target| target.path.to_ascii_lowercase().contains("\\.git") == false));

    let quarantine = fixture.root.join("quarantine");
    let report = cleanup::execute_plan(
        &plan,
        &quarantine,
        &AtomicBool::new(false),
        &|_| {},
    )
    .expect("cleanup");
    assert!(report.quarantined > 0, "fixture files should move");
    // The cleanup engine removes files and empty child directories but keeps
    // the reviewed artifact root itself. An empty root has no files and is
    // reported as removed by verification.
    assert!(
        !app.join("node_modules/pkg/index.js").exists(),
        "node_modules files should be quarantined"
    );
    assert!(
        !app.join("dist/index.html").exists(),
        "dist files should be quarantined"
    );
    assert!(
        std::fs::read_dir(app.join("node_modules"))
            .map(|entries| entries.count() == 0)
            .unwrap_or(true),
        "the artifact root should be empty after cleanup"
    );
    assert_present(&app.join("src/index.ts"));
    assert_present(&app.join(".env"));
    assert_present(&app.join(".git/config"));

    let verification = devscan::verify(&scan, &ids).expect("verification");
    assert_eq!(verification.removed_artifacts, 2);
    assert_eq!(verification.remaining_artifacts, 0);

    let restored = cleanup::restore_batch(&quarantine, &report.run_id).expect("restore");
    assert_eq!(restored.restored, report.quarantined);
    assert_present(&app.join("src/index.ts"));
    assert_present(&app.join(".env"));
    assert_present(&app.join(".git/config"));
    assert!(
        app.join("node_modules").exists(),
        "restored node_modules should return"
    );
    assert!(app.join("dist").exists(), "restored dist should return");
}

#[test]
fn next_cache_cleanup_leaves_the_rest_of_the_build_and_source() {
    let fixture = Fixture::new();
    let app = fixture.project();
    fixture.write(
        "app/package.json",
        br#"{"name":"phase3-next","dependencies":{"next":"15.0.0"}}"#,
    );
    fixture.write("app/package-lock.json", b"{}");
    fixture.write("app/.next/cache/webpack/client.js", b"cache payload");
    fixture.write("app/.next/server/app/page.js", b"server output");
    fixture.write("app/.next/BUILD_ID", b"build-id");
    fixture.write("app/src/page.tsx", b"export default function Page() { return null; }");

    let scan = devscan::scan_selected(vec![app.to_string_lossy().to_string()]).expect("scan");
    let project = scan.projects.first().expect("fixture project");
    assert!(
        project.artifacts.iter().any(|artifact| artifact.kind == "next-cache"),
        ".next/cache must be detected"
    );
    assert!(
        !project
            .artifacts
            .iter()
            .any(|artifact| artifact.path.to_ascii_lowercase().ends_with("\\.next")),
        "the rest of .next must not be a target"
    );
    let ids: Vec<String> = project
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == "next-cache")
        .map(|artifact| artifact.id.clone())
        .collect();
    let specs = devscan::selected_specs(&scan, &ids).expect("selected cache");
    assert_eq!(specs.len(), 1);
    assert!(specs[0].path.ends_with(".next\\cache"));

    let (plan, preview) = cleanup::plan_specs(specs, false).expect("preview");
    assert_eq!(preview.targets.len(), 1);
    let quarantine = fixture.root.join("quarantine");
    let report = cleanup::execute_plan(
        &plan,
        &quarantine,
        &AtomicBool::new(false),
        &|_| {},
    )
    .expect("cleanup");
    assert!(report.quarantined > 0);
    assert!(
        !app.join(".next/cache/webpack/client.js").exists(),
        "cache files should be quarantined"
    );
    assert!(
        app.join(".next/server/app/page.js").exists(),
        "server output is not a cache and must stay"
    );
    assert_present(&app.join(".next/BUILD_ID"));
    assert_present(&app.join("src/page.tsx"));

    let verification = devscan::verify(&scan, &ids).expect("verification");
    assert_eq!(verification.removed_artifacts, 1);
    assert_eq!(verification.remaining_artifacts, 0);

    let restored = cleanup::restore_batch(&quarantine, &report.run_id).expect("restore");
    assert_eq!(restored.restored, report.quarantined);
    assert_present(&app.join(".next/cache/webpack/client.js"));
    assert_present(&app.join(".next/server/app/page.js"));
}

#[test]
fn selecting_a_protected_or_unknown_artifact_is_refused() {
    let fixture = Fixture::new();
    fixture.write("app/package.json", br#"{"name":"guard-fixture"}"#);
    fixture.write("app/node_modules/pkg/index.js", b"x");
    let scan = devscan::scan_selected(vec![fixture.project().to_string_lossy().to_string()])
        .expect("scan");

    assert!(devscan::selected_specs(&scan, &["artifact-not-present".into()]).is_err());
    assert!(devscan::selected_specs(&scan, &[]).is_err());
}
