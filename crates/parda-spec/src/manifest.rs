use std::collections::BTreeMap;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::EntityType;

/// Contents of `tools/<name>/tool.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolManifest {
    pub name: String,
    pub version: String,
    pub commit: String,
    pub license: String,
    pub language: String,
    pub homepage: String,
    pub transport: Transport,
    pub capabilities: Vec<Capability>,
    /// Tool's own label to Parda entity type; labels missing here are scored as unmapped, never dropped.
    pub entity_map: BTreeMap<String, EntityType>,
    /// Named configurations; `default` is required by the harness, `tuned` only when the tool's docs recommend one.
    pub configs: BTreeMap<String, ToolConfig>,
}

/// How the harness starts and reaches a tool. Commands run with `tools/<name>/` as working directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Transport {
    /// Long-lived worker speaking the NDJSON adapter protocol on stdin/stdout.
    Stdio { command: Vec<String> },
    /// OpenAI/Anthropic-compatible proxy; the harness sets `upstream_env` to the mock upstream URL.
    HttpProxy {
        start: Vec<String>,
        base_url: String,
        upstream_env: String,
    },
    /// Service exposing its own REST detect/mask endpoints.
    HttpApi {
        start: Vec<String>,
        base_url: String,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Detect,
    Mask,
    Unmask,
    UnmaskStream,
    Proxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolConfig {
    /// Tool-specific config file, relative to the tool directory.
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}
