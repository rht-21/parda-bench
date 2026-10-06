"""A tool directory: `tools/<name>/tool.toml` plus the chosen named configuration."""

import tomllib
from dataclasses import dataclass
from pathlib import Path

from pydantic import ValidationError

from parda_bench.adapters.errors import AdapterError
from parda_bench.spec.manifest import Capability, HttpProxy, Stdio, ToolConfig, ToolManifest

CONFIG_FILE_ENV = "PARDA_TOOL_CONFIG"
"""Env var holding the absolute path of the selected config's `file`, when it has one."""


@dataclass(frozen=True)
class Tool:
    dir: Path
    manifest: ToolManifest
    config_name: str

    @staticmethod
    def load(directory: Path, config_name: str = "default") -> "Tool":
        """Loads and checks `directory/tool.toml`, selecting configuration `config_name`."""
        path = directory / "tool.toml"
        try:
            manifest = ToolManifest.model_validate(tomllib.loads(path.read_text(encoding="utf-8")))
        except (OSError, tomllib.TOMLDecodeError, ValidationError) as e:
            raise AdapterError(f"invalid manifest {path}: {e}") from e
        problem = _manifest_problem(manifest)
        if problem:
            raise AdapterError(f"invalid manifest {path}: {problem}")
        if config_name not in manifest.configs:
            raise AdapterError(f"{path}: no config named `{config_name}`; available: {sorted(manifest.configs)}")
        return Tool(directory.resolve(), manifest, config_name)

    @property
    def config(self) -> ToolConfig:
        return self.manifest.configs[self.config_name]

    def has(self, capability: Capability) -> bool:
        return capability in self.manifest.capabilities

    def env(self) -> dict[str, str]:
        """The config's `env`, plus `PARDA_TOOL_CONFIG` when it names a file."""
        env = dict(self.config.env)
        if self.config.file is not None:
            env[CONFIG_FILE_ENV] = str(self.dir / self.config.file)
        return env


def _manifest_problem(m: ToolManifest) -> str | None:
    if "default" not in m.configs:
        return "`configs.default` is required"
    if isinstance(m.transport, HttpProxy) != (Capability.PROXY in m.capabilities):
        return "the `proxy` capability goes with, and only with, the `http-proxy` transport"
    command = m.transport.command if isinstance(m.transport, Stdio) else m.transport.start
    if not command:
        return "the transport's command is empty"
    if Capability.UNMASK_STREAM in m.capabilities and Capability.MASK not in m.capabilities:
        return "`unmask_stream` requires `mask`"
    return None
