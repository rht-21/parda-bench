"""LiteLLM `completion`, which speaks OpenAI format to the app for both upstream APIs."""

from __future__ import annotations

from typing import Any

import litellm

from parda_drivers.protocol import Completed, ToolCall, openai_input, openai_tools, serve

PROVIDER = {"openai_chat": "openai", "anthropic_messages": "anthropic"}


def run_turn(request: dict[str, Any], history: list[Any], turn: dict[str, Any]) -> Completed:
    history.append(openai_input(turn))
    args: dict[str, Any] = {
        "model": f"{PROVIDER[request['api']]}/{request['model']}",
        "api_base": request["base_url"],
        "api_key": "parda-bench",
        "messages": history,
        "num_retries": 0,
        "max_tokens": 1024,
    }
    if request["tools"]:
        args["tools"] = openai_tools(request)
    if request["stream"]:
        text, calls = "", {}
        for chunk in litellm.completion(stream=True, **args):
            delta = chunk.choices[0].delta
            text += delta.content or ""
            for tc in delta.tool_calls or []:
                entry = calls.setdefault(tc.index, ToolCall("", "", ""))
                entry.id = tc.id or entry.id
                entry.name = tc.function.name or entry.name
                entry.arguments += tc.function.arguments or ""
        done = Completed(text, [calls[i] for i in sorted(calls)])
    else:
        message = litellm.completion(**args).choices[0].message
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
