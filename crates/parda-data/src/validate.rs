//! Dataset integrity checks: span geometry, structural validity of labeled identifiers, unique ids.

use std::collections::HashSet;

use parda_spec::sample::{Labels, Sample};

use crate::ids::is_valid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub sample_id: String,
    pub message: String,
}

/// Every problem found in `samples`; empty means the dataset is consistent.
#[must_use]
pub fn validate(samples: &[Sample]) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut ids = HashSet::new();
    for sample in samples {
        let mut report = |message: String| {
            issues.push(Issue {
                sample_id: sample.id.clone(),
                message,
            });
        };
        if !ids.insert(sample.id.as_str()) {
            report("duplicate id".to_owned());
        }
        if sample.text.trim().is_empty() {
            report("empty text".to_owned());
        }
        if let Labels::Positive { spans } = &sample.labels {
            let chars: Vec<char> = sample.text.chars().collect();
            let mut previous_end = 0;
            for span in spans {
                if span.start >= span.end || span.end > chars.len() {
                    report(format!(
                        "span {}..{} out of bounds for {} chars",
                        span.start,
                        span.end,
                        chars.len()
                    ));
                    continue;
                }
                if span.start < previous_end {
                    report(format!(
                        "span {}..{} overlaps or is out of order",
                        span.start, span.end
                    ));
                }
                previous_end = span.end;
                let covered: String = chars[span.start..span.end].iter().collect();
                if covered.trim() != covered {
                    report(format!("span {covered:?} has surrounding whitespace"));
                }
                if is_valid(span.entity, &covered) == Some(false) {
                    report(format!("{covered:?} is not a valid {:?}", span.entity));
                }
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use parda_spec::entity::EntityType;
    use parda_spec::sample::{Difficulty, Lang, Source, Span};

    use super::*;

    fn sample(id: &str, text: &str, spans: Vec<Span>) -> Sample {
        Sample {
            id: id.to_owned(),
            text: text.to_owned(),
            labels: Labels::Positive { spans },
            lang: Lang::En,
            difficulty: Difficulty::Easy,
            source: Source::Handwritten,
            generator_version: "test".to_owned(),
        }
    }

    fn span(start: usize, end: usize, entity: EntityType) -> Span {
        Span { start, end, entity }
    }

    #[test]
    fn accepts_a_consistent_sample() {
        let s = sample("a", "PAN ABCPS1234K ok", vec![span(4, 14, EntityType::Pan)]);
        assert!(validate(&[s]).is_empty());
    }

    #[test]
    fn flags_invalid_checksum_inside_span() {
        let text = |last: char| format!("Aadhaar 2345 6789 012{last}");
        let valid = sample("a", &text('4'), vec![span(8, 22, EntityType::Aadhaar)]);
        let invalid = sample("b", &text('5'), vec![span(8, 22, EntityType::Aadhaar)]);
        assert!(validate(&[valid]).is_empty());
        assert_eq!(validate(&[invalid]).len(), 1);
    }

    #[test]
    fn flags_span_past_end_and_duplicate_id() {
        let a = sample("a", "short", vec![span(2, 9, EntityType::PersonName)]);
        let b = sample("a", "short", vec![]);
        let messages: Vec<String> = validate(&[a, b]).into_iter().map(|i| i.message).collect();
        assert_eq!(messages.len(), 2, "{messages:?}");
    }

    #[test]
    fn flags_overlapping_spans() {
        let name = EntityType::PersonName;
        let s = sample(
            "a",
            "Rahul Sharma",
            vec![span(0, 5, name), span(3, 12, name)],
        );
        assert_eq!(validate(&[s]).len(), 1);
    }

    #[test]
    fn flags_span_with_surrounding_whitespace() {
        let s = sample(
            "a",
            "Rahul Sharma",
            vec![span(0, 6, EntityType::PersonName)],
        );
        assert_eq!(validate(&[s]).len(), 1);
    }
}
