import json
from pathlib import Path
from typing import Any

from pydantic import TypeAdapter

from parda_bench.spec.manifest import ToolManifest
from parda_bench.spec.protocol import AdapterRequest, AdapterResponse, DriverRequest, DriverResponse
from parda_bench.spec.record import DetectionRecord, RestoreRecord, RunMeta
from parda_bench.spec.sample import Sample
from parda_bench.spec.scenario import Scenario

CONTRACTS: dict[str, Any] = {
    "sample": Sample,
    "tool-manifest": ToolManifest,
    "adapter-request": AdapterRequest,
    "adapter-response": AdapterResponse,
    "driver-request": DriverRequest,
    "driver-response": DriverResponse,
    "scenario": Scenario,
    "detection-record": DetectionRecord,
    "restore-record": RestoreRecord,
    "run-meta": RunMeta,
}


def schemas() -> dict[str, dict[str, Any]]:
    """Every published contract's JSON Schema, keyed by the file stem used under `spec/`."""
    return {name: TypeAdapter(model).json_schema(by_alias=True) for name, model in CONTRACTS.items()}


def write_schemas(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    for name, schema in schemas().items():
        (out / f"{name}.schema.json").write_text(json.dumps(schema, indent=2, ensure_ascii=False) + "\n")
