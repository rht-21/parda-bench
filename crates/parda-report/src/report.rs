//! Builds the report document from scored runs. Pure: no file access.

use std::collections::{BTreeMap, BTreeSet};

use parda_eval::detection::DetectionReport;
use parda_eval::matching::MatchMode;
use parda_eval::metrics::Counts;
use parda_spec::record::{CONTROL_TOOL, RestoreOutcome, RestoreRecord, RunMeta, Suite};
use parda_spec::scenario::CheckKind;

use crate::doc::Doc;

const DISCLAIMER: &str = "Results describe this dataset and these scenarios only. They are not a verdict on \
overall tool quality and not a compliance certification.";

/// A detection run with its scores.
pub struct ScoredDetection<'a> {
    pub meta: &'a RunMeta,
    pub report: DetectionReport,
}

/// A restore run with its per-scenario outcomes.
pub struct RestoreRun<'a> {
    pub meta: &'a RunMeta,
    pub records: &'a [RestoreRecord],
}

#[must_use]
pub fn build(detections: &[ScoredDetection<'_>], restores: &[RestoreRun<'_>]) -> Doc {
    let mut doc = Doc {
        title: "Parda Bench report".to_owned(),
        blocks: Vec::new(),
    };
    doc.paragraph(DISCLAIMER);
    if !detections.is_empty() {
        detection_section(&mut doc, detections);
    }
    let (control, tools): (Vec<&RestoreRun<'_>>, Vec<&RestoreRun<'_>>) = restores
        .iter()
        .partition(|r| r.meta.tool.name == CONTROL_TOOL);
    if !tools.is_empty() {
        restore_section(&mut doc, &tools);
    }
    if !control.is_empty() {
        control_section(&mut doc, &control);
    }
    if detections.is_empty() && restores.is_empty() {
        doc.paragraph("No runs found.");
    }
    doc.heading(2, "Runs");
    let mut runs: Vec<&RunMeta> = detections
        .iter()
        .map(|d| d.meta)
        .chain(restores.iter().map(|r| r.meta))
        .collect();
    runs.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    doc.table(
        &["Run", "Started", "Tool", "Version", "Commit", "Hardware"],
        runs.iter()
            .map(|m| {
                vec![
                    m.run_id.clone(),
                    m.started_at.clone(),
                    format!("{} ({})", m.tool.name, m.tool.config),
                    m.tool.version.clone(),
                    m.tool.commit.clone(),
                    format!(
                        "{} {}, {}, {} cores, {} MB",
                        m.hardware.os,
                        m.hardware.arch,
                        m.hardware.cpu,
                        m.hardware.cores,
                        m.hardware.memory_mb
                    ),
                ]
            })
            .collect(),
    );
    doc
}

fn detection_section(doc: &mut Doc, runs: &[ScoredDetection<'_>]) {
    doc.heading(2, "Detection");
    doc.paragraph(
        "F1 is micro-averaged over all entity types. Strict matching needs identical boundaries and type; relaxed \
         matching needs any overlap and the same type. Hard-negative rate: share of decoy texts where the tool flagged \
         the decoy's entity type. Latency is per sample: tool time is measured inside the tool, wall time by the \
         harness including IPC.",
    );
    doc.table(
        &[
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
        runs.iter().map(summary_row).collect(),
    );
    for run in runs {
        detection_detail(doc, run);
    }
}

fn summary_row(run: &ScoredDetection<'_>) -> Vec<String> {
    let r = &run.report;
    let strict = micro(r, MatchMode::Strict);
    let relaxed = micro(r, MatchMode::Relaxed);
    let (flagged, negatives) = r.hard_negatives.values().fold((0, 0), |(f, n), c| {
        (f + c.flagged_as_decoy_type, n + c.samples)
    });
    let dataset = match &run.meta.suite {
        Suite::Detection {
            dataset_version, ..
        } => dataset_version.clone(),
        Suite::Restore { .. } => String::new(),
    };
    vec![
        tool_label(run.meta),
        dataset,
        format!(
            "{}{}",
            r.evaluated_samples,
            if r.missing_samples > 0 {
                format!(" of {}", r.evaluated_samples + r.missing_samples)
            } else {
                String::new()
            }
        ),
        pct(strict.f1()),
        pct(relaxed.f1()),
        pct(relaxed.precision()),
        pct(relaxed.recall()),
        pct(ratio(flagged, negatives)),
        r.latency.as_ref().map_or_else(dash, |l| ms(l.tool_ns.p50)),
        r.latency.as_ref().map_or_else(dash, |l| ms(l.tool_ns.p95)),
        r.latency.as_ref().map_or_else(dash, |l| ms(l.wall_ns.p50)),
        r.failed_samples.to_string(),
    ]
}

fn detection_detail(doc: &mut Doc, run: &ScoredDetection<'_>) {
    let r = &run.report;
    doc.heading(3, tool_label(run.meta));
    entity_table(doc, r);
    slice_table(
        doc,
        "Language",
        r.by_lang.iter().map(|(k, v)| (enum_name(k), v)),
    );
    slice_table(
        doc,
        "Difficulty",
        r.by_difficulty.iter().map(|(k, v)| (enum_name(k), v)),
    );
    slice_table(
        doc,
        "Source",
        r.by_source.iter().map(|(k, v)| (enum_name(k), v)),
    );
    if !r.hard_negatives.is_empty() {
        let rows = r
            .hard_negatives
            .iter()
            .map(|(e, c)| {
                vec![
                    enum_name(e),
                    c.samples.to_string(),
                    c.flagged_as_decoy_type.to_string(),
                    c.flagged_any.to_string(),
                ]
            })
            .collect();
        doc.table(
            &[
                "Decoy type",
                "Texts",
                "Flagged as that type",
                "Flagged anything",
            ],
            rows,
        );
    }
    if !r.unmapped.is_empty() {
        doc.paragraph(
            "Labels the tool emitted that its `entity_map` does not translate; they count as neither hits nor \
             false alarms.",
        );
        let rows = r
            .unmapped
            .iter()
            .map(|(l, c)| {
                vec![
                    l.clone(),
                    c.predictions.to_string(),
                    c.overlapping_gold.to_string(),
                ]
            })
            .collect();
        doc.table(
            &["Tool label", "Predictions", "Overlapping a gold span"],
            rows,
        );
    }
}

fn entity_table(doc: &mut Doc, r: &DetectionReport) {
    let by_entity = |mode| {
        r.scores
            .get(&mode)
            .map(|s| s.by_entity.clone())
            .unwrap_or_default()
    };
    let (strict, relaxed) = (by_entity(MatchMode::Strict), by_entity(MatchMode::Relaxed));
    let entities: BTreeSet<_> = strict.keys().chain(relaxed.keys()).collect();
    let rows = entities
        .into_iter()
        .map(|e| {
            let s = strict.get(e).copied().unwrap_or_default();
            let x = relaxed.get(e).copied().unwrap_or_default();
            vec![
                enum_name(e),
                (x.tp + x.fn_).to_string(),
                pct(s.precision()),
                pct(s.recall()),
                pct(s.f1()),
                pct(x.precision()),
                pct(x.recall()),
                pct(x.f1()),
            ]
        })
        .collect();
    doc.table(
        &[
            "Entity",
            "Gold",
            "Strict P",
            "Strict R",
            "Strict F1",
            "Relaxed P",
            "Relaxed R",
            "Relaxed F1",
        ],
        rows,
    );
}

/// Micro F1 per value of one slicing dimension (language, difficulty).
fn slice_table<'a>(
    doc: &mut Doc,
    dimension: &str,
    slices: impl Iterator<Item = (String, &'a BTreeMap<MatchMode, Counts>)>,
) {
    let rows = slices
        .map(|(name, by_mode)| {
            let f1 = |m| pct(by_mode.get(&m).copied().unwrap_or_default().f1());
            vec![name, f1(MatchMode::Strict), f1(MatchMode::Relaxed)]
        })
        .collect();
    doc.table(&[dimension, "Strict F1", "Relaxed F1"], rows);
}

fn restore_section(doc: &mut Doc, runs: &[&RestoreRun<'_>]) {
    doc.heading(2, "Restore");
    doc.paragraph(
        "Each scenario sends real values through the tool to a scripted mock LLM and checks what reached the \
         upstream and what the application got back. Proxies are exercised through each client driver; library \
         tools are called directly by the harness (driver `library`).",
    );
    doc.table(
        &["Tool", "Driver", "Pass", "Fail", "Not applicable"],
        runs.iter()
            .map(|r| {
                let count = |f: fn(&RestoreOutcome) -> bool| {
                    r.records
                        .iter()
                        .filter(|x| f(&x.outcome))
                        .count()
                        .to_string()
                };
                vec![
                    tool_label(r.meta),
                    driver_of(r.meta),
                    count(|o| matches!(o, RestoreOutcome::Pass)),
                    count(|o| matches!(o, RestoreOutcome::Fail { .. })),
                    count(|o| matches!(o, RestoreOutcome::NotApplicable { .. })),
                ]
            })
            .collect(),
    );
    let columns: Vec<String> = runs
        .iter()
        .map(|r| format!("{} / {}", tool_label(r.meta), driver_of(r.meta)))
        .collect();
    let scenarios: BTreeSet<&str> = runs
        .iter()
        .flat_map(|r| r.records.iter().map(|x| x.scenario_id.as_str()))
        .collect();
    let mut headers = vec!["Scenario"];
    headers.extend(columns.iter().map(String::as_str));
    doc.table(
        &headers,
        scenarios
            .iter()
            .map(|id| {
                std::iter::once((*id).to_owned())
                    .chain(runs.iter().map(|r| {
                        r.records
                            .iter()
                            .find(|x| x.scenario_id == *id)
                            .map_or_else(dash, |x| match &x.outcome {
                                RestoreOutcome::Pass => "pass".to_owned(),
                                RestoreOutcome::Fail { .. } => "FAIL".to_owned(),
                                RestoreOutcome::NotApplicable { .. } => "n/a".to_owned(),
                            })
                    }))
                    .collect()
            })
            .collect(),
    );
    doc.heading(3, "Failures");
    let mut failures = Vec::new();
    for r in runs {
        for record in r.records {
            if let RestoreOutcome::Fail { failures: list, .. } = &record.outcome {
                for f in list {
                    failures.push(format!(
                        "{} / {}: {}: {}: {}",
                        tool_label(r.meta),
                        driver_of(r.meta),
                        record.scenario_id,
                        enum_name(&f.check),
                        f.detail
                    ));
                }
            }
        }
    }
    if failures.is_empty() {
        doc.paragraph("None.");
    } else {
        doc.list(failures);
    }
}

fn control_section(doc: &mut Doc, runs: &[&RestoreRun<'_>]) {
    doc.heading(2, "Driver checks");
    doc.paragraph(
        "Control runs send every scenario straight to the mock upstream with no tool in between. Each \
         no_leak_upstream check fails by design; any other failure comes from the driver or its client library, \
         and would show up the same way for every tool tested through that driver.",
    );
    let mut details = Vec::new();
    let rows = runs
        .iter()
        .map(|r| {
            let mut other = 0;
            for record in r.records {
                if let RestoreOutcome::Fail { failures, .. } = &record.outcome {
                    for f in failures
                        .iter()
                        .filter(|f| f.check != CheckKind::NoLeakUpstream)
                    {
                        other += 1;
                        details.push(format!(
                            "{}: {}: {}: {}",
                            driver_of(r.meta),
                            record.scenario_id,
                            enum_name(&f.check),
                            f.detail
                        ));
                    }
                }
            }
            let ran = r
                .records
                .iter()
                .filter(|x| !matches!(x.outcome, RestoreOutcome::NotApplicable { .. }))
                .count();
            vec![driver_of(r.meta), ran.to_string(), other.to_string()]
        })
        .collect();
    doc.table(
        &["Driver", "Scenarios run", "Failures other than leaks"],
        rows,
    );
    if !details.is_empty() {
        doc.list(details);
    }
}

fn micro(r: &DetectionReport, mode: MatchMode) -> Counts {
    r.scores.get(&mode).map(|s| s.micro).unwrap_or_default()
}

fn tool_label(m: &RunMeta) -> String {
    if m.tool.config == "default" {
        m.tool.name.clone()
    } else {
        format!("{} ({})", m.tool.name, m.tool.config)
    }
}

fn driver_of(m: &RunMeta) -> String {
    match &m.suite {
        Suite::Restore { driver } => driver.clone(),
        Suite::Detection { .. } => String::new(),
    }
}

/// The serialized (wire) name of an enum value, e.g. `PAYMENT_CARD` or `hi-Deva`.
fn enum_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|j| j.as_str().map(str::to_owned))
        .unwrap_or_else(|| "?".to_owned())
}

#[allow(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
fn ratio(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}

fn pct(v: Option<f64>) -> String {
    v.map_or_else(dash, |x| format!("{:.1}", x * 100.0))
}

#[allow(
    clippy::cast_precision_loss,
    reason = "nanosecond latencies stay far below 2^52"
)]
fn ms(ns: u64) -> String {
    format!("{:.2}", ns as f64 / 1e6)
}

fn dash() -> String {
    "–".to_owned()
}

#[cfg(test)]
mod tests {
    use parda_spec::record::{CheckFailure, Hardware, ToolRef};
    use parda_spec::scenario::CheckKind;

    use super::*;

    fn meta(run_id: &str, suite: Suite) -> RunMeta {
        RunMeta {
            run_id: run_id.to_owned(),
            started_at: "2026-10-01T00:00:00Z".to_owned(),
            harness_version: "0.1.0".to_owned(),
            tool: ToolRef {
                name: "t".to_owned(),
                version: "1".to_owned(),
                commit: "c".to_owned(),
                config: "default".to_owned(),
            },
            hardware: Hardware {
                os: "linux".to_owned(),
                arch: "x86_64".to_owned(),
                cpu: "cpu".to_owned(),
                cores: 4,
                memory_mb: 1024,
            },
            suite,
        }
    }

    #[test]
    fn restore_matrix_marks_each_outcome() {
        let m = meta(
            "r1",
            Suite::Restore {
                driver: "raw_http".to_owned(),
            },
        );
        let records = vec![
            RestoreRecord {
                scenario_id: "a".to_owned(),
                outcome: RestoreOutcome::Pass,
            },
            RestoreRecord {
                scenario_id: "b".to_owned(),
                outcome: RestoreOutcome::Fail {
                    failures: vec![CheckFailure {
                        check: CheckKind::NoLeakUpstream,
                        detail: "leaked".to_owned(),
                    }],
                    upstream_excerpt: String::new(),
                    client_excerpt: String::new(),
                },
            },
        ];
        let md = build(
            &[],
            &[RestoreRun {
                meta: &m,
                records: &records,
            }],
        )
        .to_markdown();
        assert!(md.contains("| a | pass |"), "{md}");
        assert!(md.contains("| b | FAIL |"), "{md}");
        assert!(md.contains("no_leak_upstream: leaked"), "{md}");
        assert!(md.contains("| t | raw_http | 1 | 1 | 0 |"), "{md}");
    }

    #[test]
    fn control_runs_are_listed_as_driver_checks_only() {
        let mut m = meta(
            "c1",
            Suite::Restore {
                driver: "litellm".to_owned(),
            },
        );
        m.tool.name = CONTROL_TOOL.to_owned();
        let records = vec![RestoreRecord {
            scenario_id: "fail-529".to_owned(),
            outcome: RestoreOutcome::Fail {
                failures: vec![
                    CheckFailure {
                        check: CheckKind::NoLeakUpstream,
                        detail: "expected".to_owned(),
                    },
                    CheckFailure {
                        check: CheckKind::FailsClosed,
                        detail: "saw 500".to_owned(),
                    },
                ],
                upstream_excerpt: String::new(),
                client_excerpt: String::new(),
            },
        }];
        let md = build(
            &[],
            &[RestoreRun {
                meta: &m,
                records: &records,
            }],
        )
        .to_markdown();
        assert!(!md.contains("## Restore"), "{md}");
        assert!(md.contains("| litellm | 1 | 1 |"), "{md}");
        assert!(
            md.contains("litellm: fail-529: fails_closed: saw 500"),
            "{md}"
        );
    }

    #[test]
    fn formats_percentages_and_missing_values() {
        assert_eq!(pct(Some(0.8766)), "87.7");
        assert_eq!(pct(None), "–");
        assert_eq!(ms(1_234_567), "1.23");
    }
}
