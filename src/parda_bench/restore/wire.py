"""OpenAI chat-completions and Anthropic messages wire formats, as the mock upstream speaks them."""

import json
from dataclasses import dataclass
from typing import Any

from parda_bench.restore.engine import ScriptedReply, TextScript, ToolCallScript
from parda_bench.spec.scenario import Api


class WireError(ValueError):
    pass


def compact(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def api_for_path(path: str) -> Api | None:
    """Which API a request path belongs to; matched by suffix so any base-URL prefix works."""
    path = path.rstrip("/")
    if path.endswith("/chat/completions"):
        return Api.OPENAI_CHAT
    if path.endswith("/messages"):
        return Api.ANTHROPIC_MESSAGES
    return None


@dataclass(frozen=True)
class ParsedRequest:
    turn: int
    """Zero-based: the number of inputs (user messages and tool results) minus one."""
    latest_input: str
    """Text of the newest input, which the scenario's turn input is aligned against."""
    stream: bool
    model: str


def parse_request(api: Api, body: Any) -> ParsedRequest:
    messages = body.get("messages") if isinstance(body, dict) else None
    if not isinstance(messages, list):
        raise WireError("request has no `messages` array")
    roles = ("user", "tool") if api is Api.OPENAI_CHAT else ("user",)
    inputs = [m for m in messages if isinstance(m, dict) and m.get("role") in roles]
    if not inputs:
        raise WireError("request has no user or tool message")
    model = body.get("model")
    return ParsedRequest(
        turn=len(inputs) - 1,
        latest_input=_content_text(inputs[-1].get("content")),
        stream=body.get("stream") is True,
        model=model if isinstance(model, str) else "parda-mock",
    )


def _content_text(content: Any) -> str:
    """A string, or the concatenated text of a message's blocks (including tool results)."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "".join(
            _content_text(b.get("content")) if b.get("type") == "tool_result" else str(b.get("text", ""))
            for b in content
            if isinstance(b, dict)
        )
    return ""


def response_body(api: Api, reply: ScriptedReply, model: str) -> dict[str, Any]:
    """A complete (non-streaming) body; raises `WireError` for Anthropic tool calls with non-JSON arguments."""
    if isinstance(reply, TextScript):
        text = "".join(reply.chunks)
        if api is Api.OPENAI_CHAT:
            return _openai_completion(model, {"role": "assistant", "content": text}, "stop")
        return _anthropic_message(model, [{"type": "text", "text": text}], "end_turn")
    assert isinstance(reply, ToolCallScript), "error replies have no completion body"
    arguments = "".join(reply.argument_chunks)
    if api is Api.OPENAI_CHAT:
        call = {
            "id": reply.call_id,
            "type": "function",
            "function": {"name": reply.name, "arguments": arguments},
        }
        return _openai_completion(model, {"role": "assistant", "content": None, "tool_calls": [call]}, "tool_calls")
    try:
        tool_input = json.loads(arguments)
    except json.JSONDecodeError as e:
        raise WireError(f"tool-call arguments are not JSON: {e}") from e
    block = {"type": "tool_use", "id": reply.call_id, "name": reply.name, "input": tool_input}
    return _anthropic_message(model, [block], "tool_use")


def sse_events(api: Api, reply: ScriptedReply, model: str) -> list[str]:
    """One string per server-sent event; each scripted chunk is exactly one delta."""
    return _openai_events(reply, model) if api is Api.OPENAI_CHAT else _anthropic_events(reply, model)


def mock_error_body(api: Api, message: str) -> dict[str, Any]:
    """Error body in the API's shape, for when the mock itself cannot follow the script."""
    message = f"parda mock upstream: {message}"
    if api is Api.OPENAI_CHAT:
        return {"error": {"message": message, "type": "server_error"}}
    return {"type": "error", "error": {"type": "api_error", "message": message}}


def _openai_completion(model: str, message: dict[str, Any], finish_reason: str) -> dict[str, Any]:
    return {
        "id": "chatcmpl-parda",
        "object": "chat.completion",
        "created": 0,
        "model": model,
        "choices": [{"index": 0, "message": message, "finish_reason": finish_reason}],
        "usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0},
    }


def _anthropic_message(model: str, content: list[Any], stop_reason: str | None) -> dict[str, Any]:
    return {
        "id": "msg_parda",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": None,
        "usage": {"input_tokens": 0, "output_tokens": 0},
    }


def _openai_events(reply: ScriptedReply, model: str) -> list[str]:
    def chunk(delta: dict[str, Any], finish: str | None = None) -> str:
        body = {
            "id": "chatcmpl-parda",
            "object": "chat.completion.chunk",
            "created": 0,
            "model": model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
        }
        return f"data: {compact(body)}\n\n"

    events = [chunk({"role": "assistant", "content": ""})]
    if isinstance(reply, TextScript):
        events += [chunk({"content": c}) for c in reply.chunks]
        finish = "stop"
    else:
        assert isinstance(reply, ToolCallScript), "error replies are not streamed"
        start = {
            "index": 0,
            "id": reply.call_id,
            "type": "function",
            "function": {"name": reply.name, "arguments": ""},
        }
        events.append(chunk({"tool_calls": [start]}))
        events += [chunk({"tool_calls": [{"index": 0, "function": {"arguments": c}}]}) for c in reply.argument_chunks]
        finish = "tool_calls"
    return [*events, chunk({}, finish), "data: [DONE]\n\n"]


def _anthropic_events(reply: ScriptedReply, model: str) -> list[str]:
    def event(name: str, data: dict[str, Any]) -> str:
        return f"event: {name}\ndata: {compact(data)}\n\n"

    if isinstance(reply, TextScript):
        block: dict[str, Any] = {"type": "text", "text": ""}
        deltas = [{"type": "text_delta", "text": c} for c in reply.chunks]
        stop_reason = "end_turn"
    else:
        assert isinstance(reply, ToolCallScript), "error replies are not streamed"
        block = {"type": "tool_use", "id": reply.call_id, "name": reply.name, "input": {}}
        deltas = [{"type": "input_json_delta", "partial_json": c} for c in reply.argument_chunks]
        stop_reason = "tool_use"
    return [
        event("message_start", {"type": "message_start", "message": _anthropic_message(model, [], None)}),
        event("content_block_start", {"type": "content_block_start", "index": 0, "content_block": block}),
        *(event("content_block_delta", {"type": "content_block_delta", "index": 0, "delta": d}) for d in deltas),
        event("content_block_stop", {"type": "content_block_stop", "index": 0}),
        event(
            "message_delta",
            {
                "type": "message_delta",
                "delta": {"stop_reason": stop_reason, "stop_sequence": None},
                "usage": {"output_tokens": 0},
            },
        ),
        event("message_stop", {"type": "message_stop"}),
    ]
