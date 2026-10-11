import json
from pathlib import Path

import pytest

from parda_bench.cli.main import main


def run(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, *args: str) -> int:
    monkeypatch.chdir(tmp_path)
    return main(list(args))


def test_builds_validates_and_describes_a_dataset(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    assert run(tmp_path, monkeypatch, "data", "build", "--per-template", "2") == 0
    assert run(tmp_path, monkeypatch, "data", "validate", "data/v0.2.0") == 0
    capsys.readouterr()
    assert run(tmp_path, monkeypatch, "data", "stats", "data/v0.2.0") == 0
    s = json.loads(capsys.readouterr().out)
    assert s["samples"] == s["positive_samples"] + s["hard_negatives"]


def test_rebuilding_different_content_under_the_same_version_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert run(tmp_path, monkeypatch, "data", "build", "--per-template", "1") == 0
    assert run(tmp_path, monkeypatch, "data", "build", "--per-template", "1") == 0
    assert run(tmp_path, monkeypatch, "data", "build", "--per-template", "1", "--seed", "7") == 1


def test_tampered_dataset_fails_validation(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    assert run(tmp_path, monkeypatch, "data", "build", "--per-template", "1") == 0
    samples = tmp_path / "data/v0.2.0/samples.jsonl"
    samples.write_text(samples.read_text().replace("Aadhaar", "Aadhar", 1))
    assert run(tmp_path, monkeypatch, "data", "validate", "data/v0.2.0") == 1


def test_report_on_empty_results_says_so(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    (tmp_path / "results").mkdir()
    assert run(tmp_path, monkeypatch, "report") == 0
    assert "No runs found." in (tmp_path / "reports/report.md").read_text()


def test_published_datasets_rebuild_byte_for_byte(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo = Path(__file__).resolve().parents[2]
    for version in ("v0.1.0", "v0.2.0"):
        assert run(tmp_path, monkeypatch, "data", "build", "--version", version) == 0
        assert (tmp_path / "data" / version / "samples.jsonl").read_bytes() == (
            repo / "data" / version / "samples.jsonl"
        ).read_bytes()
