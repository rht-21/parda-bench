//! The native driver: plain HTTP with no client library, so results isolate the tool from SDK behaviour.

use std::time::Duration;

use futures_util::StreamExt;
use parda_spec::protocol::driver::{
    DriverRequest, DriverResponse, DriverTurn, Observation, ObservedToolCall,
};
use parda_spec::scenario::{Api, ToolDefinition};
use serde_json::{Value, json};

use crate::driver::sse::SseParser;

const TIMEOUT: Duration = Duration::from_secs(60);
const MAX_TOKENS: u32 = 1024;
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct RawHttpDriver {
    client: reqwest::Client,
}

impl Default for RawHttpDriver {
    fn default() -> Self {
        Self::new()
    }
}

/// What came back for one request, before it is appended to the history.
struct Turn {
    text: String,
    calls: Vec<ObservedToolCall>,
}

impl RawHttpDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Runs every turn; a failed turn is recorded and the conversation continues without an assistant message.
    pub async fn run(&self, request: &DriverRequest) -> DriverResponse {
        let mut messages: Vec<Value> = Vec::new();
        let mut observations = Vec::new();
        for turn in &request.turns {
            messages.push(input_message(request.api, turn));
            let observation = match self.send(request, &messages).await {
                Ok(t) => {
                    messages.push(assistant_message(request.api, &t));
                    Observation::Completed {
                        text: t.text,
                        tool_calls: t.calls,
                    }
                }
                Err(observation) => observation,
            };
            observations.push(observation);
        }
        DriverResponse {
            id: request.id,
            observations,
        }
    }

    async fn send(&self, request: &DriverRequest, messages: &[Value]) -> Result<Turn, Observation> {
        let base = request.base_url.trim_end_matches('/');
        let (url, body) = match request.api {
            Api::OpenaiChat => (
                format!("{base}/chat/completions"),
                openai_body(request, messages),
            ),
            Api::AnthropicMessages => (
                format!("{base}/v1/messages"),
                anthropic_body(request, messages),
            ),
        };
        let response = self
            .client
            .post(&url)
            .json(&body)
            .bearer_auth("parda-bench")
            .header("x-api-key", "parda-bench")
            .header("anthropic-version", ANTHROPIC_VERSION)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|e| errored(format!("request to {url} failed: {e}"), None))?;
        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|e| format!("<unreadable body: {e}>"));
            return Err(errored(text, Some(status.as_u16())));
        }
        if request.stream {
            let mut parser = SseParser::default();
            let mut acc = StreamAccumulator::default();
            let mut bytes = response.bytes_stream();
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|e| errored(format!("stream interrupted: {e}"), None))?;
                for event in parser.push(&String::from_utf8_lossy(&chunk)) {
                    acc.apply(request.api, &event.data)
                        .map_err(|m| errored(m, None))?;
                }
            }
            Ok(acc.finish())
        } else {
            let value: Value = response
                .json()
                .await
                .map_err(|e| errored(format!("response is not JSON: {e}"), None))?;
            parse_complete(request.api, &value).map_err(|m| errored(m, None))
        }
    }
}

fn errored(message: String, http_status: Option<u16>) -> Observation {
    Observation::Errored {
        message,
        http_status,
    }
}

fn input_message(api: Api, turn: &DriverTurn) -> Value {
    match (api, turn) {
        (_, DriverTurn::User { text }) => json!({"role": "user", "content": text}),
        (Api::OpenaiChat, DriverTurn::ToolResult { call_id, content }) => {
            json!({"role": "tool", "tool_call_id": call_id, "content": content})
        }
        (Api::AnthropicMessages, DriverTurn::ToolResult { call_id, content }) => json!({
            "role": "user",
            "content": [{"type": "tool_result", "tool_use_id": call_id, "content": content}]
        }),
    }
}

fn assistant_message(api: Api, turn: &Turn) -> Value {
    match api {
        Api::OpenaiChat => {
            let mut m = json!({"role": "assistant", "content": turn.text});
            if !turn.calls.is_empty() {
                m["tool_calls"] = turn
                    .calls
                    .iter()
                    .map(|c| json!({"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
                    .collect();
            }
            m
        }
        Api::AnthropicMessages => {
            let mut blocks: Vec<Value> = Vec::new();
            if !turn.text.is_empty() {
                blocks.push(json!({"type": "text", "text": turn.text}));
            }
            blocks.extend(turn.calls.iter().map(|c| {
                // Unparseable arguments already fail the restore checks; `{}` keeps the history valid for later turns.
                let input =
                    serde_json::from_str::<Value>(&c.arguments).unwrap_or_else(|_| json!({}));
                json!({"type": "tool_use", "id": c.id, "name": c.name, "input": input})
            }));
            json!({"role": "assistant", "content": blocks})
        }
    }
}

fn openai_body(request: &DriverRequest, messages: &[Value]) -> Value {
    let mut body = json!({"model": request.model, "messages": messages, "stream": request.stream});
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|t: &ToolDefinition| {
                json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}})
            })
            .collect();
    }
    body
}

fn anthropic_body(request: &DriverRequest, messages: &[Value]) -> Value {
    let mut body = json!({"model": request.model, "max_tokens": MAX_TOKENS, "messages": messages, "stream": request.stream});
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.parameters}))
            .collect();
    }
    body
}

fn parse_complete(api: Api, v: &Value) -> Result<Turn, String> {
    match api {
        Api::OpenaiChat => {
            let message = &v["choices"][0]["message"];
            if message.is_null() {
                return Err(format!("no choices[0].message in {v}"));
            }
            let calls = message["tool_calls"]
                .as_array()
                .map(|calls| {
                    calls
                        .iter()
                        .map(|c| ObservedToolCall {
                            id: str_of(&c["id"]),
                            name: str_of(&c["function"]["name"]),
                            arguments: str_of(&c["function"]["arguments"]),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(Turn {
                text: str_of(&message["content"]),
                calls,
            })
        }
        Api::AnthropicMessages => {
            let blocks = v["content"]
                .as_array()
                .ok_or_else(|| format!("no content array in {v}"))?;
            let mut turn = Turn {
                text: String::new(),
                calls: Vec::new(),
            };
            for b in blocks {
                match b["type"].as_str() {
                    Some("text") => turn.text.push_str(b["text"].as_str().unwrap_or_default()),
                    Some("tool_use") => turn.calls.push(ObservedToolCall {
                        id: str_of(&b["id"]),
                        name: str_of(&b["name"]),
                        arguments: b["input"].to_string(),
                    }),
                    _ => {}
                }
            }
            Ok(turn)
        }
    }
}

/// A JSON string's value; empty for null or a missing field, as clients surface them.
fn str_of(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// Rebuilds a turn from streamed deltas the way client libraries do.
#[derive(Default)]
struct StreamAccumulator {
    text: String,
    /// By index: (id, name, arguments).
    calls: Vec<(String, String, String)>,
}

impl StreamAccumulator {
    fn apply(&mut self, api: Api, data: &str) -> Result<(), String> {
        if data == "[DONE]" {
            return Ok(());
        }
        let v: Value =
            serde_json::from_str(data).map_err(|e| format!("bad stream event ({e}): {data}"))?;
        match api {
            Api::OpenaiChat => self.apply_openai(&v),
            Api::AnthropicMessages => self.apply_anthropic(&v),
        }
    }

    fn apply_openai(&mut self, v: &Value) -> Result<(), String> {
        if let Some(err) = v.get("error") {
            return Err(format!("error event: {err}"));
        }
        let delta = &v["choices"][0]["delta"];
        if let Some(t) = delta["content"].as_str() {
            self.text.push_str(t);
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            let index =
                usize::try_from(call["index"].as_u64().unwrap_or(0)).map_err(|e| e.to_string())?;
            let entry = self.call_at(index);
            if let Some(id) = call["id"].as_str() {
                id.clone_into(&mut entry.0);
            }
            if let Some(name) = call["function"]["name"].as_str() {
                name.clone_into(&mut entry.1);
            }
            entry
                .2
                .push_str(call["function"]["arguments"].as_str().unwrap_or_default());
        }
        Ok(())
    }

    fn apply_anthropic(&mut self, v: &Value) -> Result<(), String> {
        match v["type"].as_str() {
            Some("error") => return Err(format!("error event: {}", v["error"])),
            Some("content_block_start") if v["content_block"]["type"] == "tool_use" => {
                let index = block_index(v)?;
                let entry = self.call_at(index);
                entry.0 = str_of(&v["content_block"]["id"]);
                entry.1 = str_of(&v["content_block"]["name"]);
            }
            Some("content_block_delta") => {
                let delta = &v["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => self
                        .text
                        .push_str(delta["text"].as_str().unwrap_or_default()),
                    Some("input_json_delta") => {
                        let index = block_index(v)?;
                        self.call_at(index)
                            .2
                            .push_str(delta["partial_json"].as_str().unwrap_or_default());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn call_at(&mut self, index: usize) -> &mut (String, String, String) {
        if self.calls.len() <= index {
            self.calls.resize_with(index + 1, Default::default);
        }
        &mut self.calls[index]
    }

    fn finish(self) -> Turn {
        let calls = self
            .calls
            .into_iter()
            .filter(|(id, name, _)| !id.is_empty() || !name.is_empty())
            .map(|(id, name, arguments)| ObservedToolCall {
                id,
                name,
                arguments,
            })
            .collect();
        Turn {
            text: self.text,
            calls,
        }
    }
}

fn block_index(v: &Value) -> Result<usize, String> {
    v["index"]
        .as_u64()
        .and_then(|i| usize::try_from(i).ok())
        .ok_or_else(|| format!("stream event without a block index: {v}"))
}
