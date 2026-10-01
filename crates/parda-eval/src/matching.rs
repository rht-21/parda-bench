//! One-to-one matching of predicted spans against gold spans of the same entity type.

use std::collections::BTreeMap;

use parda_spec::entity::EntityType;
use parda_spec::sample::Span;
use serde::Serialize;

use crate::metrics::Counts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    /// Same entity type and identical boundaries.
    Strict,
    /// Same entity type and any character overlap; pairs with the largest overlap are matched first.
    Relaxed,
}

impl MatchMode {
    pub const ALL: [Self; 2] = [Self::Strict, Self::Relaxed];
}

/// Per-entity counts for one sample. Every gold and every predicted span is counted exactly once.
#[must_use]
pub fn match_spans(
    gold: &[Span],
    predicted: &[Span],
    mode: MatchMode,
) -> BTreeMap<EntityType, Counts> {
    let mut gold_used = vec![false; gold.len()];
    let mut pred_used = vec![false; predicted.len()];
    for (g, p) in candidate_pairs(gold, predicted, mode) {
        if !gold_used[g] && !pred_used[p] {
            gold_used[g] = true;
            pred_used[p] = true;
        }
    }
    let mut counts: BTreeMap<EntityType, Counts> = BTreeMap::new();
    for (span, used) in gold.iter().zip(&gold_used) {
        let c = counts.entry(span.entity).or_default();
        if *used { c.tp += 1 } else { c.fn_ += 1 }
    }
    for (span, used) in predicted.iter().zip(&pred_used) {
        if !used {
            counts.entry(span.entity).or_default().fp += 1;
        }
    }
    counts
}

/// Characters shared by two spans.
#[must_use]
pub fn overlap(a: &Span, b: &Span) -> usize {
    a.end.min(b.end).saturating_sub(a.start.max(b.start))
}

/// Matchable (gold, predicted) index pairs, best first.
fn candidate_pairs(gold: &[Span], predicted: &[Span], mode: MatchMode) -> Vec<(usize, usize)> {
    let mut pairs: Vec<(usize, usize, usize)> = Vec::new();
    for (gi, g) in gold.iter().enumerate() {
        for (pi, p) in predicted.iter().enumerate() {
            if g.entity != p.entity {
                continue;
            }
            let matches = match mode {
                MatchMode::Strict => g.start == p.start && g.end == p.end,
                MatchMode::Relaxed => overlap(g, p) > 0,
            };
            if matches {
                pairs.push((overlap(g, p), gi, pi));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    pairs.into_iter().map(|(_, g, p)| (g, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: EntityType = EntityType::PersonName;
    const PAN: EntityType = EntityType::Pan;

    fn s(start: usize, end: usize, entity: EntityType) -> Span {
        Span { start, end, entity }
    }

    fn c(tp: usize, fp: usize, fn_: usize) -> Counts {
        Counts { tp, fp, fn_ }
    }

    #[test]
    fn exact_match_counts_in_both_modes() {
        for mode in MatchMode::ALL {
            let r = match_spans(&[s(0, 5, PAN)], &[s(0, 5, PAN)], mode);
            assert_eq!(r[&PAN], c(1, 0, 0));
        }
    }

    #[test]
    fn partial_overlap_matches_only_relaxed() {
        let gold = [s(0, 12, NAME)];
        let pred = [s(0, 5, NAME)];
        assert_eq!(
            match_spans(&gold, &pred, MatchMode::Strict)[&NAME],
            c(0, 1, 1)
        );
        assert_eq!(
            match_spans(&gold, &pred, MatchMode::Relaxed)[&NAME],
            c(1, 0, 0)
        );
    }

    #[test]
    fn wrong_type_is_a_miss_and_a_false_alarm() {
        let r = match_spans(&[s(0, 10, PAN)], &[s(0, 10, NAME)], MatchMode::Relaxed);
        assert_eq!(r[&PAN], c(0, 0, 1));
        assert_eq!(r[&NAME], c(0, 1, 0));
    }

    #[test]
    fn one_prediction_covering_two_golds_matches_only_one() {
        let gold = [s(0, 5, NAME), s(6, 12, NAME)];
        let r = match_spans(&gold, &[s(0, 12, NAME)], MatchMode::Relaxed);
        assert_eq!(r[&NAME], c(1, 0, 1));
    }

    #[test]
    fn duplicate_predictions_count_one_false_positive() {
        let r = match_spans(
            &[s(0, 5, PAN)],
            &[s(0, 5, PAN), s(0, 5, PAN)],
            MatchMode::Strict,
        );
        assert_eq!(r[&PAN], c(1, 1, 0));
    }

    #[test]
    fn largest_overlap_wins_the_match() {
        let gold = [s(0, 10, NAME), s(10, 12, NAME)];
        let pred = [s(8, 12, NAME)];
        let r = match_spans(&gold, &pred, MatchMode::Relaxed);
        assert_eq!(r[&NAME], c(1, 0, 1));
        let r2 = match_spans(&gold, &[s(8, 12, NAME), s(0, 4, NAME)], MatchMode::Relaxed);
        assert_eq!(r2[&NAME], c(2, 0, 0));
    }
}
