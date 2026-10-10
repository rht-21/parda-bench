"""Reading `results/<run-id>/` directories and the datasets detection runs refer to."""

from dataclasses import dataclass
from pathlib import Path
from typing import TypeVar

from pydantic import BaseModel, ValidationError

from parda_bench.data.io import SAMPLES_FILE, file_sha256, read_samples
from parda_bench.spec.record import DetectionRecord, DetectionSuite, RestoreRecord, RunMeta
from parda_bench.spec.sample import Sample

META_FILE = "meta.json"
DETECTIONS_FILE = "detections.jsonl"
RESTORE_FILE = "restore.jsonl"
M = TypeVar("M", bound=BaseModel)


class RunsError(ValueError):
    pass


@dataclass(frozen=True)
class Run:
    meta: RunMeta
    detections: list[DetectionRecord]
    restores: list[RestoreRecord]


def _read_jsonl(path: Path, model: type[M]) -> list[M]:
    records = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if line.strip():
            try:
                records.append(model.model_validate_json(line))
            except ValidationError as e:
                raise RunsError(f"{path}: line {number}: {e}") from e
    return records


def load_runs(results: Path) -> list[Run]:
    """Every run directory under `results`, oldest first."""
    runs = []
    for directory in sorted(p for p in results.iterdir() if (p / META_FILE).is_file()):
        try:
            meta = RunMeta.model_validate_json((directory / META_FILE).read_text(encoding="utf-8"))
        except ValidationError as e:
            raise RunsError(f"{directory / META_FILE}: {e}") from e
        if isinstance(meta.suite, DetectionSuite):
            runs.append(Run(meta, _read_jsonl(directory / DETECTIONS_FILE, DetectionRecord), []))
        else:
            runs.append(Run(meta, [], _read_jsonl(directory / RESTORE_FILE, RestoreRecord)))
    return sorted(runs, key=lambda r: (r.meta.started_at, r.meta.run_id))


def load_dataset_for(data: Path, meta: RunMeta) -> list[Sample]:
    """The samples of `data/<version>/`, checked against the digest the run recorded."""
    suite = meta.suite
    if not isinstance(suite, DetectionSuite):
        raise RunsError(f"run {meta.run_id} is not a detection run")
    path = data / suite.dataset_version / SAMPLES_FILE
    try:
        found = file_sha256(path)
    except OSError as e:
        raise RunsError(f"{path}: {e}") from e
    if found != suite.dataset_sha256:
        raise RunsError(
            f"run {meta.run_id} used dataset {suite.dataset_version} with sha256 {suite.dataset_sha256}, "
            f"but {path} has {found}"
        )
    return read_samples(path)
