"""Wire protocols for out-of-process workers: one JSON object per line on stdin (requests) and stdout (responses).

Text offsets in replies follow the `Span` convention (Unicode code points).
"""

from typing import Annotated, Literal

from pydantic import Field

from parda_bench.spec.base import Lenient, Strict
from parda_bench.spec.manifest import Capability
from parda_bench.spec.scenario import Api, ToolDefinition


class HelloRequest(Strict):
    id: int
    op: Literal["hello"] = "hello"


class DetectRequest(Strict):
    id: int
    op: Literal["detect"] = "detect"
    text: str


class MaskRequest(Strict):
    """Mask `text`, remembering placeholders under `session` for later unmasking."""

    id: int
    op: Literal["mask"] = "mask"
    session: str
    text: str


class UnmaskRequest(Strict):
    id: int
    op: Literal["unmask"] = "unmask"
    session: str
    text: str


class UnmaskStreamRequest(Strict):
    """Feed one streamed chunk; `is_final` tells the tool to flush anything it held back."""

    id: int
    op: Literal["unmask_stream"] = "unmask_stream"
    session: str
    chunk: str
    is_final: bool


class ShutdownRequest(Strict):
    id: int
    op: Literal["shutdown"] = "shutdown"


AdapterRequest = Annotated[
    HelloRequest | DetectRequest | MaskRequest | UnmaskRequest | UnmaskStreamRequest | ShutdownRequest,
    Field(discriminator="op"),
]


class DetectedEntity(Lenient):
    """An entity as the tool reports it, before mapping to `EntityType`."""

    start: int
    end: int
    label: str
    score: float | None = None


class HelloResult(Lenient):
    op: Literal["hello"]
    tool_version: str
    capabilities: list[Capability]


class DetectResult(Lenient):
    op: Literal["detect"]
    entities: list[DetectedEntity]


class MaskResult(Lenient):
    op: Literal["mask"]
    text: str


class UnmaskResult(Lenient):
    op: Literal["unmask"]
    text: str


class UnmaskStreamResult(Lenient):
    """Text safe to emit now; the tool may hold back a partial placeholder until a later chunk."""

    op: Literal["unmask_stream"]
    text: str


class ShutdownResult(Lenient):
    op: Literal["shutdown"]


AdapterResult = Annotated[
    HelloResult | DetectResult | MaskResult | UnmaskResult | UnmaskStreamResult | ShutdownResult,
    Field(discriminator="op"),
]


class OkResponse(Lenient):
    """`elapsed_ns` is measured inside the worker, so it excludes IPC overhead."""

    id: int
    status: Literal["ok"]
    elapsed_ns: int
    result: AdapterResult


class ErrorResponse(Lenient):
    id: int
    status: Literal["error"]
    message: str


AdapterResponse = Annotated[OkResponse | ErrorResponse, Field(discriminator="status")]


class UserTurn(Strict):
    kind: Literal["user"] = "user"
    text: str


class ToolResultTurn(Strict):
    kind: Literal["tool_result"] = "tool_result"
    call_id: str
    content: str


DriverTurn = Annotated[UserTurn | ToolResultTurn, Field(discriminator="kind")]


class DriverRequest(Strict):
    """Harness to client-framework driver: run one conversation against `base_url` with real values."""

    id: int
    base_url: str
    api: Api
    model: str
    stream: bool
    tools: list[ToolDefinition]
    turns: list[DriverTurn]


class ObservedToolCall(Lenient):
    id: str
    name: str
    arguments: str
    """Raw JSON string exactly as the client library surfaced it."""


class Completed(Lenient):
    kind: Literal["completed"] = "completed"
    text: str
    tool_calls: list[ObservedToolCall] = Field(default_factory=list)


class Errored(Lenient):
    kind: Literal["errored"] = "errored"
    message: str
    http_status: int | None = None


Observation = Annotated[Completed | Errored, Field(discriminator="kind")]


class DriverResponse(Lenient):
    """Driver to harness: what the application saw, one observation per turn."""

    id: int
    observations: list[Observation]
