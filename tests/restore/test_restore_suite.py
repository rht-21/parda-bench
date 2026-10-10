import asyncio
from collections.abc import Awaitable, Callable
from pathlib import Path

from parda_bench.adapters.tool import Tool
from parda_bench.restore.driver import RAW_HTTP, Driver
from parda_bench.restore.runner import ToolRole, role, run_control, run_library, run_proxy
from parda_bench.restore.scenarios import load_dir
from parda_bench.spec.record import Fail, NotApplicable, Pass, RestoreOutcome, RestoreRecord
from parda_bench.spec.scenario import Category, CheckKind, Scenario

REPO = Path(__file__).resolve().parents[2]


def scenarios() -> list[Scenario]:
    return load_dir(REPO / "scenarios")


def by_id(records: list[RestoreRecord]) -> dict[str, RestoreOutcome]:
    return {r.scenario_id: r.outcome for r in records}


def failed(outcome: RestoreOutcome) -> list[CheckKind]:
    return [f.check for f in outcome.failures] if isinstance(outcome, Fail) else []


def test_repository_scenarios_cover_every_category() -> None:
    s = scenarios()
    assert len({x.category for x in s}) == 9 and len(s) >= 30


async def _with_raw_http(run: Callable[[Driver], Awaitable[list[RestoreRecord]]]) -> list[RestoreRecord]:
    driver = await Driver.open(RAW_HTTP, REPO / "drivers")
    try:
        return await run(driver)
    finally:
        await driver.close()


def test_control_run_fails_only_the_leak_check() -> None:
    all_scenarios = scenarios()
    outcomes = by_id(asyncio.run(_with_raw_http(lambda d: run_control(d, all_scenarios))))
    for s in all_scenarios:
        outcome = outcomes[s.id]
        if s.category is Category.PLACEHOLDER_MANGLING:
            assert isinstance(outcome, NotApplicable), s.id
            continue
        expected = [CheckKind.NO_LEAK_UPSTREAM] * len(s.pii) if CheckKind.NO_LEAK_UPSTREAM in s.checks else []
        assert failed(outcome) == expected, (s.id, outcome)


def test_library_run_with_regex_baseline() -> None:
    tool = Tool.load(REPO / "tools/regex-baseline")
    assert role(tool) is ToolRole.LIBRARY
    outcomes = by_id(asyncio.run(run_library(tool, scenarios(), None)))
    for sid in [
        "basic-openai-aadhaar",
        "split-openai-two-chunks",
        "split-openai-first-char",
        "tools-openai-stream-args-split",
    ]:
        assert isinstance(outcomes[sid], Pass), (sid, outcomes[sid])
    assert failed(outcomes["mangle-lowercase"]) == [
        CheckKind.RESTORED_EXACT,
        CheckKind.NO_PLACEHOLDER_FRAGMENT,
    ]
    assert failed(outcomes["basic-openai-contact"]) == [CheckKind.NO_LEAK_UPSTREAM]
    assert isinstance(outcomes["fail-openai-500"], NotApplicable)


def test_proxy_run_with_regex_proxy() -> None:
    tool = Tool.load(REPO / "tools/regex-proxy")
    assert role(tool) is ToolRole.PROXY
    all_scenarios = scenarios()
    outcomes = by_id(asyncio.run(_with_raw_http(lambda d: run_proxy(tool, d, all_scenarios, None))))
    for sid in [
        "basic-openai-aadhaar",
        "basic-anthropic-pan",
        "split-anthropic-two-values",
        "tools-openai-call-args",
        "tools-anthropic-stream-args",
        "multi-anthropic-repeat-value",
        "fail-openai-500",
        "fail-anthropic-529",
    ]:
        assert isinstance(outcomes[sid], Pass), (sid, outcomes[sid])
    assert CheckKind.NO_LEAK_UPSTREAM in failed(outcomes["tools-anthropic-roundtrip"])
