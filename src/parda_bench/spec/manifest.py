from enum import StrEnum
from typing import Annotated, Literal

from pydantic import Field

from parda_bench.spec.base import Strict
from parda_bench.spec.entity import EntityType


class Capability(StrEnum):
    DETECT = "detect"
    MASK = "mask"
    UNMASK = "unmask"
    UNMASK_STREAM = "unmask_stream"
    PROXY = "proxy"


class Stdio(Strict):
    """Long-lived worker speaking the NDJSON adapter protocol on stdin/stdout."""

    kind: Literal["stdio"]
    command: list[str]


class HttpProxy(Strict):
    """OpenAI/Anthropic-compatible proxy; the harness sets `upstream_env` to the mock upstream URL."""

    kind: Literal["http-proxy"]
    start: list[str]
    base_url: str
    upstream_env: str


class HttpApi(Strict):
    """Service that takes each adapter request as a JSON POST to `base_url`."""

    kind: Literal["http-api"]
    start: list[str]
    base_url: str


Transport = Annotated[Stdio | HttpProxy | HttpApi, Field(discriminator="kind")]


class ToolConfig(Strict):
    file: str | None = None
    """Tool-specific config file, relative to the tool directory."""
    env: dict[str, str] = Field(default_factory=dict)


class ToolManifest(Strict):
    """Contents of `tools/<name>/tool.toml`. Commands run with the tool directory as working directory."""

    name: str
    version: str
    commit: str
    license: str
    language: str
    homepage: str
    transport: Transport
    capabilities: list[Capability]
    entity_map: dict[str, EntityType]
    """Tool's own label to Parda entity type; labels missing here are scored as unmapped, never dropped."""
    configs: dict[str, ToolConfig]
    """Named configurations; `default` is required, `tuned` only when the tool's docs recommend one."""
