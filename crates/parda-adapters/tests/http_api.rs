#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] functions"
)]

use std::path::PathBuf;

use parda_adapters::process::Output;
use parda_adapters::{Adapter, AdapterError, Tool};

fn fake_http_tool() -> Tool {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-http-tool");
    Tool::load(&dir, "default").unwrap()
}

#[tokio::test]
async fn http_api_tool_detects_reports_errors_and_stops_with_its_handle() {
    let tool = fake_http_tool();
    let mut adapter = Adapter::start(&tool, &Output::Inherit).await.unwrap();

    let (entities, reply) = adapter.detect("Rahul").await.unwrap();
    assert_eq!(
        (
            entities[0].start,
            entities[0].end,
            entities[0].label.as_str()
        ),
        (0, 5, "WORD")
    );
    assert_eq!(reply.elapsed_ns, 1000);
    assert!(reply.wall_ns > 0);

    let err = adapter.detect("error").await.unwrap_err();
    assert!(
        matches!(err, AdapterError::Tool(ref m) if m == "tool rejected input"),
        "{err}"
    );

    adapter.shutdown().await;
    let refused = tokio::net::TcpStream::connect(("127.0.0.1", 18432)).await;
    assert!(refused.is_err(), "service still listening after shutdown");
}
