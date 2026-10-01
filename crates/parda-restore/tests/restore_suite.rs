#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] functions"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use parda_adapters::Tool;
use parda_adapters::process::Output;
use parda_restore::driver::{Driver, RAW_HTTP};
use parda_restore::runner::{ToolRole, role, run_control, run_library, run_proxy};
use parda_restore::scenarios::load_dir;
use parda_spec::record::{RestoreOutcome, RestoreRecord};
use parda_spec::scenario::{Category, CheckKind, Scenario};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scenarios() -> Vec<Scenario> {
    load_dir(&repo().join("scenarios")).unwrap()
}

fn raw_http() -> Driver {
    Driver::open(RAW_HTTP, &repo().join("drivers")).unwrap()
}

fn by_id(records: Vec<RestoreRecord>) -> BTreeMap<String, RestoreOutcome> {
    records
        .into_iter()
        .map(|r| (r.scenario_id, r.outcome))
        .collect()
}

fn failed_checks(outcome: &RestoreOutcome) -> Vec<CheckKind> {
    match outcome {
        RestoreOutcome::Fail { failures, .. } => failures.iter().map(|f| f.check).collect(),
        RestoreOutcome::Pass | RestoreOutcome::NotApplicable { .. } => vec![],
    }
}

#[test]
fn repository_scenarios_cover_every_category() {
    let s = scenarios();
    let categories: std::collections::BTreeSet<_> = s.iter().map(|s| s.category).collect();
    assert_eq!(categories.len(), 9);
    assert!(s.len() >= 30);
}

/// With no tool in the path every value reaches the upstream unmasked, so the leak check is the only one that
/// fails; mangling scenarios are not applicable to a control run.
#[tokio::test]
async fn control_run_fails_only_the_leak_check() {
    let scenarios = scenarios();
    let outcomes = by_id(run_control(&mut raw_http(), &scenarios).await.unwrap());
    for s in &scenarios {
        let outcome = &outcomes[&s.id];
        if s.category == Category::PlaceholderMangling {
            assert!(
                matches!(outcome, RestoreOutcome::NotApplicable { .. }),
                "{}",
                s.id
            );
            continue;
        }
        let expected = if s.checks.contains(&CheckKind::NoLeakUpstream) {
            vec![CheckKind::NoLeakUpstream; s.pii.len()]
        } else {
            vec![]
        };
        assert_eq!(failed_checks(outcome), expected, "{}: {outcome:?}", s.id);
    }
}

#[tokio::test]
async fn library_run_with_regex_baseline() {
    let tool = Tool::load(&repo().join("tools/regex-baseline"), "default").unwrap();
    assert_eq!(role(&tool), ToolRole::Library);
    let outcomes = by_id(
        run_library(&tool, &scenarios(), &Output::Inherit)
            .await
            .unwrap(),
    );
    for id in [
        "basic-openai-aadhaar",
        "split-openai-two-chunks",
        "split-openai-first-char",
        "tools-openai-stream-args-split",
    ] {
        assert_eq!(outcomes[id], RestoreOutcome::Pass, "{id}");
    }
    assert_eq!(
        failed_checks(&outcomes["mangle-lowercase"]),
        vec![CheckKind::RestoredExact, CheckKind::NoPlaceholderFragment]
    );
    assert_eq!(
        failed_checks(&outcomes["basic-openai-contact"]),
        vec![CheckKind::NoLeakUpstream]
    );
    assert!(matches!(
        outcomes["fail-openai-500"],
        RestoreOutcome::NotApplicable { .. }
    ));
}

#[tokio::test]
async fn proxy_run_with_regex_proxy() {
    let tool = Tool::load(&repo().join("tools/regex-proxy"), "default").unwrap();
    assert_eq!(role(&tool), ToolRole::Proxy);
    let outcomes = by_id(
        run_proxy(&tool, &mut raw_http(), &scenarios(), &Output::Inherit)
            .await
            .unwrap(),
    );
    for id in [
        "basic-openai-aadhaar",
        "basic-anthropic-pan",
        "split-anthropic-two-values",
        "tools-openai-call-args",
        "tools-anthropic-stream-args",
        "multi-anthropic-repeat-value",
        "fail-openai-500",
        "fail-anthropic-529",
    ] {
        assert_eq!(outcomes[id], RestoreOutcome::Pass, "{id}");
    }
    assert!(
        failed_checks(&outcomes["tools-anthropic-roundtrip"]).contains(&CheckKind::NoLeakUpstream)
    );
}
