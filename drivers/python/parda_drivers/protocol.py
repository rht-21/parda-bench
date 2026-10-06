"""Driver loop: one `driver-request` per stdin line, one `driver-response` per stdout line.

Each framework module provides `run_turn(request, history, turn) -> Completed` and appends to `history` itself.
A turn that raises becomes an `errored` observation, and the conversation continues with the next turn.
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, field
from typing import Any, Callable


@dataclass
class ToolCall:
    id: str
    name: str
    arguments: str


@dataclass
class Completed:
    text: str
    tool_calls: list[ToolCall] = field(default_factory=list)


RunTurn = Callable[[dict[str, Any], list[Any], dict[str, Any]], Completed]


def status_of(error: BaseException) -> int | None:
    """HTTP status from any SDK's exception, when it carries one."""
    for attr in ("status_code", "status"):
        value = getattr(error, attr, None)
        if isinstance(value, int):
            return value
    response = getattr(error, "response", None)
    value = getattr(response, "status_code", None)
    return value if isinstance(value, int) else None


def observe(run_turn: RunTurn, request: dict[str, Any], history: list[Any], turn: dict[str, Any]) -> dict[str, Any]:
    try:
        done = run_turn(request, history, turn)
    except Exception as e:  # noqa: BLE001 - any client failure is what the app would see; report it
        return {"kind": "errored", "message": f"{type(e).__name__}: {e}", "http_status": status_of(e)}
    return {
        "kind": "completed",
        "text": done.text,
        "tool_calls": [{"id": c.id, "name": c.name, "arguments": c.arguments} for c in done.tool_calls],
    }


def serve(run_turn: RunTurn) -> None:
    proto_out = sys.stdout
    sys.stdout = sys.stderr
    for line in sys.stdin:
        if not line.strip():
            continue
        request = json.loads(line)
        history: list[Any] = []
        observations = [observe(run_turn, request, history, turn) for turn in request["turns"]]
        proto_out.write(json.dumps({"id": request["id"], "observations": observations}, ensure_ascii=False) + "\n")
        proto_out.flush()


def openai_tools(request: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {"type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["parameters"]}}
        for t in request["tools"]
    ]


def openai_input(turn: dict[str, Any]) -> dict[str, Any]:
    if turn["kind"] == "user":
        return {"role": "user", "content": turn["text"]}
    return {"role": "tool", "tool_call_id": turn["call_id"], "content": turn["content"]}
