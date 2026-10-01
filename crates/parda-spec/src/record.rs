//! Result files written under `results/<run-id>/`; every report number traces back to these.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::EntityType;
use crate::scenario::CheckKind;

/// `ToolRef::name` of a control run: drivers talk straight to the mock upstream, with no tool in between.
pub const CONTROL_TOOL: &str = "control";

/// `meta.json` of one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunMeta {
    pub run_id: String,
    /// RFC 3339 timestamp.
    pub started_at: String,
    pub harness_version: String,
    pub tool: ToolRef,
    pub hardware: Hardware,
    pub suite: Suite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolRef {
    pub name: String,
    pub version: String,
    pub commit: String,
    pub config: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Hardware {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cores: u32,
    pub memory_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Suite {
    Detection {
        dataset_version: String,
        dataset_sha256: String,
    },
    Restore {
        driver: String,
    },
}

/// One line of `detections.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DetectionRecord {
    pub sample_id: String,
    pub result: DetectionResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DetectionResult {
    /// `elapsed_ns` is tool-internal; `wall_ns` is measured by the harness and includes IPC.
    Detected {
        predicted: Vec<PredictedSpan>,
        elapsed_ns: u64,
        wall_ns: u64,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PredictedSpan {
    pub start: usize,
    pub end: usize,
    pub entity: PredictedEntity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PredictedEntity {
    Mapped(EntityType),
    Unmapped(String),
}

/// One line of `restore.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RestoreRecord {
    pub scenario_id: String,
    pub outcome: RestoreOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RestoreOutcome {
    Pass,
    Fail {
        failures: Vec<CheckFailure>,
        upstream_excerpt: String,
        client_excerpt: String,
    },
    NotApplicable {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckFailure {
    pub check: CheckKind,
    pub detail: String,
}
