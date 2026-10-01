use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use parda_spec::manifest::Capability;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("reading {path}: {source}")]
    ManifestRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid manifest {path}: {message}")]
    Manifest { path: PathBuf, message: String },
    #[error("starting `{command}` in {dir}: {source}")]
    Spawn {
        command: String,
        dir: PathBuf,
        source: std::io::Error,
    },
    #[error("worker I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("worker exited before replying to request {request_id}")]
    WorkerExited { request_id: u64 },
    #[error("worker sent a line that is not a valid adapter response ({source}): {line}")]
    BadResponse {
        line: String,
        source: serde_json::Error,
    },
    #[error("worker replied to request {got}, expected {expected}")]
    IdMismatch { expected: u64, got: u64 },
    #[error("worker replied with `{got}` to a `{expected}` request")]
    OpMismatch { expected: &'static str, got: String },
    #[error("no reply to request {request_id} within {timeout:?}")]
    Timeout { request_id: u64, timeout: Duration },
    #[error("tool reported an error: {0}")]
    Tool(String),
    #[error("worker capabilities {reported:?} differ from manifest {declared:?}")]
    CapabilityMismatch {
        declared: BTreeSet<Capability>,
        reported: BTreeSet<Capability>,
    },
    #[error("{url} did not accept connections within {timeout:?}")]
    NotReady { url: String, timeout: Duration },
    #[error("HTTP request to {url}: {source}")]
    Http { url: String, source: reqwest::Error },
    #[error("tool uses transport `{0}`, which cannot run this operation")]
    WrongTransport(&'static str),
}
