"""The native driver: plain HTTP with no client library, so results isolate the tool from SDK behaviour."""

import json
from dataclasses import dataclass, field
from typing import Any

import aiohttp

from parda_bench.restore.driver.sse import SseParser
from parda_bench.spec.protocol import (
    Completed,
    DriverRequest,
    DriverResponse,
    DriverTurn,
    Errored,
    Observation,
    ObservedToolCall,
    UserTurn,
)
from parda_bench.spec.scenario import Api

TIMEOUT = aiohttp.ClientTimeout(total=60)
MAX_TOKENS = 1024
ANTHROPIC_VERSION = "2023-06-01"


class TurnFailed(Exception):
    def __init__(self, message: str, http_status: int | None = None) -> None:
        super().__init__(message)
        self.observation = Errored(message=message, http_status=http_status)


class RawHttpDriver:
    def __init__(self) -> None:
        self._session: aiohttp.ClientSession | None = None

    async def run(self, request: DriverRequest) -> DriverResponse:
        """Runs every turn; a failed turn is recorded and the conversation continues without an assistant message."""
        if self._session is None:
            self._session = aiohttp.ClientSession(timeout=TIMEOUT)
        messages: list[dict[str, Any]] = []
        observations: list[Observation] = []
        for turn in request.turns:
            messages.append(_input_message(request.api, turn))
            try:
                done = await self._send(self._session, request, messages)
            except TurnFailed as e:
                observations.append(e.observation)
                continue
            messages.append(_assistant_message(request.api, done))
            observations.append(done)
        return DriverResponse(id=request.id, observations=observations)

    async def close(self) -> None:
        if self._session is not None:
            await self._session.close()

    async def _send(
        self, session: aiohttp.ClientSession, request: DriverRequest, messages: list[dict[str, Any]]
    ) -> Completed:
        base = request.base_url.rstrip("/")
        if request.api is Api.OPENAI_CHAT:
            url, body = f"{base}/chat/completions", _openai_body(request, messages)
        else:
            url, body = f"{base}/v1/messages", _anthropic_body(request, messages)
        headers = {
            "authorization": "Bearer parda-bench",
            "x-api-key": "parda-bench",
            "anthropic-version": ANTHROPIC_VERSION,
        }
        try:
            async with session.post(url, json=body, headers=headers) as response:
                if response.status >= 400:
                    raise TurnFailed(await response.text(), response.status)
                if request.stream:
                    parser, acc = SseParser(), _StreamAccumulator()
                    async for chunk in response.content.iter_any():
                        for event in parser.push(chunk.decode("utf-8", errors="replace")):
                            acc.apply(request.api, event.data)
                    return acc.finish()
                return _parse_complete(request.api, await response.json(content_type=None))
        except (aiohttp.ClientError, TimeoutError) as e:
            raise TurnFailed(f"request to {url} failed: {e}") from e
        except (json.JSONDecodeError, KeyError, TypeError, IndexError) as e:
            raise TurnFailed(f"malformed response from {url}: {e}") from e


def _input_message(api: Api, turn: DriverTurn) -> dict[str, Any]:
    if isinstance(turn, UserTurn):
        return {"role": "user", "content": turn.text}
    if api is Api.OPENAI_CHAT:
        return {"role": "tool", "tool_call_id": turn.call_id, "content": turn.content}
    return {
        "role": "user",
        "content": [{"type": "tool_result", "tool_use_id": turn.call_id, "content": turn.content}],
    }


def _assistant_message(api: Api, done: Completed) -> dict[str, Any]:
    if api is Api.OPENAI_CHAT:
        message: dict[str, Any] = {"role": "assistant", "content": done.text}
        if done.tool_calls:
            message["tool_calls"] = [
                {"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}
                for c in done.tool_calls
            ]
        return message
    blocks: list[dict[str, Any]] = [{"type": "text", "text": done.text}] if done.text else []
    for c in done.tool_calls:
        try:
            tool_input = json.loads(c.arguments)
        except json.JSONDecodeError:
            # Unparseable arguments already fail the restore checks; `{}` keeps the history valid for later turns.
            tool_input = {}
        blocks.append({"type": "tool_use", "id": c.id, "name": c.name, "input": tool_input})
    return {"role": "assistant", "content": blocks}


def _openai_body(request: DriverRequest, messages: list[dict[str, Any]]) -> dict[str, Any]:
    body: dict[str, Any] = {"model": request.model, "messages": messages, "stream": request.stream}
    if request.tools:
        body["tools"] = [
            {
                "type": "function",
                "function": {"name": t.name, "description": t.description, "parameters": t.parameters},
            }
            for t in request.tools
        ]
    return body


def _anthropic_body(request: DriverRequest, messages: list[dict[str, Any]]) -> dict[str, Any]:
    body: dict[str, Any] = {
        "model": request.model,
        "max_tokens": MAX_TOKENS,
        "messages": messages,
        "stream": request.stream,
    }
    if request.tools:
        body["tools"] = [
            {"name": t.name, "description": t.description, "input_schema": t.parameters} for t in request.tools
        ]
    return body


def _parse_complete(api: Api, v: dict[str, Any]) -> Completed:
    if api is Api.OPENAI_CHAT:
        message = v["choices"][0]["message"]
        calls = [
            ObservedToolCall(id=c["id"], name=c["function"]["name"], arguments=c["function"]["arguments"])
            for c in message.get("tool_calls") or []
        ]
        return Completed(text=message.get("content") or "", tool_calls=calls)
    text = "".join(b.get("text", "") for b in v["content"] if b.get("type") == "text")
    calls = [
        ObservedToolCall(id=b["id"], name=b["name"], arguments=json.dumps(b["input"], ensure_ascii=False))
        for b in v["content"]
        if b.get("type") == "tool_use"
    ]
    return Completed(text=text, tool_calls=calls)


@dataclass
class _StreamAccumulator:
    """Rebuilds a turn from streamed deltas the way client libraries do."""

    text: str = ""
    calls: dict[int, list[str]] = field(default_factory=dict)
    """By index: [id, name, arguments]."""

    def apply(self, api: Api, data: str) -> None:
        if data == "[DONE]":
            return
        v = json.loads(data)
        if api is Api.OPENAI_CHAT:
            self._openai(v)
        else:
            self._anthropic(v)

    def _call(self, index: int) -> list[str]:
        return self.calls.setdefault(index, ["", "", ""])

    def _openai(self, v: dict[str, Any]) -> None:
        if "error" in v:
            raise TurnFailed(f"error event: {v['error']}")
        delta = v["choices"][0]["delta"]
        self.text += delta.get("content") or ""
        for c in delta.get("tool_calls") or []:
            entry = self._call(c.get("index", 0))
            entry[0] = c.get("id") or entry[0]
            function = c.get("function") or {}
            entry[1] = function.get("name") or entry[1]
            entry[2] += function.get("arguments") or ""

    def _anthropic(self, v: dict[str, Any]) -> None:
        kind = v.get("type")
        if kind == "error":
            raise TurnFailed(f"error event: {v.get('error')}")
        if kind == "content_block_start" and v["content_block"]["type"] == "tool_use":
            entry = self._call(v["index"])
            entry[0], entry[1] = v["content_block"]["id"], v["content_block"]["name"]
        elif kind == "content_block_delta":
            delta = v["delta"]
            if delta["type"] == "text_delta":
                self.text += delta["text"]
            elif delta["type"] == "input_json_delta":
                self._call(v["index"])[2] += delta["partial_json"]

    def finish(self) -> Completed:
        calls = [ObservedToolCall(id=i, name=n, arguments=a) for _, (i, n, a) in sorted(self.calls.items()) if i or n]
        return Completed(text=self.text, tool_calls=calls)
