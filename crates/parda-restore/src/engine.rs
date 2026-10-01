//! The mock upstream's scripted behaviour for one scenario, independent of HTTP.

use std::collections::BTreeMap;

use parda_spec::scenario::{Reply, Scenario, TurnInput};

use crate::align::{SlotText, align, render_upstream_chunk};

/// What the mock answers for one turn, before it is put into an API's wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedReply {
    Text {
        chunks: Vec<String>,
    },
    ToolCall {
        call_id: String,
        name: String,
        argument_chunks: Vec<String>,
    },
    HttpError {
        status: u16,
        body: String,
    },
    /// The request could not be matched to the script (tool altered literal text, unexpected turn, ...).
    MockError {
        message: String,
    },
}

/// Everything the upstream saw and learned during one scenario.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpstreamLog {
    /// Request payloads exactly as received.
    pub payloads: Vec<serde_json::Value>,
    /// Per turn, the text found in place of each slot occurrence.
    pub alignments: Vec<(usize, Vec<SlotText>)>,
    pub errors: Vec<String>,
}

impl UpstreamLog {
    /// The first placeholder seen for each slot.
    #[must_use]
    pub fn placeholders(&self) -> BTreeMap<usize, String> {
        let mut map = BTreeMap::new();
        for (slot, text) in self.alignments.iter().flat_map(|(_, a)| a) {
            map.entry(*slot).or_insert_with(|| text.clone());
        }
        map
    }
}

pub struct Engine {
    scenario: Scenario,
    log: UpstreamLog,
}

impl Engine {
    #[must_use]
    pub fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            log: UpstreamLog::default(),
        }
    }

    pub fn record_payload(&mut self, payload: serde_json::Value) {
        self.log.payloads.push(payload);
    }

    /// Learns placeholders from turn `turn`'s masked input and renders the scripted reply with them.
    pub fn reply(&mut self, turn: usize, masked_input: &str) -> ScriptedReply {
        match self.try_reply(turn, masked_input) {
            Ok(reply) => reply,
            Err(message) => {
                self.log.errors.push(format!("turn {turn}: {message}"));
                ScriptedReply::MockError { message }
            }
        }
    }

    #[must_use]
    pub fn into_log(self) -> UpstreamLog {
        self.log
    }

    fn try_reply(&mut self, turn: usize, masked_input: &str) -> Result<ScriptedReply, String> {
        let script = self.scenario.turns.get(turn).ok_or_else(|| {
            format!(
                "request for turn {turn}, but the scenario has {}",
                self.scenario.turns.len()
            )
        })?;
        let segments = match &script.input {
            TurnInput::User { message } => message,
            TurnInput::ToolResult { content, .. } => content,
        };
        let found = align(segments, masked_input, &self.scenario.pii).map_err(|e| e.to_string())?;
        self.log.alignments.push((turn, found));
        let placeholders = self.log.placeholders();
        let render = |chunks: &[Vec<_>]| -> Result<Vec<String>, String> {
            chunks
                .iter()
                .map(|c| {
                    render_upstream_chunk(c, &placeholders)
                        .map_err(|slot| format!("no placeholder for slot {slot}"))
                })
                .collect()
        };
        Ok(match &script.reply {
            Reply::Text { chunks } => ScriptedReply::Text {
                chunks: render(chunks)?,
            },
            Reply::ToolCall {
                call_id,
                name,
                argument_chunks,
            } => ScriptedReply::ToolCall {
                call_id: call_id.clone(),
                name: name.clone(),
                argument_chunks: render(argument_chunks)?,
            },
            Reply::HttpError { status, body } => ScriptedReply::HttpError {
                status: *status,
                body: body.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario() -> Scenario {
        toml::from_str(
            r#"
id = "s"
category = "multi_turn"
description = "d"
api = "openai_chat"
stream = true
pii = ["Ananya Iyer"]
checks = ["restored_exact"]
[[turns]]
input = { kind = "user", message = [{ text = "I am " }, { slot = 0 }] }
reply = { kind = "text", chunks = [[{ text = "Hi " }, { slot = 0, to = 4 }], [{ slot = 0, from = 4 }, { text = "!" }]] }
[[turns]]
input = { kind = "user", message = [{ text = "Again: " }, { slot = 0 }] }
reply = { kind = "text", chunks = [[{ slot = 0, mangle = "uppercase" }]] }
"#,
        )
        .unwrap()
    }

    #[test]
    fn learns_placeholder_and_renders_split_reply() {
        let mut e = Engine::new(scenario());
        assert_eq!(
            e.reply(0, "I am <person_1>"),
            ScriptedReply::Text {
                chunks: vec!["Hi <per".to_owned(), "son_1>!".to_owned()]
            }
        );
        assert_eq!(
            e.reply(1, "Again: <person_1>"),
            ScriptedReply::Text {
                chunks: vec!["<PERSON_1>".to_owned()]
            }
        );
        let log = e.into_log();
        assert_eq!(log.alignments.len(), 2);
        assert!(log.errors.is_empty());
    }

    #[test]
    fn first_placeholder_wins_when_a_tool_changes_it() {
        let mut e = Engine::new(scenario());
        e.reply(0, "I am <P1>");
        assert_eq!(
            e.reply(1, "Again: <P2>"),
            ScriptedReply::Text {
                chunks: vec!["<P1>".to_owned()]
            }
        );
        assert_eq!(e.into_log().placeholders()[&0], "<P1>");
    }

    #[test]
    fn misaligned_input_and_extra_turns_are_mock_errors() {
        let mut e = Engine::new(scenario());
        assert!(matches!(
            e.reply(0, "Changed text"),
            ScriptedReply::MockError { .. }
        ));
        assert!(matches!(
            e.reply(5, "I am X"),
            ScriptedReply::MockError { .. }
        ));
        assert_eq!(e.into_log().errors.len(), 2);
    }
}
