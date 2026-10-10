"""Builds the report document from scored runs. Pure: no file access."""

from dataclasses import dataclass
from enum import Enum

from parda_bench.eval.detection import DetectionReport
from parda_bench.eval.matching import MatchMode
from parda_bench.eval.metrics import Counts, ratio
from parda_bench.report.doc import Doc
from parda_bench.spec.record import (
    CONTROL_TOOL,
    DetectionSuite,
    Fail,
    NotApplicable,
    Pass,
    RestoreRecord,
    RestoreSuite,
    RunMeta,
)
from parda_bench.spec.scenario import CheckKind

DISCLAIMER = (
    "Results describe this dataset and these scenarios only. They are not a verdict on overall tool quality "
    "and not a compliance certification."
)
DASH = "–"


@dataclass(frozen=True)
class ScoredDetection:
    meta: RunMeta
    report: DetectionReport


@dataclass(frozen=True)
class RestoreRun:
    meta: RunMeta
    records: list[RestoreRecord]


def pct(value: float | None) -> str:
    return DASH if value is None else f"{value * 100:.1f}"


def ms(ns: int) -> str:
    return f"{ns / 1e6:.2f}"


def tool_label(m: RunMeta) -> str:
    return m.tool.name if m.tool.config == "default" else f"{m.tool.name} ({m.tool.config})"


def driver_of(m: RunMeta) -> str:
    return m.suite.driver if isinstance(m.suite, RestoreSuite) else ""


def name(value: Enum) -> str:
    return str(value.value)


def _hardware(m: RunMeta) -> str:
    h = m.hardware
    return f"{h.os} {h.arch}, {h.cpu}, {h.cores} cores, {h.memory_mb} MB"


def build(detections: list[ScoredDetection], restores: list[RestoreRun]) -> Doc:
    doc = Doc("Parda Bench report")
    doc.paragraph(DISCLAIMER)
    if detections:
        _detection_section(doc, detections)
    tools = [r for r in restores if r.meta.tool.name != CONTROL_TOOL]
    control = [r for r in restores if r.meta.tool.name == CONTROL_TOOL]
    if tools:
        _restore_section(doc, tools)
    if control:
        _control_section(doc, control)
    if not detections and not restores:
        doc.paragraph("No runs found.")
    doc.heading(2, "Runs")
    metas = sorted([d.meta for d in detections] + [r.meta for r in restores], key=lambda m: m.run_id)
    doc.table(
        ["Run", "Started", "Tool", "Version", "Commit", "Hardware"],
        [
            [
                m.run_id,
                m.started_at,
                f"{m.tool.name} ({m.tool.config})",
                m.tool.version,
                m.tool.commit,
                _hardware(m),
            ]
            for m in metas
        ],
    )
    return doc


def _detection_section(doc: Doc, runs: list[ScoredDetection]) -> None:
    doc.heading(2, "Detection")
    doc.paragraph(
        "F1 is micro-averaged over all entity types. Strict matching needs identical boundaries and type; relaxed "
        "matching needs any overlap and the same type. Hard-negative rate: share of decoy texts where the tool "
        "flagged the decoy's entity type. Latency is per sample: tool time is measured inside the tool, wall time "
        "by the harness including IPC."
    )
    doc.table(
        [
            "Tool",
            "Dataset",
            "Samples",
            "Strict F1",
            "Relaxed F1",
            "Relaxed P",
            "Relaxed R",
            "Hard-neg rate",
            "Tool p50 ms",
            "Tool p95 ms",
            "Wall p50 ms",
            "Failed",
        ],
        [_summary_row(r) for r in runs],
    )
    for run in runs:
        _detection_detail(doc, run)


def _summary_row(run: ScoredDetection) -> list[str]:
    r = run.report
    strict, relaxed = r.scores[MatchMode.STRICT].micro, r.scores[MatchMode.RELAXED].micro
    flagged = sum(c.flagged_as_decoy_type for c in r.hard_negatives.values())
    negatives = sum(c.samples for c in r.hard_negatives.values())
    samples = str(r.evaluated_samples)
    if r.missing_samples:
        samples += f" of {r.evaluated_samples + r.missing_samples}"
    lat = r.latency
    dataset = run.meta.suite.dataset_version if isinstance(run.meta.suite, DetectionSuite) else ""
    return [
        tool_label(run.meta),
        dataset,
        samples,
        pct(strict.f1()),
        pct(relaxed.f1()),
        pct(relaxed.precision()),
        pct(relaxed.recall()),
        pct(ratio(flagged, negatives)),
        ms(lat.tool_ns.p50) if lat else DASH,
        ms(lat.tool_ns.p95) if lat else DASH,
        ms(lat.wall_ns.p50) if lat else DASH,
        str(r.failed_samples),
    ]


def _detection_detail(doc: Doc, run: ScoredDetection) -> None:
    r = run.report
    doc.heading(3, tool_label(run.meta))
    strict, relaxed = r.scores[MatchMode.STRICT].by_entity, r.scores[MatchMode.RELAXED].by_entity
    rows = []
    for entity in sorted(set(strict) | set(relaxed)):
        s, x = strict.get(entity, Counts()), relaxed.get(entity, Counts())
        rows.append(
            [
                name(entity),
                str(x.tp + x.fn),
                pct(s.precision()),
                pct(s.recall()),
                pct(s.f1()),
                pct(x.precision()),
                pct(x.recall()),
                pct(x.f1()),
            ]
        )
    doc.table(["Entity", "Gold", "Strict P", "Strict R", "Strict F1", "Relaxed P", "Relaxed R", "Relaxed F1"], rows)
    for dimension, slices in (
        ("Language", r.by_lang),
        ("Difficulty", r.by_difficulty),
        ("Source", r.by_source),
    ):
        doc.table(
            [dimension, "Strict F1", "Relaxed F1"],
            [[name(k), pct(v[MatchMode.STRICT].f1()), pct(v[MatchMode.RELAXED].f1())] for k, v in slices.items()],
        )
    if r.hard_negatives:
        doc.table(
            ["Decoy type", "Texts", "Flagged as that type", "Flagged anything"],
            [
                [name(e), str(c.samples), str(c.flagged_as_decoy_type), str(c.flagged_any)]
                for e, c in sorted(r.hard_negatives.items())
            ],
        )
    if r.unmapped:
        doc.paragraph(
            "Labels the tool emitted that its `entity_map` does not translate; they count as neither hits nor "
            "false alarms."
        )
        doc.table(
            ["Tool label", "Predictions", "Overlapping a gold span"],
            [[label, str(c.predictions), str(c.overlapping_gold)] for label, c in sorted(r.unmapped.items())],
        )


def _outcome_label(record: RestoreRecord | None) -> str:
    if record is None:
        return DASH
    return {Pass: "pass", Fail: "FAIL", NotApplicable: "n/a"}[type(record.outcome)]


def _restore_section(doc: Doc, runs: list[RestoreRun]) -> None:
    doc.heading(2, "Restore")
    doc.paragraph(
        "Each scenario sends real values through the tool to a scripted mock LLM and checks what reached the "
        "upstream and what the application got back. Proxies are exercised through each client driver; library "
        "tools are called directly by the harness (driver `library`)."
    )

    def count(run: RestoreRun, kind: type) -> str:
        return str(sum(isinstance(r.outcome, kind) for r in run.records))

    doc.table(
        ["Tool", "Driver", "Pass", "Fail", "Not applicable"],
        [
            [tool_label(r.meta), driver_of(r.meta), count(r, Pass), count(r, Fail), count(r, NotApplicable)]
            for r in runs
        ],
    )
    scenario_ids = sorted({x.scenario_id for r in runs for x in r.records})
    lookup = [{x.scenario_id: x for x in r.records} for r in runs]
    doc.table(
        ["Scenario", *(f"{tool_label(r.meta)} / {driver_of(r.meta)}" for r in runs)],
        [[sid, *(_outcome_label(records.get(sid)) for records in lookup)] for sid in scenario_ids],
    )
    doc.heading(3, "Failures")
    failures = [
        f"{tool_label(r.meta)} / {driver_of(r.meta)}: {record.scenario_id}: {f.check.value}: {f.detail}"
        for r in runs
        for record in r.records
        if isinstance(record.outcome, Fail)
        for f in record.outcome.failures
    ]
    if failures:
        doc.bullets(failures)
    else:
        doc.paragraph("None.")


def _control_section(doc: Doc, runs: list[RestoreRun]) -> None:
    doc.heading(2, "Driver checks")
    doc.paragraph(
        "Control runs send every scenario straight to the mock upstream with no tool in between. Each "
        "no_leak_upstream check fails by design; any other failure comes from the driver or its client library, "
        "and would show up the same way for every tool tested through that driver."
    )
    rows, details = [], []
    for r in runs:
        other = [
            f"{driver_of(r.meta)}: {record.scenario_id}: {f.check.value}: {f.detail}"
            for record in r.records
            if isinstance(record.outcome, Fail)
            for f in record.outcome.failures
            if f.check is not CheckKind.NO_LEAK_UPSTREAM
        ]
        ran = sum(not isinstance(x.outcome, NotApplicable) for x in r.records)
        rows.append([driver_of(r.meta), str(ran), str(len(other))])
        details += other
    doc.table(["Driver", "Scenarios run", "Failures other than leaks"], rows)
    if details:
        doc.bullets(details)
