"""Runs detection over samples, mapping tool labels to entity types and restarting a worker that crashed."""

import logging
from pathlib import Path

from parda_bench.adapters.adapter import Adapter
from parda_bench.adapters.errors import ToolError, TransportError
from parda_bench.adapters.tool import Tool
from parda_bench.spec.entity import EntityType
from parda_bench.spec.protocol import DetectedEntity
from parda_bench.spec.record import (
    Detected,
    DetectionRecord,
    DetectionResult,
    Failed,
    Mapped,
    PredictedSpan,
    Unmapped,
)
from parda_bench.spec.sample import Sample

log = logging.getLogger(__name__)


def map_entities(entities: list[DetectedEntity], entity_map: dict[str, EntityType]) -> list[PredictedSpan]:
    """Translates tool labels through `entity_map`; unknown labels are kept as `Unmapped`."""
    return [
        PredictedSpan(
            start=e.start,
            end=e.end,
            entity=Mapped(mapped=entity_map[e.label]) if e.label in entity_map else Unmapped(unmapped=e.label),
        )
        for e in entities
    ]


class DetectionRunner:
    def __init__(self, tool: Tool, log_path: Path | None) -> None:
        self._tool = tool
        self._log_path = log_path
        self._adapter: Adapter | None = None

    async def detect(self, sample: Sample) -> DetectionRecord:
        """A tool error becomes a `failed` record; a transport failure also restarts the worker for later samples."""
        if self._adapter is None:
            self._adapter = await Adapter.start(self._tool, self._log_path)
        result: DetectionResult
        try:
            entities, reply = await self._adapter.detect(sample.text)
            predicted = map_entities(entities, self._tool.manifest.entity_map)
            result = Detected(predicted=predicted, elapsed_ns=reply.elapsed_ns, wall_ns=reply.wall_ns)
        except ToolError as e:
            result = Failed(message=str(e))
        except TransportError as e:
            log.warning("sample %s: restarting worker after: %s", sample.id, e)
            await self.shutdown()
            result = Failed(message=str(e))
        return DetectionRecord(sample_id=sample.id, result=result)

    async def shutdown(self) -> None:
        if self._adapter is not None:
            adapter, self._adapter = self._adapter, None
            await adapter.shutdown()
