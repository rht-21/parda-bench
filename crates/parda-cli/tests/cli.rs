#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] functions"
)]

use std::path::Path;
use std::process::{Command, Output};

fn bench(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_parda-bench"))
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap()
}

#[test]
fn builds_validates_and_describes_a_dataset() {
    let dir = tempfile::tempdir().unwrap();
    let built = bench(
        &["data", "build", "--per-template", "2", "--version", "vtest"],
        dir.path(),
    );
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let valid = bench(&["data", "validate", "data/vtest"], dir.path());
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stdout)
    );
    let stats = bench(&["data", "stats", "data/vtest"], dir.path());
    let json: serde_json::Value = serde_json::from_slice(&stats.stdout).unwrap();
    assert_eq!(
        json["samples"],
        json["positive_samples"].as_u64().unwrap() + json["hard_negatives"].as_u64().unwrap()
    );
}

#[test]
fn rebuilding_with_different_content_under_the_same_version_fails() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        bench(
            &["data", "build", "--per-template", "1", "--version", "v1"],
            dir.path()
        )
        .status
        .success()
    );
    assert!(
        bench(
            &["data", "build", "--per-template", "1", "--version", "v1"],
            dir.path()
        )
        .status
        .success()
    );
    let changed = bench(
        &[
            "data",
            "build",
            "--per-template",
            "1",
            "--seed",
            "7",
            "--version",
            "v1",
        ],
        dir.path(),
    );
    assert!(!changed.status.success());
}

#[test]
fn tampered_dataset_fails_validation() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        bench(
            &["data", "build", "--per-template", "1", "--version", "v1"],
            dir.path()
        )
        .status
        .success()
    );
    let samples = dir.path().join("data/v1/samples.jsonl");
    let text = std::fs::read_to_string(&samples).unwrap();
    std::fs::write(&samples, text.replacen("Aadhaar", "Aadhar", 1)).unwrap();
    assert!(
        !bench(&["data", "validate", "data/v1"], dir.path())
            .status
            .success()
    );
}

#[test]
fn report_on_empty_results_says_so() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("results")).unwrap();
    assert!(bench(&["report"], dir.path()).status.success());
    let md = std::fs::read_to_string(dir.path().join("reports/report.md")).unwrap();
    assert!(md.contains("No runs found."));
}
