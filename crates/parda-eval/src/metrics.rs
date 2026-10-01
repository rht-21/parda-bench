use serde::Serialize;

/// True positives, false positives and false negatives for one bucket.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub tp: usize,
    pub fp: usize,
    #[serde(rename = "fn")]
    pub fn_: usize,
}

impl Counts {
    /// `None` when nothing was predicted.
    #[must_use]
    pub fn precision(self) -> Option<f64> {
        ratio(self.tp, self.tp + self.fp)
    }

    /// `None` when there was nothing to find.
    #[must_use]
    pub fn recall(self) -> Option<f64> {
        ratio(self.tp, self.tp + self.fn_)
    }

    /// `None` when there was nothing to find and nothing was predicted.
    #[must_use]
    pub fn f1(self) -> Option<f64> {
        ratio(2 * self.tp, 2 * self.tp + self.fp + self.fn_)
    }

    pub fn add(&mut self, other: Self) {
        self.tp += other.tp;
        self.fp += other.fp;
        self.fn_ += other.fn_;
    }
}

#[allow(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_precision_recall_f1() {
        let c = Counts {
            tp: 6,
            fp: 2,
            fn_: 4,
        };
        assert_eq!(c.precision(), Some(0.75));
        assert_eq!(c.recall(), Some(0.6));
        assert_eq!(c.f1(), Some(12.0 / 18.0));
    }

    #[test]
    fn empty_bucket_has_no_scores() {
        let c = Counts::default();
        assert_eq!((c.precision(), c.recall(), c.f1()), (None, None, None));
    }

    #[test]
    fn nothing_predicted_has_zero_recall_and_no_precision() {
        let c = Counts {
            tp: 0,
            fp: 0,
            fn_: 3,
        };
        assert_eq!(
            (c.precision(), c.recall(), c.f1()),
            (None, Some(0.0), Some(0.0))
        );
    }
}
