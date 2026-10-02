use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::EntityType;

/// A labeled region of `Sample::text`.
///
/// Offsets count Unicode scalar values (Rust `char`, Python `str` index), end exclusive — not bytes, not UTF-16 units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub entity: EntityType,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Lang {
    #[serde(rename = "en")]
    En,
    #[serde(rename = "hi-Latn")]
    HiLatn,
    #[serde(rename = "hi-Deva")]
    HiDeva,
    #[serde(rename = "mixed")]
    Mixed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Template,
    Llm,
    Handwritten,
}

/// Gold labels: either real entities, or a decoy that looks like one but must not be flagged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Labels {
    Positive { spans: Vec<Span> },
    HardNegative { decoy: EntityType },
}

/// One JSONL record of the dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub id: String,
    pub text: String,
    pub labels: Labels,
    pub lang: Lang,
    pub difficulty: Difficulty,
    pub source: Source,
    pub generator_version: String,
}
