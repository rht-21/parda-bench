"""NDJSON adapter loop (schemas `spec/adapter-request.schema.json` and `spec/adapter-response.schema.json`).

Offsets are Python `str` indices, which is what the protocol expects (Unicode scalar values).
"""

from __future__ import annotations

import json
import sys
import time
from dataclasses import dataclass
from typing import IO, Any, Callable


@dataclass(frozen=True)
class Entity:
    start: int
    end: int
    label: str
    score: float | None = None


class Adapter:
    """Base class. Override the operations the tool supports; capabilities are derived from the overrides."""

    tool_version: str = "unknown"

    def detect(self, text: str) -> list[Entity]:
        raise NotImplementedError

    def mask(self, session: str, text: str) -> str:
        raise NotImplementedError

    def unmask(self, session: str, text: str) -> str:
        raise NotImplementedError

    def unmask_stream(self, session: str, chunk: str, is_final: bool) -> str:
        raise NotImplementedError

    def capabilities(self) -> list[str]:
        ops = ["detect", "mask", "unmask", "unmask_stream"]
        return [op for op in ops if getattr(type(self), op) is not getattr(Adapter, op)]


def run(adapter: Adapter, stdin: IO[str] | None = None, stdout: IO[str] | None = None) -> None:
    """Serve requests until `shutdown` or end of input.

    While running, `sys.stdout` points at stderr so stray prints from libraries cannot corrupt the protocol.
    """
    proto_in = stdin if stdin is not None else sys.stdin
    proto_out = stdout if stdout is not None else sys.stdout
    sys.stdout = sys.stderr
    try:
        for line in proto_in:
            if not line.strip():
                continue
            response, done = _handle(adapter, line)
            proto_out.write(json.dumps(response, ensure_ascii=False) + "\n")
            proto_out.flush()
            if done:
                return
    finally:
        sys.stdout = sys.__stdout__


def _handle(adapter: Adapter, line: str) -> tuple[dict[str, Any], bool]:
    try:
        request = json.loads(line)
        request_id = request["id"]
        op = request["op"]
    except (ValueError, KeyError, TypeError) as e:
        return {"id": 0, "status": "error", "message": f"malformed request: {e}"}, False
    handler = _HANDLERS.get(op)
    if handler is None:
        return {"id": request_id, "status": "error", "message": f"unknown op {op!r}"}, False
    started = time.perf_counter_ns()
    try:
        result = handler(adapter, request)
    except Exception as e:  # noqa: BLE001 - any tool failure is reported to the harness, which records it
        return {"id": request_id, "status": "error", "message": f"{type(e).__name__}: {e}"}, False
    elapsed_ns = time.perf_counter_ns() - started
    return {"id": request_id, "status": "ok", "elapsed_ns": elapsed_ns, "result": {"op": op, **result}}, op == "shutdown"


def _hello(adapter: Adapter, _request: dict[str, Any]) -> dict[str, Any]:
    return {"tool_version": adapter.tool_version, "capabilities": adapter.capabilities()}


def _detect(adapter: Adapter, request: dict[str, Any]) -> dict[str, Any]:
    entities = adapter.detect(request["text"])
    return {"entities": [{"start": e.start, "end": e.end, "label": e.label, "score": e.score} for e in entities]}


def _mask(adapter: Adapter, request: dict[str, Any]) -> dict[str, Any]:
    return {"text": adapter.mask(request["session"], request["text"])}


def _unmask(adapter: Adapter, request: dict[str, Any]) -> dict[str, Any]:
    return {"text": adapter.unmask(request["session"], request["text"])}


def _unmask_stream(adapter: Adapter, request: dict[str, Any]) -> dict[str, Any]:
    return {"text": adapter.unmask_stream(request["session"], request["chunk"], request["is_final"])}


def _shutdown(_adapter: Adapter, _request: dict[str, Any]) -> dict[str, Any]:
    return {}


_HANDLERS: dict[str, Callable[[Adapter, dict[str, Any]], dict[str, Any]]] = {
    "hello": _hello,
    "detect": _detect,
    "mask": _mask,
    "unmask": _unmask,
    "unmask_stream": _unmask_stream,
    "shutdown": _shutdown,
}
