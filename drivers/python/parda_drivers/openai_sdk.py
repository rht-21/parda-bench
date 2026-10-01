"""Official OpenAI Python SDK, chat completions."""

from __future__ import annotations

from typing import Any

from openai import OpenAI

from parda_drivers.protocol import Completed, ToolCall, openai_input, openai_tools, serve


def run_turn(request: dict[str, Any], history: list[Any], turn: dict[str, Any]) -> Completed:
    client = OpenAI(base_url=request["base_url"], api_key="parda-bench", max_retries=0, timeout=60)
    history.append(openai_input(turn))
    extra: dict[str, Any] = {"tools": openai_tools(request)} if request["tools"] else {}
    if request["stream"]:
        text = ""
        calls: dict[int, ToolCall] = {}
        for chunk in client.chat.completions.create(
            model=request["model"], messages=history, stream=True, **extra
        ):
            if not chunk.choices:
                continue
            delta = chunk.choices[0].delta
            text += delta.content or ""
            for tc in delta.tool_calls or []:
                entry = calls.setdefault(tc.index, ToolCall("", "", ""))
                entry.id = tc.id or entry.id
                if tc.function is not None:
                    entry.name = tc.function.name or entry.name
                    entry.arguments += tc.function.arguments or ""
        done = Completed(text, [calls[i] for i in sorted(calls)])
    else:
        message = client.chat.completions.create(model=request["model"], messages=history, **extra).choices[0].message
        done = Completed(
            message.content or "",
            [ToolCall(c.id, c.function.name, c.function.arguments) for c in message.tool_calls or []],
        )
    assistant: dict[str, Any] = {"role": "assistant", "content": done.text}
    if done.tool_calls:
        assistant["tool_calls"] = [
            {"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}
            for c in done.tool_calls
        ]
    history.append(assistant)
    return done


if __name__ == "__main__":
    serve(run_turn)
