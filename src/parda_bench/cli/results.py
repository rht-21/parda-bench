"""A run's directory: `meta.json`, one JSONL record file, and the tool's log."""

import json
from datetime import UTC, datetime
from pathlib import Path
from typing import IO

from pydantic import BaseModel

from parda_bench import __version__
from parda_bench.adapters.tool import Tool
from parda_bench.cli.hardware import probe
from parda_bench.report.runs import META_FILE
from parda_bench.spec.record import CONTROL_TOOL, DetectionSuite, RestoreSuite, RunMeta, ToolRef


class RunDir:
    def __init__(self, path: Path, run_id: str, records: IO[str]) -> None:
        self.path = path
        self.run_id = run_id
        self._records = records

    @staticmethod
    def create(results: Path, tool: Tool, suite: DetectionSuite | RestoreSuite, records_file: str) -> "RunDir":
        m = tool.manifest
        ref = ToolRef(name=m.name, version=m.version, commit=m.commit, config=tool.config_name)
        return RunDir._create(results, ref, suite, records_file)

    @staticmethod
    def create_control(results: Path, driver: str, records_file: str) -> "RunDir":
        """A restore run with no tool between the driver and the mock."""
        ref = ToolRef(name=CONTROL_TOOL, version="-", commit="-", config="default")
        return RunDir._create(results, ref, RestoreSuite(driver=driver), records_file)

    @staticmethod
    def _create(results: Path, tool: ToolRef, suite: DetectionSuite | RestoreSuite, records_file: str) -> "RunDir":
        started_at = datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
        target = "detect" if isinstance(suite, DetectionSuite) else f"restore-{suite.driver}"
        stamp = "".join(c for c in started_at if c.isalnum())
        run_id = f"{stamp}-{tool.name}-{tool.config}-{target}"
        path = results / run_id
        results.mkdir(parents=True, exist_ok=True)
        try:
            path.mkdir()  # never reuse a run directory: records from two runs must not mix
        except FileExistsError as e:
            raise FileExistsError(f"{path}: a run with this id already exists; retry in a second") from e
        meta = RunMeta(
            run_id=run_id,
            started_at=started_at,
            harness_version=__version__,
            tool=tool,
            hardware=probe(),
            suite=suite,
        )
        (path / META_FILE).write_text(meta.model_dump_json(indent=2) + "\n", encoding="utf-8")
        return RunDir(path, run_id, (path / records_file).open("w", encoding="utf-8"))

    @property
    def tool_log(self) -> Path:
        return self.path / "tool.log"

    def append(self, record: BaseModel) -> None:
        """Appends one record and flushes, so an interrupted run keeps everything recorded so far."""
        self._records.write(
            json.dumps(record.model_dump(mode="json"), ensure_ascii=False, separators=(",", ":")) + "\n"
        )
        self._records.flush()

    def close(self) -> None:
        self._records.close()
