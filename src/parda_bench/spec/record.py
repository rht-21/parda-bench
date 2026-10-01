"""Result files written under `results/<run-id>/`; every report number traces back to these."""

from typing import Annotated, Literal

from pydantic import Field

from parda_bench.spec.base import Strict
from parda_bench.spec.entity import EntityType
from parda_bench.spec.scenario import CheckKind

CONTROL_TOOL = "control"
"""`ToolRef.name` of a control run: drivers talk straight to the mock upstream, with no tool in between."""


class ToolRef(Strict):
    name: str
    version: str
    commit: str
    config: str


class Hardware(Strict):
    os: str
    arch: str
    cpu: str
    cores: int
    memory_mb: int


class DetectionSuite(Strict):
    kind: Literal["detection"] = "detection"
    dataset_version: str
    dataset_sha256: str


class RestoreSuite(Strict):
    kind: Literal["restore"] = "restore"
    driver: str


Suite = Annotated[DetectionSuite | RestoreSuite, Field(discriminator="kind")]


class RunMeta(Strict):
    """`meta.json` of one run."""

    run_id: str
    started_at: str
    """RFC 3339 timestamp."""
    harness_version: str
    tool: ToolRef
    hardware: Hardware
    suite: Suite


class Mapped(Strict):
    mapped: EntityType


class Unmapped(Strict):
    unmapped: str


PredictedEntity = Mapped | Unmapped


class PredictedSpan(Strict):
    start: int
    end: int
    entity: PredictedEntity


class Detected(Strict):
    """`elapsed_ns` is tool-internal; `wall_ns` is measured by the harness and includes IPC."""

    status: Literal["detected"] = "detected"
    predicted: list[PredictedSpan]
    elapsed_ns: int
    wall_ns: int


class Failed(Strict):
    status: Literal["failed"] = "failed"
    message: str


DetectionResult = Annotated[Detected | Failed, Field(discriminator="status")]


class DetectionRecord(Strict):
    """One line of `detections.jsonl`."""

    sample_id: str
    result: DetectionResult


class CheckFailure(Strict):
    check: CheckKind
    detail: str


class Pass(Strict):
    status: Literal["pass"] = "pass"


class Fail(Strict):
    status: Literal["fail"] = "fail"
    failures: list[CheckFailure]
    upstream_excerpt: str
    client_excerpt: str


class NotApplicable(Strict):
    status: Literal["not_applicable"] = "not_applicable"
    reason: str


RestoreOutcome = Annotated[Pass | Fail | NotApplicable, Field(discriminator="status")]


class RestoreRecord(Strict):
    """One line of `restore.jsonl`."""

    scenario_id: str
    outcome: RestoreOutcome
