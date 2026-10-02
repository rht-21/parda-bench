//! Scores one detection run (`detections.jsonl`) against the dataset it ran on.

use std::collections::{BTreeMap, HashMap, HashSet};

use parda_spec::entity::EntityType;
use parda_spec::record::{DetectionRecord, DetectionResult, PredictedEntity, PredictedSpan};
use parda_spec::sample::{Difficulty, Labels, Lang, Sample, Source, Span};
use serde::Serialize;

use crate::latency::Percentiles;
use crate::matching::{MatchMode, match_spans};
use crate::metrics::Counts;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EvalError {
    #[error("record for sample {0} appears more than once")]
    DuplicateRecord(String),
    #[error(
        "record for sample {0}, which is not in the dataset: was the run made on a different dataset?"
    )]
    UnknownSample(String),
}

/// Micro-averaged and per-entity counts for one match mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Scores {
    pub micro: Counts,
    pub by_entity: BTreeMap<EntityType, Counts>,
}

impl Scores {
    fn add(&mut self, per_entity: &BTreeMap<EntityType, Counts>) {
        for (entity, counts) in per_entity {
            self.by_entity.entry(*entity).or_default().add(*counts);
            self.micro.add(*counts);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct NegativeCounts {
    pub samples: usize,
    /// Samples where the tool flagged the decoy's own entity type.
    pub flagged_as_decoy_type: usize,
    /// Samples where the tool flagged anything at all.
    pub flagged_any: usize,
}

/// Predictions whose tool label had no `entity_map` entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct UnmappedCounts {
    pub predictions: usize,
    /// Of those, how many overlap a gold span: a hint that `entity_map` is missing an entry.
    pub overlapping_gold: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LatencySummary {
    /// Measured inside the tool.
    pub tool_ns: Percentiles,
    /// Measured by the harness, including IPC.
    pub wall_ns: Percentiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectionReport {
    pub evaluated_samples: usize,
    /// Dataset samples with no record, e.g. a run made with `--limit`.
    pub missing_samples: usize,
    pub failed_samples: usize,
    pub scores: BTreeMap<MatchMode, Scores>,
    pub by_lang: BTreeMap<Lang, BTreeMap<MatchMode, Counts>>,
    pub by_difficulty: BTreeMap<Difficulty, BTreeMap<MatchMode, Counts>>,
    pub by_source: BTreeMap<Source, BTreeMap<MatchMode, Counts>>,
    pub hard_negatives: BTreeMap<EntityType, NegativeCounts>,
    pub unmapped: BTreeMap<String, UnmappedCounts>,
    pub latency: Option<LatencySummary>,
}

/// Scores `records` against `samples`. A failed detection counts all of its sample's gold spans as misses.
///
/// # Errors
/// Fails if a record is duplicated or names a sample that is not in `samples`.
pub fn evaluate(
    samples: &[Sample],
    records: &[DetectionRecord],
) -> Result<DetectionReport, EvalError> {
    let by_id: HashMap<&str, &Sample> = samples.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut acc = Accumulator::default();
    for record in records {
        let sample = *by_id
            .get(record.sample_id.as_str())
            .ok_or_else(|| EvalError::UnknownSample(record.sample_id.clone()))?;
        if !seen.insert(record.sample_id.as_str()) {
            return Err(EvalError::DuplicateRecord(record.sample_id.clone()));
        }
        acc.add(sample, &record.result);
    }
    Ok(acc.finish(samples.len() - seen.len()))
}

#[derive(Default)]
struct Accumulator {
    evaluated: usize,
    failed: usize,
    scores: BTreeMap<MatchMode, Scores>,
    by_lang: BTreeMap<Lang, BTreeMap<MatchMode, Counts>>,
    by_difficulty: BTreeMap<Difficulty, BTreeMap<MatchMode, Counts>>,
    by_source: BTreeMap<Source, BTreeMap<MatchMode, Counts>>,
    hard_negatives: BTreeMap<EntityType, NegativeCounts>,
    unmapped: BTreeMap<String, UnmappedCounts>,
    tool_ns: Vec<u64>,
    wall_ns: Vec<u64>,
}

impl Accumulator {
    fn add(&mut self, sample: &Sample, result: &DetectionResult) {
        self.evaluated += 1;
        let predicted: &[PredictedSpan] = match result {
            DetectionResult::Detected {
                predicted,
                elapsed_ns,
                wall_ns,
            } => {
                self.tool_ns.push(*elapsed_ns);
                self.wall_ns.push(*wall_ns);
                predicted
            }
            DetectionResult::Failed { .. } => {
                self.failed += 1;
                &[]
            }
        };
        let gold: &[Span] = match &sample.labels {
            Labels::Positive { spans } => spans,
            Labels::HardNegative { .. } => &[],
        };
        let (mapped, unmapped) = split_mapped(predicted);
        for (label, p) in unmapped {
            let entry = self.unmapped.entry(label.to_owned()).or_default();
            entry.predictions += 1;
            if gold.iter().any(|g| g.start < p.end && p.start < g.end) {
                entry.overlapping_gold += 1;
            }
        }
        if let Labels::HardNegative { decoy } = &sample.labels {
            let entry = self.hard_negatives.entry(*decoy).or_default();
            entry.samples += 1;
            entry.flagged_as_decoy_type += usize::from(mapped.iter().any(|p| p.entity == *decoy));
            entry.flagged_any += usize::from(!predicted.is_empty());
        }
        for mode in MatchMode::ALL {
            let per_entity = match_spans(gold, &mapped, mode);
            let total = per_entity.values().fold(Counts::default(), |mut a, c| {
                a.add(*c);
                a
            });
            self.scores.entry(mode).or_default().add(&per_entity);
            add_to_slice(&mut self.by_lang, sample.lang, mode, total);
            add_to_slice(&mut self.by_difficulty, sample.difficulty, mode, total);
            add_to_slice(&mut self.by_source, sample.source, mode, total);
        }
    }

    fn finish(self, missing_samples: usize) -> DetectionReport {
        let latency = Percentiles::of(&self.tool_ns)
            .zip(Percentiles::of(&self.wall_ns))
            .map(|(tool_ns, wall_ns)| LatencySummary { tool_ns, wall_ns });
        DetectionReport {
            evaluated_samples: self.evaluated,
            missing_samples,
            failed_samples: self.failed,
            scores: self.scores,
            by_lang: self.by_lang,
            by_difficulty: self.by_difficulty,
            by_source: self.by_source,
            hard_negatives: self.hard_negatives,
            unmapped: self.unmapped,
            latency,
        }
    }
}

fn add_to_slice<K: Ord>(
    slices: &mut BTreeMap<K, BTreeMap<MatchMode, Counts>>,
    key: K,
    mode: MatchMode,
    counts: Counts,
) {
    slices
        .entry(key)
        .or_default()
        .entry(mode)
        .or_default()
        .add(counts);
}

/// Mapped predictions as gold-comparable spans, and the unmapped predictions as they are.
fn split_mapped(predicted: &[PredictedSpan]) -> (Vec<Span>, Vec<(&str, &PredictedSpan)>) {
    let mut mapped = Vec::new();
    let mut unmapped = Vec::new();
    for p in predicted {
        match &p.entity {
            PredictedEntity::Mapped(entity) => mapped.push(Span {
                start: p.start,
                end: p.end,
                entity: *entity,
            }),
            PredictedEntity::Unmapped(label) => unmapped.push((label.as_str(), p)),
        }
    }
    (mapped, unmapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, labels: Labels, lang: Lang) -> Sample {
        Sample {
            id: id.to_owned(),
            text: "x".repeat(40),
            labels,
            lang,
            difficulty: Difficulty::Easy,
            source: Source::Handwritten,
            generator_version: "test".to_owned(),
        }
    }

    fn positive(spans: Vec<Span>) -> Labels {
        Labels::Positive { spans }
    }

    fn detected(id: &str, predicted: Vec<PredictedSpan>, ns: u64) -> DetectionRecord {
        DetectionRecord {
            sample_id: id.to_owned(),
            result: DetectionResult::Detected {
                predicted,
                elapsed_ns: ns,
                wall_ns: ns + 10,
            },
        }
    }

    fn pred(start: usize, end: usize, entity: PredictedEntity) -> PredictedSpan {
        PredictedSpan { start, end, entity }
    }

    const PAN: EntityType = EntityType::Pan;

    #[test]
    fn scores_a_small_run_end_to_end() {
        let samples = [
            sample(
                "a",
                positive(vec![Span {
                    start: 0,
                    end: 10,
                    entity: PAN,
                }]),
                Lang::En,
            ),
            sample(
                "b",
                Labels::HardNegative {
                    decoy: EntityType::Phone,
                },
                Lang::HiLatn,
            ),
            sample("c", positive(vec![]), Lang::En),
        ];
        let records = [
            detected("a", vec![pred(0, 10, PredictedEntity::Mapped(PAN))], 100),
            detected(
                "b",
                vec![pred(5, 15, PredictedEntity::Mapped(EntityType::Phone))],
                300,
            ),
            detected(
                "c",
                vec![pred(1, 3, PredictedEntity::Unmapped("ORG".to_owned()))],
                200,
            ),
        ];
        let r = evaluate(&samples, &records).unwrap();
        assert_eq!(r.evaluated_samples, 3);
        assert_eq!(
            r.scores[&MatchMode::Strict].by_entity[&PAN],
            Counts {
                tp: 1,
                fp: 0,
                fn_: 0
            }
        );
        assert_eq!(
            r.scores[&MatchMode::Strict].micro,
            Counts {
                tp: 1,
                fp: 1,
                fn_: 0
            }
        );
        assert_eq!(
            r.hard_negatives[&EntityType::Phone],
            NegativeCounts {
                samples: 1,
                flagged_as_decoy_type: 1,
                flagged_any: 1
            }
        );
        assert_eq!(
            r.unmapped["ORG"],
            UnmappedCounts {
                predictions: 1,
                overlapping_gold: 0
            }
        );
        assert_eq!(
            r.by_lang[&Lang::HiLatn][&MatchMode::Relaxed],
            Counts {
                tp: 0,
                fp: 1,
                fn_: 0
            }
        );
        assert_eq!(
            r.by_source[&Source::Handwritten][&MatchMode::Strict],
            Counts { tp: 1, fp: 1, fn_: 0 }
        );
        let latency = r.latency.unwrap();
        assert_eq!((latency.tool_ns.p50, latency.wall_ns.max), (200, 310));
    }

    #[test]
    fn failed_detection_counts_gold_as_missed() {
        let samples = [sample(
            "a",
            positive(vec![Span {
                start: 0,
                end: 10,
                entity: PAN,
            }]),
            Lang::En,
        )];
        let records = [DetectionRecord {
            sample_id: "a".to_owned(),
            result: DetectionResult::Failed {
                message: "boom".to_owned(),
            },
        }];
        let r = evaluate(&samples, &records).unwrap();
        assert_eq!(r.failed_samples, 1);
        assert_eq!(
            r.scores[&MatchMode::Relaxed].micro,
            Counts {
                tp: 0,
                fp: 0,
                fn_: 1
            }
        );
        assert_eq!(r.latency, None);
    }

    #[test]
    fn unmapped_label_overlapping_gold_is_reported() {
        let samples = [sample(
            "a",
            positive(vec![Span {
                start: 0,
                end: 10,
                entity: PAN,
            }]),
            Lang::En,
        )];
        let records = [detected(
            "a",
            vec![pred(2, 8, PredictedEntity::Unmapped("IN_PAN".to_owned()))],
            1,
        )];
        let r = evaluate(&samples, &records).unwrap();
        assert_eq!(r.unmapped["IN_PAN"].overlapping_gold, 1);
        assert_eq!(
            r.scores[&MatchMode::Relaxed].micro,
            Counts {
                tp: 0,
                fp: 0,
                fn_: 1
            }
        );
    }

    #[test]
    fn partial_run_reports_missing_samples() {
        let samples = [
            sample("a", positive(vec![]), Lang::En),
            sample("b", positive(vec![]), Lang::En),
        ];
        let r = evaluate(&samples, &[detected("a", vec![], 1)]).unwrap();
        assert_eq!((r.evaluated_samples, r.missing_samples), (1, 1));
    }

    #[test]
    fn rejects_records_from_another_dataset_and_duplicates() {
        let samples = [sample("a", positive(vec![]), Lang::En)];
        assert_eq!(
            evaluate(&samples, &[detected("zzz", vec![], 1)]),
            Err(EvalError::UnknownSample("zzz".to_owned()))
        );
        assert_eq!(
            evaluate(
                &samples,
                &[detected("a", vec![], 1), detected("a", vec![], 1)]
            ),
            Err(EvalError::DuplicateRecord("a".to_owned()))
        );
    }
}
