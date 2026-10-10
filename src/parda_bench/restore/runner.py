"""Runs scenarios against a target and scores each one."""

import logging
from enum import Enum
from pathlib import Path

from parda_bench.adapters.adapter import STARTUP_TIMEOUT, Adapter
from parda_bench.adapters.errors import AdapterError, ToolError, TransportError
from parda_bench.adapters.process import spawn_service, wait_until_listening
from parda_bench.adapters.tool import Tool
from parda_bench.restore.align import render_input
from parda_bench.restore.checks import evaluate
from parda_bench.restore.driver import Driver
from parda_bench.restore.engine import Engine, HttpErrorScript, MockError, TextScript, UpstreamLog
from parda_bench.restore.mock import MockUpstream
from parda_bench.spec.manifest import Capability, HttpProxy
from parda_bench.spec.protocol import (
    Completed,
    DriverRequest,
    DriverTurn,
    Errored,
    Observation,
    ObservedToolCall,
    ToolResultTurn,
    UserTurn,
)
from parda_bench.spec.record import CheckFailure, Fail, NotApplicable, RestoreOutcome, RestoreRecord
from parda_bench.spec.scenario import Category, CheckKind, Scenario, UserInput, segments

MODEL = "parda-mock"
LIBRARY_DRIVER = "library"
"""Label used as the driver of a library-tool run, where the harness calls the tool directly."""

log = logging.getLogger(__name__)


class ToolRole(Enum):
    PROXY = "proxy"
    """Sits between client and upstream; exercised through drivers."""
    LIBRARY = "library"
    """Called by the harness around the upstream request."""
    REDACT_ONLY = "redact_only"
    """Cannot restore values; every scenario is not applicable."""


def role(tool: Tool) -> ToolRole:
    if tool.has(Capability.PROXY):
        return ToolRole.PROXY
    if tool.has(Capability.MASK) and tool.has(Capability.UNMASK):
        return ToolRole.LIBRARY
    return ToolRole.REDACT_ONLY


async def run_proxy(
    tool: Tool, driver: Driver, scenarios: list[Scenario], log_path: Path | None
) -> list[RestoreRecord]:
    """Every scenario through `driver` into a proxy tool, which forwards to the mock upstream."""
    transport = tool.manifest.transport
    if not isinstance(transport, HttpProxy):
        raise AdapterError(f"{tool.manifest.name} is not an http-proxy tool")
    mock = MockUpstream()
    await mock.start()
    try:
        proxy = await spawn_service(
            tool.dir, transport.start, {**tool.env(), transport.upstream_env: mock.base_url}, log_path
        )
        try:
            await wait_until_listening(transport.base_url, proxy, STARTUP_TIMEOUT)
            return await _run_with_driver(driver, mock, transport.base_url, scenarios)
        finally:
            await proxy.terminate()
    finally:
        await mock.stop()


async def run_control(driver: Driver, scenarios: list[Scenario]) -> list[RestoreRecord]:
    """Every scenario through `driver` straight into the mock, with no tool: checks the drivers and the suite.

    Every `no_leak_upstream` check is expected to fail; mangling scenarios are skipped because their script alters
    the raw value itself, which says nothing about a driver.
    """
    runnable = [s for s in scenarios if s.category is not Category.PLACEHOLDER_MANGLING]
    mock = MockUpstream()
    await mock.start()
    try:
        records = await _run_with_driver(driver, mock, mock.base_url, runnable)
    finally:
        await mock.stop()
    records += [
        RestoreRecord(
            scenario_id=s.id,
            outcome=NotApplicable(reason="control run: the script alters the raw value itself"),
        )
        for s in scenarios
        if s.category is Category.PLACEHOLDER_MANGLING
    ]
    return sorted(records, key=lambda r: r.scenario_id)


async def _run_with_driver(
    driver: Driver, mock: MockUpstream, base_url: str, scenarios: list[Scenario]
) -> list[RestoreRecord]:
    records = []
    for n, scenario in enumerate(scenarios, 1):
        outcome: RestoreOutcome
        if not driver.supports(scenario.api):
            outcome = NotApplicable(reason=f"driver {driver.name} does not speak {scenario.api.value}")
        else:
            mock.load(scenario)
            response = await driver.run(driver_request(n, base_url, scenario))
            outcome = evaluate(scenario, mock.finish(), response.observations)
        records.append(RestoreRecord(scenario_id=scenario.id, outcome=outcome))
    return records


async def run_library(tool: Tool, scenarios: list[Scenario], log_path: Path | None) -> list[RestoreRecord]:
    """Every scenario through a library tool: the harness masks the input, plays the upstream in process, and
    unmasks the reply (chunk by chunk with `unmask_stream` when the scenario streams)."""
    adapter: Adapter | None = None
    records = []
    try:
        for n, scenario in enumerate(scenarios):
            reason = _library_not_applicable(tool, scenario)
            if reason:
                records.append(RestoreRecord(scenario_id=scenario.id, outcome=NotApplicable(reason=reason)))
                continue
            if adapter is None:
                adapter = await Adapter.start(tool, log_path)
            try:
                upstream, observations = await _run_library_scenario(adapter, f"{scenario.id}#{n}", scenario)
                outcome = evaluate(scenario, upstream, observations)
            except TransportError as e:
                log.warning("scenario %s: restarting tool after: %s", scenario.id, e)
                await adapter.shutdown()
                adapter = None
                outcome = Fail(
                    failures=[CheckFailure(check=CheckKind.RESTORED_EXACT, detail=f"tool stopped responding: {e}")],
                    upstream_excerpt="",
                    client_excerpt="",
                )
            records.append(RestoreRecord(scenario_id=scenario.id, outcome=outcome))
    finally:
        if adapter is not None:
            await adapter.shutdown()
    return records


def _library_not_applicable(tool: Tool, scenario: Scenario) -> str | None:
    if scenario.stream and not tool.has(Capability.UNMASK_STREAM):
        return "streaming scenario; tool has no unmask_stream"
    if CheckKind.FAILS_CLOSED in scenario.checks:
        return "failure behaviour is a property of proxies; a library tool has no upstream to fail"
    return None


async def _run_library_scenario(
    adapter: Adapter, session: str, scenario: Scenario
) -> tuple[UpstreamLog, list[Observation]]:
    """Tool-reported errors become `errored` observations; transport failures propagate."""
    engine = Engine(scenario)
    observations: list[Observation] = []
    for turn, script in enumerate(scenario.turns):
        try:
            masked = await adapter.mask(session, render_input(segments(script.input), scenario.pii))
        except ToolError as e:
            observations.append(Errored(message=str(e)))
            continue
        engine.record_payload(masked)
        reply = engine.reply(turn, masked)
        if isinstance(reply, HttpErrorScript):
            observations.append(Errored(message=reply.body, http_status=reply.status))
        elif isinstance(reply, MockError):
            observations.append(Errored(message=reply.message))
        else:
            chunks = reply.chunks if isinstance(reply, TextScript) else reply.argument_chunks
            try:
                restored = await _restore(adapter, session, scenario.stream, chunks)
            except ToolError as e:
                observations.append(Errored(message=str(e)))
                continue
            if isinstance(reply, TextScript):
                observations.append(Completed(text=restored))
            else:
                call = ObservedToolCall(id=reply.call_id, name=reply.name, arguments=restored)
                observations.append(Completed(text="", tool_calls=[call]))
    return engine.log, observations


async def _restore(adapter: Adapter, session: str, stream: bool, chunks: list[str]) -> str:
    if not stream:
        return await adapter.unmask(session, "".join(chunks))
    parts = [await adapter.unmask_stream(session, c, i == len(chunks) - 1) for i, c in enumerate(chunks)]
    return "".join(parts)


def driver_request(request_id: int, base_url: str, scenario: Scenario) -> DriverRequest:
    """The conversation a driver plays: each turn's input with real values filled in."""
    turns: list[DriverTurn] = []
    for turn in scenario.turns:
        text = render_input(segments(turn.input), scenario.pii)
        if isinstance(turn.input, UserInput):
            turns.append(UserTurn(text=text))
        else:
            turns.append(ToolResultTurn(call_id=turn.input.call_id, content=text))
    return DriverRequest(
        id=request_id,
        base_url=base_url,
        api=scenario.api,
        model=MODEL,
        stream=scenario.stream,
        tools=scenario.tools,
        turns=turns,
    )


def redact_only(scenarios: list[Scenario]) -> list[RestoreRecord]:
    """Outcome for every scenario of a tool that cannot restore values."""
    reason = "tool cannot unmask (redact-only)"
    return [RestoreRecord(scenario_id=s.id, outcome=NotApplicable(reason=reason)) for s in scenarios]
