use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One restore test, authored as `scenarios/<category>/<id>.toml`.
///
/// `pii` holds the real values; segments refer to them by index (`slot`). The mock upstream learns each slot's
/// placeholder by aligning the masked request it receives against the input's literal segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub category: Category,
    pub description: String,
    pub api: Api,
    pub stream: bool,
    pub pii: Vec<String>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    pub turns: Vec<Turn>,
    pub checks: Vec<CheckKind>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Basic,
    StreamingSplit,
    ToolCalls,
    MultiTurn,
    PlaceholderMangling,
    RepeatsAdjacency,
    Unicode,
    StructuredOutput,
    FailureBehaviour,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Api {
    OpenaiChat,
    AnthropicMessages,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    pub input: TurnInput,
    pub reply: Reply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TurnInput {
    User {
        message: Vec<InputSegment>,
    },
    ToolResult {
        call_id: String,
        content: Vec<InputSegment>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum InputSegment {
    Literal { text: String },
    Slot { slot: usize },
}

/// What the mock upstream sends back. Each inner `Vec<ReplySegment>` is one SSE chunk when streaming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Text {
        chunks: Vec<Vec<ReplySegment>>,
    },
    ToolCall {
        call_id: String,
        name: String,
        argument_chunks: Vec<Vec<ReplySegment>>,
    },
    HttpError {
        status: u16,
        body: String,
    },
}

/// Literal text, or (part of) the placeholder the tool chose for `slot`, optionally altered the way models do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum ReplySegment {
    Literal {
        text: String,
    },
    Slot {
        slot: usize,
        /// Char range of the placeholder to emit; omitted bounds mean start/end of the placeholder.
        from: Option<usize>,
        to: Option<usize>,
        mangle: Option<Mangle>,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Mangle {
    Lowercase,
    Uppercase,
    StripDelimiters,
    SpacePadded,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// No `pii` value reaches the upstream, verbatim or with separators and case changed.
    NoLeakUpstream,
    /// The client sees the scripted reply with every placeholder replaced by its real value.
    RestoredExact,
    /// The client sees no whole or partial placeholder.
    NoPlaceholderFragment,
    /// A value gets the same placeholder every time it is sent, within and across turns.
    PlaceholderStableAcrossTurns,
    /// Tool-call arguments, and text replies that are JSON as scripted, are still valid JSON after restoring.
    JsonValid,
    /// An upstream error reaches the client as an error with the same status.
    FailsClosed,
}
