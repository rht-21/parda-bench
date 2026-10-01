use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::scenario::{Api, ToolDefinition};

/// Harness to client-framework driver: run one conversation against `base_url` with real (unmasked) values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DriverRequest {
    pub id: u64,
    pub base_url: String,
    pub api: Api,
    pub model: String,
    pub stream: bool,
    pub tools: Vec<ToolDefinition>,
    pub turns: Vec<DriverTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriverTurn {
    User { text: String },
    ToolResult { call_id: String, content: String },
}

/// Driver to harness: what the application saw, one observation per turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriverResponse {
    pub id: u64,
    pub observations: Vec<Observation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    Completed {
        text: String,
        tool_calls: Vec<ObservedToolCall>,
    },
    Errored {
        message: String,
        http_status: Option<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservedToolCall {
    pub id: String,
    pub name: String,
    /// Raw JSON string exactly as the client library surfaced it.
    pub arguments: String,
}
