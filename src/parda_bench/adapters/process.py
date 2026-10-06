"""Starting tool processes and waiting for services to listen.

Each process runs in its own process group, and `terminate` kills the whole group: launchers such as `uv run`
keep the real tool as a child that would otherwise outlive them.
"""

import asyncio
import contextlib
import os
import signal
import time
from pathlib import Path
from typing import IO
from urllib.parse import urlsplit

from parda_bench.adapters.errors import AdapterError

READY_POLL = 0.1
LINE_LIMIT = 64 * 1024 * 1024
"""Largest protocol line accepted from a worker."""


class GroupProcess:
    """A child process and everything it started."""

    def __init__(self, process: asyncio.subprocess.Process, log: IO[bytes] | None) -> None:
        self.process = process
        self._log = log

    def kill_group(self) -> None:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(self.process.pid, signal.SIGKILL)

    async def terminate(self) -> None:
        """Kills the process group and waits until the direct child has exited, releasing its ports and files."""
        self.kill_group()
        await self.process.wait()
        if self._log is not None:
            self._log.close()


async def _spawn(
    directory: Path,
    command: list[str],
    env: dict[str, str],
    log_file: IO[bytes] | None,
    *,
    stdin: int,
    stdout: int | IO[bytes] | None,
) -> GroupProcess:
    if not command:
        raise AdapterError(f"empty command for {directory}")
    program, *args = command
    # A relative program path like `./run.sh` would otherwise resolve against the harness's working directory.
    if "/" in program and not os.path.isabs(program):
        program = str(directory / program)
    try:
        process = await asyncio.create_subprocess_exec(
            program,
            *args,
            cwd=directory,
            env={**os.environ, **env},
            start_new_session=True,
            limit=LINE_LIMIT,
            stdin=stdin,
            stdout=stdout,
            stderr=log_file,
        )
    except OSError as e:
        if log_file is not None:
            log_file.close()
        raise AdapterError(f"starting `{' '.join(command)}` in {directory}: {e}") from e
    return GroupProcess(process, log_file)


async def spawn_worker(directory: Path, command: list[str], env: dict[str, str], log: Path | None) -> GroupProcess:
    """A stdio worker: stdin and stdout are piped for the protocol, stderr goes to `log` (or the terminal)."""
    log_file = log.open("ab") if log is not None else None
    return await _spawn(
        directory, command, env, log_file, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE
    )


async def spawn_service(directory: Path, command: list[str], env: dict[str, str], log: Path | None) -> GroupProcess:
    """A long-running service (HTTP proxy or API); both output streams go to `log` (or the terminal)."""
    log_file = log.open("ab") if log is not None else None
    return await _spawn(directory, command, env, log_file, stdin=asyncio.subprocess.DEVNULL, stdout=log_file)


async def wait_until_listening(base_url: str, service: GroupProcess, timeout: float) -> None:
    """Polls until `base_url`'s host and port accept a TCP connection."""
    url = urlsplit(base_url)
    port = url.port or (443 if url.scheme == "https" else 80)
    deadline = time.monotonic() + timeout
    while True:
        try:
            _, writer = await asyncio.open_connection(url.hostname, port)
            writer.close()
            return
        except OSError:
            pass
        if service.process.returncode is not None:
            raise AdapterError(f"service exited with {service.process.returncode} before listening on {base_url}")
        if time.monotonic() >= deadline:
            raise AdapterError(f"{base_url} did not accept connections within {timeout}s")
        await asyncio.sleep(READY_POLL)
