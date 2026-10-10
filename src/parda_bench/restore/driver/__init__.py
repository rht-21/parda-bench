"""Client-framework drivers: run a scripted conversation against a base URL and report what the app saw."""

import tomllib
from pathlib import Path

from pydantic import ValidationError

from parda_bench.adapters.errors import AdapterError
from parda_bench.restore.driver.raw_http import RawHttpDriver
from parda_bench.restore.driver.stdio import StdioDriver
from parda_bench.spec.base import Strict
from parda_bench.spec.protocol import DriverRequest, DriverResponse
from parda_bench.spec.scenario import Api

RAW_HTTP = "raw_http"
REGISTRY_FILE = "drivers.toml"


class DriverSpec(Strict):
    command: list[str]
    """Run with the drivers directory as working directory."""
    apis: list[Api]


class DriverRegistry(Strict):
    drivers: dict[str, DriverSpec]


class Driver:
    """`raw_http` (built in) or a driver listed in `drivers/drivers.toml`. Always `close` it."""

    def __init__(self, name: str, apis: list[Api] | None, worker: StdioDriver | None) -> None:
        self.name = name
        self._apis = apis
        self._worker = worker
        self._raw = RawHttpDriver() if worker is None else None

    @staticmethod
    async def open(name: str, drivers_dir: Path) -> "Driver":
        if name == RAW_HTTP:
            return Driver(name, None, None)
        path = drivers_dir / REGISTRY_FILE
        try:
            registry = DriverRegistry.model_validate(tomllib.loads(path.read_text(encoding="utf-8")))
        except (OSError, tomllib.TOMLDecodeError, ValidationError) as e:
            raise AdapterError(f"reading driver registry {path}: {e}") from e
        if name not in registry.drivers:
            raise AdapterError(f"unknown driver `{name}`; available: {[RAW_HTTP, *sorted(registry.drivers)]}")
        spec = registry.drivers[name]
        return Driver(name, spec.apis, await StdioDriver.start(drivers_dir, spec.command))

    def supports(self, api: Api) -> bool:
        return self._apis is None or api in self._apis

    async def run(self, request: DriverRequest) -> DriverResponse:
        """Fails only when the driver itself breaks; HTTP and API errors are reported as observations."""
        if self._worker is not None:
            return await self._worker.run(request)
        assert self._raw is not None
        return await self._raw.run(request)

    async def close(self) -> None:
        if self._worker is not None:
            await self._worker.close()
        if self._raw is not None:
            await self._raw.close()
