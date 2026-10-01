use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::manifest::Capability;

/// Harness to tool worker. Text offsets in replies follow the `Span` convention (Unicode scalar values).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdapterRequest {
    pub id: u64,
    #[serde(flatten)]
    pub op: AdapterOp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AdapterOp {
    Hello,
    Detect {
        text: String,
    },
    /// Mask `text`, remembering placeholders under `session` for later unmasking.
    Mask {
        session: String,
        text: String,
    },
    Unmask {
        session: String,
        text: String,
    },
    /// Feed one streamed chunk; `is_final` tells the tool to flush anything it held back.
    UnmaskStream {
        session: String,
        chunk: String,
        is_final: bool,
    },
    Shutdown,
}

/// Tool worker to harness; `id` echoes the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AdapterResponse {
    pub id: u64,
    #[serde(flatten)]
    pub outcome: AdapterOutcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AdapterOutcome {
    /// `elapsed_ns` is measured inside the worker, so it excludes IPC overhead.
    Ok {
        elapsed_ns: u64,
        result: AdapterResult,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AdapterResult {
    Hello {
        tool_version: String,
        capabilities: Vec<Capability>,
    },
    Detect {
        entities: Vec<DetectedEntity>,
    },
    Mask {
        text: String,
    },
    Unmask {
        text: String,
    },
    /// Text safe to emit now; the tool may hold back a partial placeholder until a later chunk.
    UnmaskStream {
        text: String,
    },
    Shutdown,
}

/// An entity as the tool reports it, before mapping to `EntityType`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DetectedEntity {
    pub start: usize,
    pub end: usize,
    pub label: String,
    pub score: Option<f64>,
}
