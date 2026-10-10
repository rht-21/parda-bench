"""The mock LLM upstream: an HTTP server on localhost that replays the loaded scenario."""

import json
import socket
from typing import Any

from aiohttp import web

from parda_bench.restore.engine import Engine, HttpErrorScript, MockError, UpstreamLog
from parda_bench.restore.wire import (
    WireError,
    api_for_path,
    mock_error_body,
    parse_request,
    response_body,
    sse_events,
)
from parda_bench.spec.scenario import Api, Scenario


class MockUpstream:
    """Start with `start()`, load one scenario at a time, `finish()` to collect what it saw, then `stop()`."""

    def __init__(self) -> None:
        self._engine: Engine | None = None
        self._runner: web.AppRunner | None = None
        self.base_url = ""

    async def start(self) -> None:
        app = web.Application()
        app.router.add_route("*", "/{tail:.*}", self._handle)
        self._runner = web.AppRunner(app, access_log=None)
        await self._runner.setup()
        sock = socket.socket()
        sock.bind(("127.0.0.1", 0))
        await web.SockSite(self._runner, sock).start()
        self.base_url = f"http://127.0.0.1:{sock.getsockname()[1]}"

    async def stop(self) -> None:
        if self._runner is not None:
            await self._runner.cleanup()

    def load(self, scenario: Scenario) -> None:
        """Replaces the active scenario, discarding anything recorded for the previous one."""
        self._engine = Engine(scenario)

    def finish(self) -> UpstreamLog:
        """Ends the active scenario and returns what the upstream saw."""
        if self._engine is None:
            raise RuntimeError("no scenario is loaded")
        engine, self._engine = self._engine, None
        return engine.log

    async def _handle(self, request: web.Request) -> web.StreamResponse:
        api = api_for_path(request.path)
        if api is None:
            return web.Response(status=404, text=f"parda mock upstream: no API at {request.path}")
        try:
            payload: Any = json.loads(await request.read())
        except json.JSONDecodeError as e:
            return _error(api, 400, f"body is not JSON: {e}")
        if self._engine is None:
            return _error(api, 503, "no scenario loaded")
        self._engine.record_payload(payload)
        try:
            parsed = parse_request(api, payload)
        except WireError as e:
            return _error(api, 400, str(e))
        reply = self._engine.reply(parsed.turn, parsed.latest_input)
        if isinstance(reply, HttpErrorScript):
            return web.Response(status=reply.status, text=reply.body, content_type="application/json")
        if isinstance(reply, MockError):
            return _error(api, 500, reply.message)
        try:
            if not parsed.stream:
                return web.json_response(response_body(api, reply, parsed.model))
            events = sse_events(api, reply, parsed.model)
        except WireError as e:
            return _error(api, 500, str(e))
        response = web.StreamResponse(headers={"content-type": "text/event-stream", "cache-control": "no-cache"})
        await response.prepare(request)
        for event in events:
            await response.write(event.encode())
        await response.write_eof()
        return response


def _error(api: Api, status: int, message: str) -> web.Response:
    return web.json_response(mock_error_body(api, message), status=status)
