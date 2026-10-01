//! OpenAI chat-completions and Anthropic messages wire formats, as the mock upstream speaks them.

use parda_spec::scenario::Api;
use serde_json::{Value, json};

use crate::engine::ScriptedReply;

/// Which API a request path belongs to; matched by suffix so any base-URL prefix works.
#[must_use]
pub fn api_for_path(path: &str) -> Option<Api> {
    let path = path.trim_end_matches('/');
    if path.ends_with("/chat/completions") {
        Some(Api::OpenaiChat)
    } else if path.ends_with("/messages") {
        Some(Api::AnthropicMessages)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRequest {
    /// Zero-based: the number of inputs (user messages and tool results) minus one.
    pub turn: usize,
    /// Text of the newest input, which the scenario's turn input is aligned against.
    pub latest_input: String,
    pub stream: bool,
    pub model: String,
}

/// Extracts the turn number and newest input from a request body.
///
/// # Errors
/// Fails if `messages` is missing or holds no user input.
pub fn parse_request(api: Api, body: &Value) -> Result<ParsedRequest, String> {
    let messages = body["messages"]
        .as_array()
        .ok_or("request has no `messages` array")?;
    let inputs: Vec<&Value> = messages
        .iter()
        .filter(|m| match api {
            Api::OpenaiChat => matches!(m["role"].as_str(), Some("user" | "tool")),
            Api::AnthropicMessages => m["role"] == "user",
        })
        .collect();
    let latest = inputs.last().ok_or("request has no user or tool message")?;
    Ok(ParsedRequest {
        turn: inputs.len() - 1,
        latest_input: content_text(&latest["content"]),
        stream: body["stream"].as_bool().unwrap_or(false),
        model: body["model"].as_str().unwrap_or("parda-mock").to_owned(),
    })
}

/// Text of a message `content`: a string, or the concatenated text of its blocks (including tool results).
fn content_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .map(|b| match b["type"].as_str() {
                Some("tool_result") => content_text(&b["content"]),
                _ => b["text"].as_str().unwrap_or_default().to_owned(),
            })
            .collect(),
        _ => String::new(),
    }
}

/// A complete (non-streaming) response body for a text or tool-call reply.
///
/// # Errors
/// Fails for error replies, and for Anthropic tool calls whose rendered arguments are not a JSON object.
pub fn response_body(api: Api, reply: &ScriptedReply, model: &str) -> Result<Value, String> {
    match (api, reply) {
        (Api::OpenaiChat, ScriptedReply::Text { chunks }) => Ok(openai_completion(
            model,
            &json!({
                "role": "assistant", "content": chunks.concat()
            }),
            "stop",
        )),
        (
            Api::OpenaiChat,
            ScriptedReply::ToolCall {
                call_id,
                name,
                argument_chunks,
            },
        ) => Ok(openai_completion(
            model,
            &json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{"id": call_id, "type": "function", "function": {"name": name, "arguments": argument_chunks.concat()}}]
            }),
            "tool_calls",
        )),
        (Api::AnthropicMessages, ScriptedReply::Text { chunks }) => Ok(anthropic_message(
            model,
            &json!([{"type": "text", "text": chunks.concat()}]),
            "end_turn",
        )),
        (
            Api::AnthropicMessages,
            ScriptedReply::ToolCall {
                call_id,
                name,
                argument_chunks,
            },
        ) => {
            let input: Value = serde_json::from_str(&argument_chunks.concat())
                .map_err(|e| format!("tool-call arguments are not JSON: {e}"))?;
            Ok(anthropic_message(
                model,
                &json!([{"type": "tool_use", "id": call_id, "name": name, "input": input}]),
                "tool_use",
            ))
        }
        (_, ScriptedReply::HttpError { .. } | ScriptedReply::MockError { .. }) => {
            Err("error replies have no completion body".to_owned())
        }
    }
}

/// Server-sent events for a streamed reply, one string per event; each scripted chunk is exactly one delta.
///
/// # Errors
/// Fails for error replies.
pub fn sse_events(api: Api, reply: &ScriptedReply, model: &str) -> Result<Vec<String>, String> {
    match api {
        Api::OpenaiChat => openai_events(reply, model),
        Api::AnthropicMessages => anthropic_events(reply, model),
    }
}

/// JSON error body in the API's shape, used when the mock itself cannot follow the script.
#[must_use]
pub fn mock_error_body(api: Api, message: &str) -> Value {
    let message = format!("parda mock upstream: {message}");
    match api {
        Api::OpenaiChat => json!({"error": {"message": message, "type": "server_error"}}),
        Api::AnthropicMessages => {
            json!({"type": "error", "error": {"type": "api_error", "message": message}})
        }
    }
}

fn openai_completion(model: &str, message: &Value, finish_reason: &str) -> Value {
    json!({
        "id": "chatcmpl-parda",
        "object": "chat.completion",
        "created": 0,
        "model": model,
        "choices": [{"index": 0, "message": message, "finish_reason": finish_reason}],
        "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
    })
}

fn anthropic_message(model: &str, content: &Value, stop_reason: &str) -> Value {
    json!({
        "id": "msg_parda",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {"input_tokens": 0, "output_tokens": 0}
    })
}

fn openai_events(reply: &ScriptedReply, model: &str) -> Result<Vec<String>, String> {
    let chunk = |delta: Value, finish: Value| {
        let body = json!({
            "id": "chatcmpl-parda",
            "object": "chat.completion.chunk",
            "created": 0,
            "model": model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
        });
        format!("data: {body}\n\n")
    };
    let mut events = vec![chunk(
        json!({"role": "assistant", "content": ""}),
        Value::Null,
    )];
    let finish = match reply {
        ScriptedReply::Text { chunks } => {
            events.extend(
                chunks
                    .iter()
                    .map(|c| chunk(json!({"content": c}), Value::Null)),
            );
            "stop"
        }
        ScriptedReply::ToolCall {
            call_id,
            name,
            argument_chunks,
        } => {
            events.push(chunk(
                json!({"tool_calls": [{"index": 0, "id": call_id, "type": "function", "function": {"name": name, "arguments": ""}}]}),
                Value::Null,
            ));
            events.extend(argument_chunks.iter().map(|c| {
                chunk(
                    json!({"tool_calls": [{"index": 0, "function": {"arguments": c}}]}),
                    Value::Null,
                )
            }));
            "tool_calls"
        }
        ScriptedReply::HttpError { .. } | ScriptedReply::MockError { .. } => {
            return Err("error replies are not streamed".to_owned());
        }
    };
    events.push(chunk(json!({}), json!(finish)));
    events.push("data: [DONE]\n\n".to_owned());
    Ok(events)
}

fn anthropic_events(reply: &ScriptedReply, model: &str) -> Result<Vec<String>, String> {
    let event = |name: &str, data: Value| format!("event: {name}\ndata: {data}\n\n");
    let (block, deltas, stop_reason): (Value, Vec<Value>, &str) = match reply {
        ScriptedReply::Text { chunks } => (
            json!({"type": "text", "text": ""}),
            chunks
                .iter()
                .map(|c| json!({"type": "text_delta", "text": c}))
                .collect(),
            "end_turn",
        ),
        ScriptedReply::ToolCall {
            call_id,
            name,
            argument_chunks,
        } => (
            json!({"type": "tool_use", "id": call_id, "name": name, "input": {}}),
            argument_chunks
                .iter()
                .map(|c| json!({"type": "input_json_delta", "partial_json": c}))
                .collect(),
            "tool_use",
        ),
        ScriptedReply::HttpError { .. } | ScriptedReply::MockError { .. } => {
            return Err("error replies are not streamed".to_owned());
        }
    };
    let mut start = anthropic_message(model, &json!([]), "");
    start["stop_reason"] = Value::Null;
    let mut events = vec![
        event(
            "message_start",
            json!({"type": "message_start", "message": start}),
        ),
        event(
            "content_block_start",
            json!({"type": "content_block_start", "index": 0, "content_block": block}),
        ),
    ];
    events.extend(deltas.into_iter().map(|d| {
        event(
            "content_block_delta",
            json!({"type": "content_block_delta", "index": 0, "delta": d}),
        )
    }));
    events.push(event(
        "content_block_stop",
        json!({"type": "content_block_stop", "index": 0}),
    ));
    events.push(event(
        "message_delta",
        json!({"type": "message_delta", "delta": {"stop_reason": stop_reason, "stop_sequence": null}, "usage": {"output_tokens": 0}}),
    ));
    events.push(event("message_stop", json!({"type": "message_stop"})));
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_by_path_suffix() {
        assert_eq!(api_for_path("/v1/chat/completions"), Some(Api::OpenaiChat));
        assert_eq!(api_for_path("/chat/completions"), Some(Api::OpenaiChat));
        assert_eq!(
            api_for_path("/v1/v1/messages"),
            Some(Api::AnthropicMessages)
        );
        assert_eq!(api_for_path("/v1/embeddings"), None);
    }

    #[test]
    fn openai_turn_counts_user_and_tool_messages() {
        let body = json!({"model": "m", "stream": true, "messages": [
            {"role": "system", "content": "be nice"},
            {"role": "user", "content": "first"},
            {"role": "assistant", "content": null, "tool_calls": []},
            {"role": "tool", "tool_call_id": "c1", "content": [{"type": "text", "text": "tool "}, {"type": "text", "text": "out"}]}
        ]});
        let r = parse_request(Api::OpenaiChat, &body).unwrap();
        assert_eq!(
            (r.turn, r.latest_input.as_str(), r.stream),
            (1, "tool out", true)
        );
    }

    #[test]
    fn anthropic_tool_result_text_is_the_latest_input() {
        let body = json!({"model": "m", "messages": [
            {"role": "user", "content": "hi"},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "c1", "name": "f", "input": {}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "c1", "content": [{"type": "text", "text": "result"}]}]}
        ]});
        let r = parse_request(Api::AnthropicMessages, &body).unwrap();
        assert_eq!(
            (r.turn, r.latest_input.as_str(), r.stream),
            (1, "result", false)
        );
    }

    #[test]
    fn openai_stream_has_one_delta_per_chunk_then_done() {
        let reply = ScriptedReply::Text {
            chunks: vec!["a".to_owned(), "b".to_owned()],
        };
        let events = sse_events(Api::OpenaiChat, &reply, "m").unwrap();
        assert_eq!(events.len(), 5);
        assert!(events[1].contains(r#""content":"a""#));
        assert_eq!(events[4], "data: [DONE]\n\n");
    }

    #[test]
    fn anthropic_tool_call_body_parses_arguments() {
        let reply = ScriptedReply::ToolCall {
            call_id: "c1".to_owned(),
            name: "lookup".to_owned(),
            argument_chunks: vec![r#"{"pan": "#.to_owned(), r#""<PAN_1>"}"#.to_owned()],
        };
        let body = response_body(Api::AnthropicMessages, &reply, "m").unwrap();
        assert_eq!(body["content"][0]["input"]["pan"], "<PAN_1>");
        assert_eq!(body["stop_reason"], "tool_use");
    }
}
