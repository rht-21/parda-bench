"""Dataset files: `data/<version>/samples.jsonl` and its `manifest.json`."""

import hashlib
import json
from pathlib import Path

from pydantic import ValidationError

from parda_bench.spec.base import Strict
from parda_bench.spec.sample import Sample

SAMPLES_FILE = "samples.jsonl"
MANIFEST_FILE = "manifest.json"


class DatasetManifest(Strict):
    """`manifest.json`: how a dataset was built and the digest of its samples file."""

    version: str
    seed: int
    per_template: int
    generator_version: str
    sample_count: int
    samples_sha256: str


class DataIoError(Exception):
    pass


def sample_line(sample: Sample) -> str:
    return json.dumps(sample.model_dump(mode="json"), ensure_ascii=False, separators=(",", ":")) + "\n"


def samples_sha256(samples: list[Sample]) -> str:
    """SHA-256 of `samples` serialized exactly as `write_dataset` writes them."""
    return hashlib.sha256("".join(map(sample_line, samples)).encode()).hexdigest()


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_dataset(directory: Path, samples: list[Sample], manifest: DatasetManifest) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / SAMPLES_FILE).write_text("".join(map(sample_line, samples)), encoding="utf-8")
    (directory / MANIFEST_FILE).write_text(manifest.model_dump_json(indent=2) + "\n", encoding="utf-8")


def read_samples(path: Path) -> list[Sample]:
    """Reads a JSONL samples file; raises `DataIoError` naming the line of the first bad record."""
    samples = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            samples.append(Sample.model_validate_json(line))
        except ValidationError as e:
            raise DataIoError(f"{path}:{number}: {e}") from e
    return samples


def read_manifest(directory: Path) -> DatasetManifest:
    path = directory / MANIFEST_FILE
    try:
        return DatasetManifest.model_validate_json(path.read_text(encoding="utf-8"))
    except (OSError, ValidationError) as e:
        raise DataIoError(f"{path}: {e}") from e
