//! Restore checks: compares what the upstream received and what the client saw against the scenario.

use std::collections::{BTreeMap, BTreeSet};

use parda_spec::protocol::driver::Observation;
use parda_spec::record::{CheckFailure, RestoreOutcome};
use parda_spec::scenario::{CheckKind, Reply, Scenario};
use serde_json::Value;

use crate::align::{core, render_expected};
use crate::engine::UpstreamLog;

const EXCERPT_CHARS: usize = 600;
/// Placeholder fragments shorter than this are too common in ordinary text to count.
const MIN_FRAGMENT_CHARS: usize = 3;
/// Values with at least this many letters and digits are also matched with separators removed.
const MIN_NORMALIZED_CHARS: usize = 6;

/// What the client should observe for one turn.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expected {
    Text(String),
    ToolCall { name: String, arguments: String },
    Error { status: u16 },
}

/// Scores one scenario run.
#[must_use]
pub fn evaluate(
    scenario: &Scenario,
    log: &UpstreamLog,
    observations: &[Observation],
) -> RestoreOutcome {
    let expected = expected_turns(scenario);
    let mut failures: Vec<CheckFailure> = log
        .errors
        .iter()
        .map(|e| {
            failure(
                CheckKind::RestoredExact,
                format!("mock upstream could not follow the script: {e}"),
            )
        })
        .collect();
    if observations.len() != expected.len() {
        failures.push(failure(
            CheckKind::RestoredExact,
            format!(
                "client reported {} turns, scenario has {}",
                observations.len(),
                expected.len()
            ),
        ));
    }
    for check in &scenario.checks {
        let details = match check {
            CheckKind::NoLeakUpstream => leaks(&scenario.pii, &log.payloads),
            CheckKind::RestoredExact => restore_mismatches(&expected, observations),
            CheckKind::NoPlaceholderFragment => {
                fragments(&expected, observations, &log.placeholders(), &scenario.pii)
            }
            CheckKind::PlaceholderStableAcrossTurns => unstable_placeholders(log),
            CheckKind::JsonValid => invalid_json(&expected, observations),
            CheckKind::FailsClosed => open_failures(&expected, observations),
        };
        failures.extend(details.into_iter().map(|d| failure(*check, d)));
    }
    if failures.is_empty() {
        RestoreOutcome::Pass
    } else {
        RestoreOutcome::Fail {
            failures,
            upstream_excerpt: excerpt(
                &log.payloads
                    .last()
                    .map(Value::to_string)
                    .unwrap_or_default(),
            ),
            client_excerpt: excerpt(&format!("{observations:?}")),
        }
    }
}

fn failure(check: CheckKind, detail: String) -> CheckFailure {
    CheckFailure { check, detail }
}

fn expected_turns(scenario: &Scenario) -> Vec<Expected> {
    scenario
        .turns
        .iter()
        .map(|t| match &t.reply {
            Reply::Text { chunks } => Expected::Text(render_expected(chunks, &scenario.pii)),
            Reply::ToolCall {
                name,
                argument_chunks,
                ..
            } => Expected::ToolCall {
                name: name.clone(),
                arguments: render_expected(argument_chunks, &scenario.pii),
            },
            Reply::HttpError { status, .. } => Expected::Error { status: *status },
        })
        .collect()
}

fn leaks(pii: &[String], payloads: &[Value]) -> Vec<String> {
    let mut strings = Vec::new();
    for p in payloads {
        collect_strings(p, &mut strings);
    }
    let normalized: Vec<String> = strings.iter().map(|s| alnum(s)).collect();
    pii.iter()
        .enumerate()
        .filter(|(_, value)| {
            let norm = alnum(value);
            strings.iter().any(|s| s.contains(value.as_str()))
                || (norm.chars().count() >= MIN_NORMALIZED_CHARS
                    && normalized.iter().any(|s| s.contains(&norm)))
        })
        .map(|(i, value)| format!("upstream received pii[{i}] {}", quote(value)))
        .collect()
}

fn collect_strings<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(items) => items.iter().for_each(|i| collect_strings(i, out)),
        Value::Object(map) => map.values().for_each(|i| collect_strings(i, out)),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn alnum(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn restore_mismatches(expected: &[Expected], observations: &[Observation]) -> Vec<String> {
    let mut out = Vec::new();
    for (turn, (exp, obs)) in expected.iter().zip(observations).enumerate() {
        match (exp, obs) {
            (Expected::Text(want), Observation::Completed { text, .. }) if text != want => {
                out.push(format!(
                    "turn {turn}: expected {}, client saw {}",
                    quote(want),
                    quote(text)
                ));
            }
            (Expected::ToolCall { name, arguments }, Observation::Completed { tool_calls, .. }) => {
                match tool_calls.as_slice() {
                    [call] if &call.name == name && same_json(&call.arguments, arguments) => {}
                    calls => out.push(format!(
                        "turn {turn}: expected call {name}({arguments}), client saw {calls:?}"
                    )),
                }
            }
            (
                Expected::Text(_) | Expected::ToolCall { .. },
                Observation::Errored {
                    message,
                    http_status,
                },
            ) => {
                out.push(format!(
                    "turn {turn}: client got an error ({http_status:?}): {message}"
                ));
            }
            _ => {}
        }
    }
    out
}

/// JSON-equal when both sides parse (key order and whitespace may change in transit), byte-equal otherwise.
fn same_json(a: &str, b: &str) -> bool {
    match (
        serde_json::from_str::<Value>(a),
        serde_json::from_str::<Value>(b),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn observed_texts(obs: &Observation) -> Vec<&str> {
    match obs {
        Observation::Completed { text, tool_calls } => std::iter::once(text.as_str())
            .chain(tool_calls.iter().map(|c| c.arguments.as_str()))
            .collect(),
        Observation::Errored { message, .. } => vec![message.as_str()],
    }
}

fn fragments(
    expected: &[Expected],
    observations: &[Observation],
    placeholders: &BTreeMap<usize, String>,
    pii: &[String],
) -> Vec<String> {
    let masked: Vec<&String> = placeholders
        .iter()
        .filter(|(slot, p)| pii.get(**slot) != Some(*p))
        .map(|(_, p)| p)
        .collect();
    let mut out = Vec::new();
    for (turn, (exp, obs)) in expected.iter().zip(observations).enumerate() {
        let allowed = match exp {
            Expected::Text(t) => t.to_lowercase(),
            Expected::ToolCall { arguments, .. } => arguments.to_lowercase(),
            Expected::Error { .. } => String::new(),
        };
        for seen in observed_texts(obs) {
            let seen = seen.to_lowercase();
            for placeholder in &masked {
                if let Some(fragment) = placeholder_fragments(placeholder)
                    .into_iter()
                    .find(|f| seen.contains(f) && !allowed.contains(f))
                {
                    out.push(format!(
                        "turn {turn}: client saw {} from placeholder {}",
                        quote(&fragment),
                        quote(placeholder)
                    ));
                }
            }
        }
    }
    out
}

/// Lowercased pieces that betray a placeholder: the whole thing, its core, and each half.
fn placeholder_fragments(placeholder: &str) -> Vec<String> {
    let chars: Vec<char> = placeholder.to_lowercase().chars().collect();
    let half = chars.len().div_ceil(2);
    let candidates = [
        chars.iter().collect::<String>(),
        core(&placeholder.to_lowercase()).2.to_owned(),
        chars[..half].iter().collect(),
        chars[chars.len() - half..].iter().collect(),
    ];
    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|c| c.chars().count() >= MIN_FRAGMENT_CHARS && c.chars().any(char::is_alphanumeric))
        .filter(|c| seen.insert(c.clone()))
        .collect()
}

fn unstable_placeholders(log: &UpstreamLog) -> Vec<String> {
    let mut by_slot: BTreeMap<usize, BTreeSet<&str>> = BTreeMap::new();
    for (slot, text) in log.alignments.iter().flat_map(|(_, a)| a) {
        by_slot.entry(*slot).or_default().insert(text);
    }
    by_slot
        .into_iter()
        .filter(|(_, seen)| seen.len() > 1)
        .map(|(slot, seen)| format!("slot {slot} was replaced inconsistently: {seen:?}"))
        .collect()
}

fn invalid_json(expected: &[Expected], observations: &[Observation]) -> Vec<String> {
    let mut out = Vec::new();
    for (turn, (exp, obs)) in expected.iter().zip(observations).enumerate() {
        let Observation::Completed { text, tool_calls } = obs else {
            continue;
        };
        let candidates: Vec<&str> = match exp {
            Expected::Text(want) if serde_json::from_str::<Value>(want).is_ok() => {
                vec![text.as_str()]
            }
            Expected::ToolCall { .. } => tool_calls.iter().map(|c| c.arguments.as_str()).collect(),
            Expected::Text(_) | Expected::Error { .. } => vec![],
        };
        for c in candidates {
            if let Err(e) = serde_json::from_str::<Value>(c) {
                out.push(format!("turn {turn}: not valid JSON ({e}): {}", quote(c)));
            }
        }
    }
    out
}

fn open_failures(expected: &[Expected], observations: &[Observation]) -> Vec<String> {
    let mut out = Vec::new();
    for (turn, (exp, obs)) in expected.iter().zip(observations).enumerate() {
        let Expected::Error { status } = exp else {
            continue;
        };
        match obs {
            Observation::Errored {
                http_status: Some(got),
                ..
            } if got != status => {
                out.push(format!(
                    "turn {turn}: upstream failed with {status}, client saw status {got}"
                ));
            }
            Observation::Errored { .. } => {}
            Observation::Completed { text, .. } => {
                out.push(format!(
                    "turn {turn}: upstream failed with {status}, client saw a completion {}",
                    quote(text)
                ));
            }
        }
    }
    out
}

/// Quotes text for a failure detail without escaping non-ASCII (Rust's `{:?}` would mangle Devanagari).
fn quote(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

fn excerpt(s: &str) -> String {
    let mut out: String = s.chars().take(EXCERPT_CHARS).collect();
    if s.chars().count() > EXCERPT_CHARS {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use parda_spec::protocol::driver::ObservedToolCall;
    use serde_json::json;

    use super::*;

    fn scenario(checks: &str, reply: &str) -> Scenario {
        toml::from_str(&format!(
            r#"
id = "s"
category = "basic"
description = "d"
api = "openai_chat"
stream = false
pii = ["2345 6789 0124"]
checks = [{checks}]
tools = [{{ name = "verify", description = "d", parameters = {{}} }}]
[[turns]]
input = {{ kind = "user", message = [{{ text = "Aadhaar " }}, {{ slot = 0 }}] }}
reply = {reply}
"#
        ))
        .unwrap()
    }

    fn text_reply() -> &'static str {
        r#"{ kind = "text", chunks = [[{ text = "Saved " }, { slot = 0 }]] }"#
    }

    fn log_with(payload: &str, placeholder: &str) -> UpstreamLog {
        UpstreamLog {
            payloads: vec![json!({"messages": [{"role": "user", "content": payload}]})],
            alignments: vec![(0, vec![(0, placeholder.to_owned())])],
            errors: vec![],
        }
    }

    fn completed(text: &str) -> Observation {
        Observation::Completed {
            text: text.to_owned(),
            tool_calls: vec![],
        }
    }

    fn failed_checks(outcome: &RestoreOutcome) -> Vec<CheckKind> {
        match outcome {
            RestoreOutcome::Fail { failures, .. } => failures.iter().map(|f| f.check).collect(),
            RestoreOutcome::Pass | RestoreOutcome::NotApplicable { .. } => vec![],
        }
    }

    const ALL: &str = r#""no_leak_upstream", "restored_exact", "no_placeholder_fragment""#;

    #[test]
    fn masked_and_restored_passes() {
        let s = scenario(ALL, text_reply());
        let outcome = evaluate(
            &s,
            &log_with("Aadhaar <AADHAAR_1>", "<AADHAAR_1>"),
            &[completed("Saved 2345 6789 0124")],
        );
        assert_eq!(outcome, RestoreOutcome::Pass);
    }

    #[test]
    fn reformatted_value_upstream_is_still_a_leak() {
        let s = scenario(ALL, text_reply());
        let outcome = evaluate(
            &s,
            &log_with("Aadhaar 234567890124", "234567890124"),
            &[completed("Saved 2345 6789 0124")],
        );
        assert_eq!(failed_checks(&outcome), vec![CheckKind::NoLeakUpstream]);
    }

    #[test]
    fn half_restored_placeholder_fails_restore_and_fragment_checks() {
        let s = scenario(ALL, text_reply());
        let outcome = evaluate(
            &s,
            &log_with("Aadhaar <AADHAAR_1>", "<AADHAAR_1>"),
            &[completed("Saved <AADHA")],
        );
        assert_eq!(
            failed_checks(&outcome),
            vec![CheckKind::RestoredExact, CheckKind::NoPlaceholderFragment]
        );
    }

    #[test]
    fn tool_call_arguments_compare_as_json() {
        let reply = r#"{ kind = "tool_call", call_id = "c1", name = "verify", argument_chunks = [[{ text = "{\"id\": \"" }, { slot = 0 }, { text = "\"}" }]] }"#;
        let s = scenario(r#""restored_exact", "json_valid""#, reply);
        let obs = Observation::Completed {
            text: String::new(),
            tool_calls: vec![ObservedToolCall {
                id: "c1".to_owned(),
                name: "verify".to_owned(),
                arguments: r#"{"id":"2345 6789 0124"}"#.to_owned(),
            }],
        };
        assert_eq!(
            evaluate(&s, &log_with("x", "<A>"), &[obs]),
            RestoreOutcome::Pass
        );
    }

    #[test]
    fn swallowed_upstream_error_fails_closed_check() {
        let s = scenario(
            r#""fails_closed""#,
            r#"{ kind = "http_error", status = 500, body = "{}" }"#,
        );
        let outcome = evaluate(&s, &log_with("x", "<A>"), &[completed("All good!")]);
        assert_eq!(failed_checks(&outcome), vec![CheckKind::FailsClosed]);
        let errored = Observation::Errored {
            message: "500".to_owned(),
            http_status: Some(500),
        };
        assert_eq!(
            evaluate(&s, &log_with("x", "<A>"), &[errored]),
            RestoreOutcome::Pass
        );
    }

    #[test]
    fn changing_placeholder_between_turns_is_unstable() {
        let s = scenario(r#""placeholder_stable_across_turns""#, text_reply());
        let mut log = log_with("x", "<A1>");
        log.alignments.push((1, vec![(0, "<A2>".to_owned())]));
        assert_eq!(
            failed_checks(&evaluate(&s, &log, &[completed("Saved 2345 6789 0124")])).len(),
            1
        );
    }

    #[test]
    fn mock_errors_and_missing_turns_always_fail() {
        let s = scenario(r#""no_leak_upstream""#, text_reply());
        let mut log = log_with("x", "<A>");
        log.errors.push("turn 0: literal not found".to_owned());
        assert_eq!(
            failed_checks(&evaluate(&s, &log, &[])),
            vec![CheckKind::RestoredExact, CheckKind::RestoredExact]
        );
    }
}
