use serde::Serialize;

/// Nearest-rank percentiles of a set of durations, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Percentiles {
    pub count: usize,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
    pub mean: u64,
}

impl Percentiles {
    /// `None` for an empty set.
    #[must_use]
    pub fn of(values: &[u64]) -> Option<Self> {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let max = *sorted.last()?;
        let total: u128 = sorted.iter().map(|&v| u128::from(v)).sum();
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a mean never exceeds the u64 maximum value"
        )]
        let mean = (total / sorted.len() as u128) as u64;
        Some(Self {
            count: sorted.len(),
            p50: nearest_rank(&sorted, 50),
            p95: nearest_rank(&sorted, 95),
            p99: nearest_rank(&sorted, 99),
            max,
            mean,
        })
    }
}

/// Smallest value with at least `pct` percent of the (non-empty, sorted) values at or below it.
fn nearest_rank(sorted: &[u64], pct: usize) -> u64 {
    let rank = (pct * sorted.len()).div_ceil(100).max(1);
    sorted[rank - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_on_one_to_hundred() {
        let values: Vec<u64> = (1..=100).rev().collect();
        let p = Percentiles::of(&values).unwrap();
        assert_eq!((p.p50, p.p95, p.p99, p.max, p.mean), (50, 95, 99, 100, 50));
    }

    #[test]
    fn single_value_is_every_percentile() {
        let p = Percentiles::of(&[7]).unwrap();
        assert_eq!((p.p50, p.p99, p.max), (7, 7, 7));
    }

    #[test]
    fn empty_input_has_no_percentiles() {
        assert_eq!(Percentiles::of(&[]), None);
    }
}
