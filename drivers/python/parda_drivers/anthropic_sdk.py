"""Official Anthropic Python SDK, messages API; streaming uses the SDK's `messages.stream` helper."""

from __future__ import annotations

import json
from typing import Any

from anthropic import Anthropic

from parda_drivers.protocol import Completed, ToolCall, serve

MAX_TOKENS = 1024


def run_turn(request: dict[str, Any], history: list[Any], turn: dict[str, Any]) -> Completed:
    client = Anthropic(base_url=request["base_url"], api_key="parda-bench", max_retries=0, timeout=60)
    if turn["kind"] == "user":
        history.append({"role": "user", "content": turn["text"]})
    else:
        history.append({
            "role": "user",
            "content": [{"type": "tool_result", "tool_use_id": turn["call_id"], "content": turn["content"]}],
        })
    extra: dict[str, Any] = {}
    if request["tools"]:
        extra["tools"] = [
            {"name": t["name"], "description": t["description"], "input_schema": t["parameters"]} for t in request["tools"]
        ]
    args = {"model": request["model"], "max_tokens": MAX_TOKENS, "messages": history, **extra}
    if request["stream"]:
        with client.messages.stream(**args) as stream:
            message = stream.get_final_message()
    else:
        message = client.messages.create(**args)
    text = "".join(b.text for b in message.content if b.type == "text")
    calls = [ToolCall(b.id, b.name, json.dumps(b.input, ensure_ascii=False)) for b in message.content if b.type == "tool_use"]
    history.append({"role": "assistant", "content": [b.model_dump(exclude_none=True) for b in message.content]})
    return Completed(text, calls)


if __name__ == "__main__":
    serve(run_turn)
