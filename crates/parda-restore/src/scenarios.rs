//! Loading `scenarios/<category>/<id>.toml` and rejecting scenarios the mock could not run.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use parda_spec::scenario::{CheckKind, InputSegment, Reply, ReplySegment, Scenario, TurnInput};

#[derive(Debug, thiserror::Error)]
#[error("{path}: {message}")]
pub struct ScenarioError {
    pub path: PathBuf,
    pub message: String,
}

/// All scenarios under `root`, sorted by id.
///
/// # Errors
/// Fails on unreadable files, invalid TOML, or a scenario that breaks a rule in `check`.
pub fn load_dir(root: &Path) -> Result<Vec<Scenario>, ScenarioError> {
    let err = |path: &Path, message: String| ScenarioError {
        path: path.to_owned(),
        message,
    };
    let mut scenarios = Vec::new();
    for category_dir in sorted_entries(root).map_err(|e| err(root, e.to_string()))? {
        if !category_dir.is_dir() {
            continue;
        }
        for path in sorted_entries(&category_dir).map_err(|e| err(&category_dir, e.to_string()))? {
            if path.extension().is_none_or(|e| e != "toml") {
                continue;
            }
            let source = fs::read_to_string(&path).map_err(|e| err(&path, e.to_string()))?;
            let scenario: Scenario =
                toml::from_str(&source).map_err(|e| err(&path, e.to_string()))?;
            check_location(&scenario, &path).map_err(|m| err(&path, m))?;
            check(&scenario).map_err(|m| err(&path, m))?;
            scenarios.push(scenario);
        }
    }
    scenarios.sort_by(|a, b| a.id.cmp(&b.id));
    if let Some(pair) = scenarios.windows(2).find(|w| w[0].id == w[1].id) {
        return Err(err(root, format!("duplicate scenario id {}", pair[0].id)));
    }
    Ok(scenarios)
}

/// Rules that make a scenario runnable and its expectations well defined.
///
/// # Errors
/// Returns the first rule the scenario breaks.
pub fn check(s: &Scenario) -> Result<(), String> {
    if s.turns.is_empty() || s.checks.is_empty() || s.pii.is_empty() {
        return Err("a scenario needs at least one turn, one check and one pii value".to_owned());
    }
    if let Some(i) = s.pii.iter().position(|v| v.trim().is_empty()) {
        return Err(format!("pii[{i}] is empty"));
    }
    let mut introduced = BTreeSet::new();
    let mut open_call: Option<&str> = None;
    for (t, turn) in s.turns.iter().enumerate() {
        let segments = match &turn.input {
            TurnInput::User { message } => message,
            TurnInput::ToolResult { call_id, content } => {
                if open_call != Some(call_id.as_str()) {
                    return Err(format!(
                        "turn {t}: tool result {call_id} does not answer the previous turn's call"
                    ));
                }
                content
            }
        };
        check_input(segments, s.pii.len()).map_err(|m| format!("turn {t}: {m}"))?;
        introduced.extend(segments.iter().filter_map(|seg| match seg {
            InputSegment::Slot { slot } => Some(*slot),
            InputSegment::Literal { .. } => None,
        }));
        open_call = None;
        let chunks = match &turn.reply {
            Reply::Text { chunks } => chunks,
            Reply::ToolCall {
                call_id,
                name,
                argument_chunks,
            } => {
                if !s.tools.iter().any(|tool| &tool.name == name) {
                    return Err(format!(
                        "turn {t}: tool call to {name}, which is not in `tools`"
                    ));
                }
                open_call = Some(call_id);
                argument_chunks
            }
            Reply::HttpError { status, .. } if !(400..=599).contains(status) => {
                return Err(format!(
                    "turn {t}: http_error status {status} is not a 4xx or 5xx code"
                ));
            }
            Reply::HttpError { .. } => continue,
        };
        for slot in chunks.iter().flatten().filter_map(reply_slot) {
            if !introduced.contains(&slot) {
                return Err(format!(
                    "turn {t}: reply uses slot {slot} before any input contains it"
                ));
            }
        }
    }
    let has_error_reply = s
        .turns
        .iter()
        .any(|t| matches!(t.reply, Reply::HttpError { .. }));
    if s.checks.contains(&CheckKind::FailsClosed) != has_error_reply {
        return Err("`fails_closed` goes with, and only with, an `http_error` reply".to_owned());
    }
    Ok(())
}

fn check_input(segments: &[InputSegment], pii_len: usize) -> Result<(), String> {
    let mut previous_was_slot = false;
    for seg in segments {
        match seg {
            InputSegment::Slot { slot } if *slot >= pii_len => {
                return Err(format!("slot {slot} has no pii value"));
            }
            InputSegment::Slot { .. } if previous_was_slot => {
                return Err(
                    "two slots need literal text between them for the mock to tell them apart"
                        .to_owned(),
                );
            }
            InputSegment::Slot { .. } => previous_was_slot = true,
            InputSegment::Literal { text } => previous_was_slot &= text.is_empty(),
        }
    }
    Ok(())
}

fn reply_slot(seg: &ReplySegment) -> Option<usize> {
    match seg {
        ReplySegment::Slot { slot, .. } => Some(*slot),
        ReplySegment::Literal { .. } => None,
    }
}

fn check_location(s: &Scenario, path: &Path) -> Result<(), String> {
    let stem = path
        .file_stem()
        .and_then(|f| f.to_str())
        .unwrap_or_default();
    if stem != s.id {
        return Err(format!("id {} does not match file name {stem}", s.id));
    }
    let dir = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|d| d.to_str())
        .unwrap_or_default();
    let category = serde_json::to_value(s.category).map_err(|e| e.to_string())?;
    if category.as_str() != Some(dir) {
        return Err(format!(
            "category {category} does not match directory {dir}"
        ));
    }
    Ok(())
}

fn sorted_entries(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> Scenario {
        toml::from_str(toml_src).unwrap()
    }

    const BASIC: &str = r#"
id = "s"
category = "basic"
description = "d"
api = "openai_chat"
stream = false
pii = ["ABCPS1234K"]
checks = ["no_leak_upstream", "restored_exact"]

[[turns]]
[turns.input]
kind = "user"
message = [{ text = "PAN " }, { slot = 0 }]
[turns.reply]
kind = "text"
chunks = [[{ text = "Got " }, { slot = 0, to = 3 }], [{ slot = 0, from = 3, mangle = "lowercase" }]]
"#;

    #[test]
    fn accepts_a_valid_scenario_and_parses_fragments() {
        let s = parse(BASIC);
        check(&s).unwrap();
        let Reply::Text { chunks } = &s.turns[0].reply else {
            panic!()
        };
        assert!(matches!(
            chunks[1][0],
            ReplySegment::Slot {
                from: Some(3),
                to: None,
                ..
            }
        ));
    }

    #[test]
    fn rejects_slot_without_value() {
        assert!(check(&parse(&BASIC.replace("{ slot = 0 }]", "{ slot = 1 }]"))).is_err());
    }

    #[test]
    fn rejects_adjacent_input_slots() {
        let s = parse(
            &BASIC
                .replace(r#"pii = ["ABCPS1234K"]"#, r#"pii = ["A", "B"]"#)
                .replace(
                    r#"message = [{ text = "PAN " }, { slot = 0 }]"#,
                    r"message = [{ slot = 0 }, { slot = 1 }]",
                ),
        );
        assert!(check(&s).is_err());
    }

    #[test]
    fn rejects_fails_closed_without_error_reply() {
        assert!(
            check(&parse(
                &BASIC.replace("\"restored_exact\"]", "\"fails_closed\"]")
            ))
            .is_err()
        );
    }

    #[test]
    fn rejects_reply_slot_never_seen_in_input() {
        let s = parse(
            &BASIC
                .replace(r#"pii = ["ABCPS1234K"]"#, r#"pii = ["A", "B"]"#)
                .replace(
                    r#"[{ text = "Got " }, { slot = 0, to = 3 }]"#,
                    r#"[{ text = "Got " }, { slot = 1 }]"#,
                ),
        );
        assert!(check(&s).is_err());
    }
}
