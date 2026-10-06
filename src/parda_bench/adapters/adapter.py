"""One running tool, whatever its transport, with typed helpers for each adapter operation."""

import asyncio
import json
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import aiohttp
from pydantic import TypeAdapter, ValidationError

from parda_bench.adapters.errors import AdapterError, CapabilityMismatch, ToolError, TransportError
from parda_bench.adapters.process import GroupProcess, spawn_service, spawn_worker, wait_until_listening
from parda_bench.adapters.tool import Tool
from parda_bench.spec.manifest import HttpApi, HttpProxy, Stdio
from parda_bench.spec.protocol import (
    AdapterResponse,
    AdapterResult,
    DetectedEntity,
    DetectResult,
    ErrorResponse,
    HelloResult,
    MaskResult,
    UnmaskResult,
    UnmaskStreamResult,
)

STARTUP_TIMEOUT = 600.0
"""Tools may load large models at startup."""
REQUEST_TIMEOUT = 60.0
SHUTDOWN_GRACE = 5.0
_RESPONSE: TypeAdapter[Any] = TypeAdapter(AdapterResponse)


@dataclass(frozen=True)
class Reply:
    result: AdapterResult
    elapsed_ns: int
    """Measured inside the tool."""
    wall_ns: int
    """Measured by the harness, including IPC."""


class Adapter:
    """A started tool. Create with `start`; always `shutdown` it."""

    def __init__(self, process: GroupProcess, base_url: str | None) -> None:
        self._process = process
        self._base_url = base_url
        self._session = aiohttp.ClientSession() if base_url else None
        self._next_id = 1

    @staticmethod
    async def start(tool: Tool, log: Path | None) -> "Adapter":
        """Starts the tool, performs the `hello` handshake and checks its capabilities against the manifest."""
        transport = tool.manifest.transport
        if isinstance(transport, HttpProxy):
            raise AdapterError("http-proxy tools are driven through the restore suite, not as adapters")
        if isinstance(transport, Stdio):
            adapter = Adapter(await spawn_worker(tool.dir, transport.command, tool.env(), log), None)
        else:
            assert isinstance(transport, HttpApi)
            service = await spawn_service(tool.dir, transport.start, tool.env(), log)
            try:
                await wait_until_listening(transport.base_url, service, STARTUP_TIMEOUT)
            except AdapterError:
                await service.terminate()
                raise
            adapter = Adapter(service, transport.base_url)
        try:
            hello = await adapter.call({"op": "hello"}, STARTUP_TIMEOUT)
            assert isinstance(hello.result, HelloResult)
            declared, reported = set(tool.manifest.capabilities), set(hello.result.capabilities)
            if declared != reported:
                raise CapabilityMismatch(
                    f"worker capabilities {sorted(reported)} differ from manifest {sorted(declared)}"
                )
        except BaseException:
            await adapter.shutdown()
            raise
        return adapter

    async def call(self, request: dict[str, Any], timeout: float) -> Reply:
        """Sends one operation; raises `ToolError` for a tool-reported error, `TransportError` otherwise."""
        request_id = self._next_id
        self._next_id += 1
        payload = {"id": request_id, **request}
        started = time.perf_counter_ns()
        line = await (self._post(payload, timeout) if self._session else self._exchange(payload, timeout))
        wall_ns = time.perf_counter_ns() - started
        try:
            response = _RESPONSE.validate_json(line)
        except ValidationError as e:
            raise TransportError(f"worker sent a line that is not a valid adapter response ({e}): {line!r}") from e
        if response.id != request_id:
            raise TransportError(f"worker replied to request {response.id}, expected {request_id}")
        if isinstance(response, ErrorResponse):
            raise ToolError(response.message)
        if response.result.op != request["op"]:
            raise TransportError(f"worker replied with `{response.result.op}` to a `{request['op']}` request")
        return Reply(response.result, response.elapsed_ns, wall_ns)

    async def _exchange(self, payload: dict[str, Any], timeout: float) -> bytes:
        stdin, stdout = self._process.process.stdin, self._process.process.stdout
        assert stdin is not None and stdout is not None
        try:
            stdin.write(json.dumps(payload, ensure_ascii=False).encode() + b"\n")
            await stdin.drain()
            line = await asyncio.wait_for(stdout.readline(), timeout)
        except TimeoutError as e:
            raise TransportError(f"no reply to request {payload['id']} within {timeout}s") from e
        except (ConnectionError, BrokenPipeError) as e:
            raise TransportError(f"worker I/O: {e}") from e
        if not line:
            raise TransportError(f"worker exited before replying to request {payload['id']}")
        return line

    async def _post(self, payload: dict[str, Any], timeout: float) -> bytes:
        assert self._session is not None and self._base_url is not None
        try:
            async with self._session.post(
                self._base_url, json=payload, timeout=aiohttp.ClientTimeout(total=timeout)
            ) as response:
                return await response.read()
        except TimeoutError as e:
            raise TransportError(f"no reply to request {payload['id']} within {timeout}s") from e
        except aiohttp.ClientError as e:
            raise TransportError(f"HTTP request to {self._base_url}: {e}") from e

    async def detect(self, text: str) -> tuple[list[DetectedEntity], Reply]:
        reply = await self.call({"op": "detect", "text": text}, REQUEST_TIMEOUT)
        assert isinstance(reply.result, DetectResult)
        return reply.result.entities, reply

    async def mask(self, session: str, text: str) -> str:
        reply = await self.call({"op": "mask", "session": session, "text": text}, REQUEST_TIMEOUT)
        assert isinstance(reply.result, MaskResult)
        return reply.result.text

    async def unmask(self, session: str, text: str) -> str:
        reply = await self.call({"op": "unmask", "session": session, "text": text}, REQUEST_TIMEOUT)
        assert isinstance(reply.result, UnmaskResult)
        return reply.result.text

    async def unmask_stream(self, session: str, chunk: str, is_final: bool) -> str:
        request = {"op": "unmask_stream", "session": session, "chunk": chunk, "is_final": is_final}
        reply = await self.call(request, REQUEST_TIMEOUT)
        assert isinstance(reply.result, UnmaskStreamResult)
        return reply.result.text

    async def shutdown(self) -> None:
        """Asks a stdio worker to exit, then kills the process group and waits for it."""
        if self._session is None:
            try:
                await self.call({"op": "shutdown"}, SHUTDOWN_GRACE)
                await asyncio.wait_for(self._process.process.wait(), SHUTDOWN_GRACE)
            except (AdapterError, TimeoutError):
                pass  # the group is killed below either way
        else:
            await self._session.close()
        await self._process.terminate()
