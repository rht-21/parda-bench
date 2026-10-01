#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] functions"
)]

use std::path::PathBuf;
use std::time::Duration;

use parda_adapters::detect::DetectionRunner;
use parda_adapters::process::Output;
use parda_adapters::{Adapter, AdapterError, Tool};
use parda_spec::entity::EntityType;
use parda_spec::protocol::adapter::AdapterOp;
use parda_spec::record::{DetectionRecord, DetectionResult, PredictedEntity};
use parda_spec::sample::{Difficulty, Labels, Lang, Sample, Source};

fn fake_tool(config: &str) -> Tool {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-tool");
    Tool::load(&dir, config).unwrap()
}

fn sample(text: &str) -> Sample {
    Sample {
        id: text.to_owned(),
        text: text.to_owned(),
        labels: Labels::Positive { spans: vec![] },
        lang: Lang::En,
        difficulty: Difficulty::Easy,
        source: Source::Handwritten,
        generator_version: "test".to_owned(),
    }
}

fn is_ok_detection(record: &DetectionRecord) -> bool {
    matches!(record.result, DetectionResult::Detected { .. })
}

#[tokio::test]
async fn detects_and_maps_labels_with_both_latencies() {
    let tool = fake_tool("default");
    let mut runner = DetectionRunner::start(&tool, Output::Inherit)
        .await
        .unwrap();
    let record = runner.detect(&sample("Rahul")).await.unwrap();
    let DetectionResult::Detected {
        predicted,
        elapsed_ns,
        wall_ns,
    } = record.result
    else {
        panic!("expected a detection, got {record:?}");
    };
    assert_eq!(
        predicted[0].entity,
        PredictedEntity::Mapped(EntityType::PersonName)
    );
    assert_eq!(
        predicted[1].entity,
        PredictedEntity::Unmapped("INITIAL".to_owned())
    );
    assert!(wall_ns >= elapsed_ns);
    runner.shutdown().await;
}

#[tokio::test]
async fn tool_error_is_recorded_and_run_continues() {
    let tool = fake_tool("default");
    let mut runner = DetectionRunner::start(&tool, Output::Inherit)
        .await
        .unwrap();
    let failed = runner.detect(&sample("error")).await.unwrap();
    assert_eq!(
        failed.result,
        DetectionResult::Failed {
            message: "RuntimeError: tool rejected input".to_owned()
        }
    );
    assert!(is_ok_detection(
        &runner.detect(&sample("next")).await.unwrap()
    ));
    runner.shutdown().await;
}

#[tokio::test]
async fn crashed_worker_is_restarted_for_the_next_sample() {
    let tool = fake_tool("default");
    let mut runner = DetectionRunner::start(&tool, Output::Inherit)
        .await
        .unwrap();
    assert!(!is_ok_detection(
        &runner.detect(&sample("crash")).await.unwrap()
    ));
    assert!(is_ok_detection(
        &runner.detect(&sample("after crash")).await.unwrap()
    ));
    runner.shutdown().await;
}

#[tokio::test]
async fn garbled_reply_is_a_failure_and_worker_is_restarted() {
    let tool = fake_tool("default");
    let mut runner = DetectionRunner::start(&tool, Output::Inherit)
        .await
        .unwrap();
    let garbled = runner.detect(&sample("garbage")).await.unwrap();
    let DetectionResult::Failed { message } = garbled.result else {
        panic!("expected failure")
    };
    assert!(
        message.contains("not a valid adapter response"),
        "{message}"
    );
    assert!(is_ok_detection(
        &runner.detect(&sample("after garbage")).await.unwrap()
    ));
    runner.shutdown().await;
}

#[tokio::test]
async fn slow_reply_times_out() {
    let tool = fake_tool("default");
    let mut adapter = Adapter::start(&tool, &Output::Inherit).await.unwrap();
    let err = adapter
        .call(
            AdapterOp::Detect {
                text: "hang".to_owned(),
            },
            Duration::from_millis(300),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AdapterError::Timeout { .. }), "{err}");
}

#[tokio::test]
async fn capabilities_differing_from_manifest_fail_the_handshake() {
    let tool = fake_tool("overclaims");
    let err = Adapter::start(&tool, &Output::Inherit).await.err().unwrap();
    assert!(
        matches!(err, AdapterError::CapabilityMismatch { .. }),
        "{err}"
    );
}
