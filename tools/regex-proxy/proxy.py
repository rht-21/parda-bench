"""OpenAI chat-completions and Anthropic messages proxy that masks requests and restores replies.

One process-wide session: a value gets the same placeholder in every conversation, which keeps history stable.
Upstream errors are passed through unchanged, so the client sees the failure (fails closed).
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Iterator

from regex_baseline.vault import Vault

SESSION = "proxy"
# Keys whose values are protocol fields, never user text.
_STRUCTURAL_KEYS = frozenset({"role", "type", "id", "tool_call_id", "tool_use_id", "name", "model"})

vault = Vault()


def transform_strings(value: Any, fn: Any) -> Any:
    if isinstance(value, str):
        return fn(value)
    if isinstance(value, list):
        return [transform_strings(v, fn) for v in value]
    if isinstance(value, dict):
        return {k: v if k in _STRUCTURAL_KEYS else transform_strings(v, fn) for k, v in value.items()}
    return value


def mask_request(body: dict[str, Any]) -> dict[str, Any]:
    masked = dict(body)
    for key in ("messages", "system"):
        if key in masked:
            masked[key] = transform_strings(masked[key], lambda s: vault.mask(SESSION, s))
    return masked


def unmask_response(body: Any) -> Any:
    return transform_strings(body, lambda s: vault.unmask(SESSION, s))


class StreamRestorer:
    """Restores placeholders in streamed deltas. Each text stream (content, or a tool call's arguments) has its own
    buffer; buffers are flushed as an extra delta just before the event that ends them."""

    def __init__(self, anthropic: bool) -> None:
        self.anthropic = anthropic
        self.open_streams: set[str] = set()

    def _restore(self, stream: str, text: str) -> str:
        self.open_streams.add(stream)
        return vault.unmask_stream(SESSION, text, False, stream)

    def _flush(self, stream: str) -> str:
        self.open_streams.discard(stream)
        return vault.unmask_stream(SESSION, "", True, stream)

    def events(self, event: dict[str, Any]) -> list[dict[str, Any]]:
        return self._anthropic(event) if self.anthropic else self._openai(event)

    def _openai(self, event: dict[str, Any]) -> list[dict[str, Any]]:
        out: list[dict[str, Any]] = []
        for choice in event.get("choices", []):
            delta = choice.get("delta", {})
            if isinstance(delta.get("content"), str):
                delta["content"] = self._restore("content", delta["content"])
            for call in delta.get("tool_calls") or []:
                fn = call.get("function", {})
                if isinstance(fn.get("arguments"), str):
                    fn["arguments"] = self._restore(f"args:{call.get('index', 0)}", fn["arguments"])
            if choice.get("finish_reason") is not None:
                out.extend(self._openai_flush(event))
        out.append(event)
        return out

    def _openai_flush(self, template: dict[str, Any]) -> list[dict[str, Any]]:
        flushed = []
        for stream in sorted(self.open_streams):
            text = self._flush(stream)
            if not text:
                continue
            if stream == "content":
                delta: dict[str, Any] = {"content": text}
            else:
                delta = {"tool_calls": [{"index": int(stream.split(":")[1]), "function": {"arguments": text}}]}
            flushed.append({**template, "choices": [{"index": 0, "delta": delta, "finish_reason": None}]})
        return flushed

    def _anthropic(self, event: dict[str, Any]) -> list[dict[str, Any]]:
        index = event.get("index", 0)
        if event.get("type") == "content_block_delta":
            delta = event["delta"]
            if delta.get("type") == "text_delta":
                delta["text"] = self._restore(f"block:{index}:text", delta["text"])
            elif delta.get("type") == "input_json_delta":
                delta["partial_json"] = self._restore(f"block:{index}:json", delta["partial_json"])
            return [event]
        if event.get("type") == "content_block_stop":
            out = []
            for stream in [s for s in sorted(self.open_streams) if s.startswith(f"block:{index}:")]:
                text = self._flush(stream)
                if text:
                    kind = "text_delta" if stream.endswith(":text") else "input_json_delta"
                    field = "text" if kind == "text_delta" else "partial_json"
                    out.append({"type": "content_block_delta", "index": index, "delta": {"type": kind, field: text}})
            return out + [event]
        return [event]


def sse_events(lines: Iterator[bytes]) -> Iterator[tuple[str | None, str]]:
    name: str | None = None
    data: list[str] = []
    for raw in lines:
        line = raw.decode("utf-8").rstrip("\r\n")
        if not line:
            if data:
                yield name, "\n".join(data)
            name, data = None, []
        elif line.startswith("event:"):
            name = line[6:].strip()
        elif line.startswith("data:"):
            data.append(line[5:].lstrip(" "))
    if data:
        yield name, "\n".join(data)


class Handler(BaseHTTPRequestHandler):
    upstream = ""

    def do_POST(self) -> None:  # noqa: N802 - http.server naming
        length = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(length) or b"{}")
        anthropic = self.path.rstrip("/").endswith("/messages")
        request = urllib.request.Request(
            self.upstream.rstrip("/") + self.path,
            data=json.dumps(mask_request(body)).encode(),
            headers={k: v for k, v in self.headers.items() if k.lower() not in ("host", "content-length")},
            method="POST",
        )
        try:
            upstream = urllib.request.urlopen(request, timeout=120)
        except urllib.error.HTTPError as e:
            self._send(e.code, e.headers.get("content-type", "application/json"), e.read())
            return
        with upstream:
            if body.get("stream"):
                self._stream(upstream, anthropic)
            else:
                restored = unmask_response(json.loads(upstream.read()))
                self._send(200, "application/json", json.dumps(restored).encode())

    def _send(self, status: int, content_type: str, payload: bytes) -> None:
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def _stream(self, upstream: Any, anthropic: bool) -> None:
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.end_headers()
        restorer = StreamRestorer(anthropic)
        for name, data in sse_events(iter(upstream.readline, b"")):
            if data == "[DONE]":
                self._write(None, data)
                continue
            for event in restorer.events(json.loads(data)):
                self._write(name if anthropic else None, json.dumps(event))

    def _write(self, name: str | None, data: str) -> None:
        prefix = f"event: {event_name(name, data)}\n" if name else ""
        self.wfile.write(f"{prefix}data: {data}\n\n".encode())
        self.wfile.flush()

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002 - http.server signature
        sys.stderr.write(format % args + "\n")


def event_name(original: str, data: str) -> str:
    """Flushed deltas are emitted under their own event type, not the event they precede."""
    kind = json.loads(data).get("type")
    return kind if isinstance(kind, str) else original


def main() -> None:
    upstream = os.environ.get("PARDA_UPSTREAM_URL")
    if not upstream:
        sys.exit("PARDA_UPSTREAM_URL is required")
    Handler.upstream = upstream
    port = int(os.environ.get("PARDA_PROXY_PORT", "18431"))
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()


if __name__ == "__main__":
    main()
