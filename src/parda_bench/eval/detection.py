"""Scores one detection run (`detections.jsonl`) against the dataset it ran on."""

from collections import defaultdict
from dataclasses import dataclass, field
from enum import Enum

from parda_bench.eval.latency import Percentiles
from parda_bench.eval.matching import MatchMode, match_spans
from parda_bench.eval.metrics import Counts
from parda_bench.spec.entity import EntityType
from parda_bench.spec.record import Detected, DetectionRecord, Mapped, Unmapped
from parda_bench.spec.sample import HardNegative, Positive, Sample, Span


class EvalError(ValueError):
    pass


@dataclass
class Scores:
    """Micro-averaged and per-entity counts for one match mode."""

    micro: Counts = field(default_factory=Counts)
    by_entity: dict[EntityType, Counts] = field(default_factory=lambda: defaultdict(Counts))


@dataclass
class NegativeCounts:
    samples: int = 0
    flagged_as_decoy_type: int = 0
    """Samples where the tool flagged the decoy's own entity type."""
    flagged_any: int = 0
    """Samples where the tool flagged anything at all."""


@dataclass
class UnmappedCounts:
    """Predictions whose tool label had no `entity_map` entry."""

    predictions: int = 0
    overlapping_gold: int = 0
    """Of those, how many overlap a gold span: a hint that `entity_map` is missing an entry."""


@dataclass(frozen=True)
class LatencySummary:
    tool_ns: Percentiles
    """Measured inside the tool."""
    wall_ns: Percentiles
    """Measured by the harness, including IPC."""


def _slices() -> defaultdict[Enum, dict[MatchMode, Counts]]:
    return defaultdict(lambda: {m: Counts() for m in MatchMode})


@dataclass
class DetectionReport:
    evaluated_samples: int = 0
    missing_samples: int = 0
    """Dataset samples with no record, e.g. a run made with `--limit`."""
    failed_samples: int = 0
    scores: dict[MatchMode, Scores] = field(default_factory=lambda: {m: Scores() for m in MatchMode})
    by_lang: defaultdict[Enum, dict[MatchMode, Counts]] = field(default_factory=_slices)
    by_difficulty: defaultdict[Enum, dict[MatchMode, Counts]] = field(default_factory=_slices)
    by_source: defaultdict[Enum, dict[MatchMode, Counts]] = field(default_factory=_slices)
    hard_negatives: dict[EntityType, NegativeCounts] = field(default_factory=lambda: defaultdict(NegativeCounts))
    unmapped: dict[str, UnmappedCounts] = field(default_factory=lambda: defaultdict(UnmappedCounts))
    latency: LatencySummary | None = None


def evaluate(samples: list[Sample], records: list[DetectionRecord]) -> DetectionReport:
    """Scores `records` against `samples`. A failed detection counts all of its sample's gold spans as misses.

    Raises `EvalError` if a record is duplicated or names a sample that is not in `samples`.
    """
    by_id = {s.id: s for s in samples}
    seen: set[str] = set()
    report = DetectionReport()
    tool_ns: list[int] = []
    wall_ns: list[int] = []
    for record in records:
        sample = by_id.get(record.sample_id)
        if sample is None:
            raise EvalError(
                f"record for sample {record.sample_id}, which is not in the dataset: was the run made on a "
                "different dataset?"
            )
        if record.sample_id in seen:
            raise EvalError(f"record for sample {record.sample_id} appears more than once")
        seen.add(record.sample_id)
        if isinstance(record.result, Detected):
            tool_ns.append(record.result.elapsed_ns)
            wall_ns.append(record.result.wall_ns)
        else:
            report.failed_samples += 1
        _add(report, sample, record)
    report.evaluated_samples = len(seen)
    report.missing_samples = len(samples) - len(seen)
    tool, wall = Percentiles.of(tool_ns), Percentiles.of(wall_ns)
    report.latency = LatencySummary(tool, wall) if tool and wall else None
    return report


def _add(report: DetectionReport, sample: Sample, record: DetectionRecord) -> None:
    predicted = record.result.predicted if isinstance(record.result, Detected) else []
    gold = sample.labels.spans if isinstance(sample.labels, Positive) else []
    mapped = [Span(start=p.start, end=p.end, entity=p.entity.mapped) for p in predicted if isinstance(p.entity, Mapped)]
    for p in predicted:
        if isinstance(p.entity, Unmapped):
            entry = report.unmapped[p.entity.unmapped]
            entry.predictions += 1
            entry.overlapping_gold += any(g.start < p.end and p.start < g.end for g in gold)
    if isinstance(sample.labels, HardNegative):
        negative = report.hard_negatives[sample.labels.decoy]
        negative.samples += 1
        negative.flagged_as_decoy_type += any(m.entity == sample.labels.decoy for m in mapped)
        negative.flagged_any += bool(predicted)
    for mode in MatchMode:
        total = Counts()
        for entity, counts in match_spans(gold, mapped, mode).items():
            report.scores[mode].by_entity[entity].add(counts)
            report.scores[mode].micro.add(counts)
            total.add(counts)
        for slices, key in (
            (report.by_lang, sample.lang),
            (report.by_difficulty, sample.difficulty),
            (report.by_source, sample.source),
        ):
            slices[key][mode].add(total)
