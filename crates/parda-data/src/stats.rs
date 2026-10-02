//! Dataset composition counts.

use std::collections::BTreeMap;

use parda_spec::entity::EntityType;
use parda_spec::sample::{Difficulty, Labels, Lang, Sample, Source};
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Stats {
    pub samples: usize,
    pub positive_samples: usize,
    /// Positive samples with no entities at all (ordinary text).
    pub empty_samples: usize,
    pub hard_negatives: usize,
    pub spans_by_entity: BTreeMap<EntityType, usize>,
    pub hard_negatives_by_decoy: BTreeMap<EntityType, usize>,
    pub by_lang: BTreeMap<Lang, usize>,
    pub by_difficulty: BTreeMap<Difficulty, usize>,
    pub by_source: BTreeMap<Source, usize>,
}

impl Stats {
    #[must_use]
    pub fn of(samples: &[Sample]) -> Self {
        let mut stats = Self {
            samples: samples.len(),
            ..Self::default()
        };
        for sample in samples {
            *stats.by_lang.entry(sample.lang).or_default() += 1;
            *stats.by_difficulty.entry(sample.difficulty).or_default() += 1;
            *stats.by_source.entry(sample.source).or_default() += 1;
            match &sample.labels {
                Labels::Positive { spans } => {
                    stats.positive_samples += 1;
                    if spans.is_empty() {
                        stats.empty_samples += 1;
                    }
                    for span in spans {
                        *stats.spans_by_entity.entry(span.entity).or_default() += 1;
                    }
                }
                Labels::HardNegative { decoy } => {
                    stats.hard_negatives += 1;
                    *stats.hard_negatives_by_decoy.entry(*decoy).or_default() += 1;
                }
            }
        }
        stats
    }
}
