import asyncio
from collections.abc import Awaitable, Callable
from pathlib import Path

import pytest

from parda_bench.adapters.adapter import Adapter
from parda_bench.adapters.detect import DetectionRunner, map_entities
from parda_bench.adapters.errors import AdapterError, CapabilityMismatch, ToolError, TransportError
from parda_bench.adapters.tool import CONFIG_FILE_ENV, Tool
from parda_bench.spec.entity import EntityType
from parda_bench.spec.protocol import DetectedEntity
from parda_bench.spec.record import Detected, Failed, Mapped, Unmapped
from parda_bench.spec.sample import Difficulty, Lang, Positive, Sample, Source

FIXTURES = Path(__file__).parent / "fixtures"


def sample(text: str) -> Sample:
    return Sample(
        id=text,
        text=text,
        labels=Positive(spans=[]),
        lang=Lang.EN,
        difficulty=Difficulty.EASY,
        source=Source.HANDWRITTEN,
        generator_version="t",
    )


def with_runner(body: Callable[[DetectionRunner], Awaitable[None]]) -> None:
    async def run() -> None:
        runner = DetectionRunner(Tool.load(FIXTURES / "fake-tool"), None)
        try:
            await body(runner)
        finally:
            await runner.shutdown()

    asyncio.run(run())


def test_detects_and_maps_labels_with_both_latencies() -> None:
    async def body(runner: DetectionRunner) -> None:
        result = (await runner.detect(sample("Rahul"))).result
        assert isinstance(result, Detected)
        assert result.predicted[0].entity == Mapped(mapped=EntityType.PERSON_NAME)
        assert result.predicted[1].entity == Unmapped(unmapped="INITIAL")
        assert result.wall_ns >= result.elapsed_ns

    with_runner(body)


def test_tool_error_is_recorded_and_run_continues() -> None:
    async def body(runner: DetectionRunner) -> None:
        assert (await runner.detect(sample("error"))).result == Failed(message="RuntimeError: tool rejected input")
        assert isinstance((await runner.detect(sample("next"))).result, Detected)

    with_runner(body)


@pytest.mark.parametrize("trigger", ["crash", "garbage"])
def test_broken_worker_is_restarted_for_the_next_sample(trigger: str) -> None:
    async def body(runner: DetectionRunner) -> None:
        assert isinstance((await runner.detect(sample(trigger))).result, Failed)
        assert isinstance((await runner.detect(sample("after"))).result, Detected)

    with_runner(body)


def test_slow_reply_times_out() -> None:
    async def run() -> None:
        adapter = await Adapter.start(Tool.load(FIXTURES / "fake-tool"), None)
        try:
            with pytest.raises(TransportError, match="within"):
                await adapter.call({"op": "detect", "text": "hang"}, 0.3)
        finally:
            await adapter.shutdown()

    asyncio.run(run())


def test_capabilities_differing_from_manifest_fail_the_handshake() -> None:
    with pytest.raises(CapabilityMismatch):
        asyncio.run(Adapter.start(Tool.load(FIXTURES / "fake-tool", "overclaims"), None))


def test_http_api_tool_detects_reports_errors_and_stops() -> None:
    async def run() -> None:
        adapter = await Adapter.start(Tool.load(FIXTURES / "fake-http-tool"), None)
        try:
            entities, reply = await adapter.detect("Rahul")
            assert (entities[0].start, entities[0].end, entities[0].label, reply.elapsed_ns) == (
                0,
                5,
                "WORD",
                1000,
            )
            with pytest.raises(ToolError, match="tool rejected input"):
                await adapter.detect("error")
        finally:
            await adapter.shutdown()
        with pytest.raises(OSError):
            await asyncio.open_connection("127.0.0.1", 18432)

    asyncio.run(run())


def test_maps_known_labels_and_keeps_unknown_ones() -> None:
    entities = [
        DetectedEntity(start=0, end=10, label="IN_PAN", score=0.9),
        DetectedEntity(start=11, end=15, label="ORG"),
    ]
    mapped = map_entities(entities, {"IN_PAN": EntityType.PAN})
    assert [p.entity for p in mapped] == [Mapped(mapped=EntityType.PAN), Unmapped(unmapped="ORG")]


MANIFEST = """
name = "t"
version = "1"
commit = "abc"
license = "MIT"
language = "python"
homepage = "https://example.org"
capabilities = ["detect"]
transport = { kind = "stdio", command = ["python", "adapter.py"] }
[entity_map]
IN_PAN = "PAN"
[configs.default]
[configs.tuned]
file = "tuned.yaml"
env = { MODE = "fast" }
"""


def write_tool(tmp_path: Path, toml: str) -> Path:
    (tmp_path / "tool.toml").write_text(toml)
    return tmp_path


def test_loads_manifest_and_builds_config_env(tmp_path: Path) -> None:
    tool = Tool.load(write_tool(tmp_path, MANIFEST), "tuned")
    assert tool.env()["MODE"] == "fast" and tool.env()[CONFIG_FILE_ENV].endswith("tuned.yaml")
    assert Tool.load(tmp_path).env() == {}


@pytest.mark.parametrize(
    "toml",
    [
        MANIFEST.replace('["detect"]', '["detect", "proxy"]'),
        MANIFEST.replace("[configs.default]\n", ""),
        MANIFEST.replace("[entity_map]", "entitymap = {}\n[entity_map]"),
    ],
)
def test_rejects_bad_manifests(tmp_path: Path, toml: str) -> None:
    with pytest.raises(AdapterError):
        Tool.load(write_tool(tmp_path, toml), "tuned")


def test_rejects_unknown_config(tmp_path: Path) -> None:
    with pytest.raises(AdapterError, match="fastest"):
        Tool.load(write_tool(tmp_path, MANIFEST), "fastest")
