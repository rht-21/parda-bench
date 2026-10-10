"""Out-of-process driver speaking `driver-request` / `driver-response` NDJSON on stdin/stdout."""

import asyncio
from pathlib import Path

from pydantic import ValidationError

from parda_bench.adapters.errors import TransportError
from parda_bench.adapters.process import GroupProcess, spawn_worker
from parda_bench.spec.protocol import DriverRequest, DriverResponse

CONVERSATION_TIMEOUT = 300.0
"""A whole conversation; turns are individually bounded by the driver's own HTTP timeouts."""


class StdioDriver:
    def __init__(self, process: GroupProcess) -> None:
        self._process = process

    @staticmethod
    async def start(directory: Path, command: list[str]) -> "StdioDriver":
        return StdioDriver(await spawn_worker(directory, command, {}, None))

    async def run(self, request: DriverRequest) -> DriverResponse:
        stdin, stdout = self._process.process.stdin, self._process.process.stdout
        assert stdin is not None and stdout is not None
        stdin.write(request.model_dump_json().encode() + b"\n")
        try:
            await stdin.drain()
            line = await asyncio.wait_for(stdout.readline(), CONVERSATION_TIMEOUT)
        except TimeoutError as e:
            raise TransportError(f"driver did not finish the conversation within {CONVERSATION_TIMEOUT}s") from e
        if not line:
            raise TransportError("driver exited without answering")
        try:
            response = DriverResponse.model_validate_json(line)
        except ValidationError as e:
            raise TransportError(f"driver sent an invalid response ({e}): {line!r}") from e
        if response.id != request.id:
            raise TransportError(f"driver answered request {response.id}, expected {request.id}")
        return response

    async def close(self) -> None:
        await self._process.terminate()
