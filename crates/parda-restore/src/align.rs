//! Placeholder discovery and scripted-reply rendering.
//!
//! The mock never assumes a placeholder format. It finds each slot's placeholder by anchoring the scenario's
//! literal input segments in the masked text the tool sent: whatever sits where a slot's value was is the
//! tool's placeholder for it.

use std::collections::BTreeMap;

use parda_spec::scenario::{InputSegment, Mangle, ReplySegment};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AlignError {
    #[error("literal {literal:?} not found where expected in the masked text {masked:?}")]
    LiteralNotFound { literal: String, masked: String },
    #[error(
        "slots {first} and {second} are adjacent with no literal text between them, so they cannot be told apart"
    )]
    AdjacentSlots { first: usize, second: usize },
    #[error("slot {slot} was replaced by nothing")]
    EmptyPlaceholder { slot: usize },
}

/// One slot occurrence and the text the tool put in its place.
pub type SlotText = (usize, String);

/// Splits `masked` into the scenario's literals and per-slot replacements, in input order.
///
/// A slot's text runs to the next occurrence of the following literal, except that a value which reached the
/// upstream unmasked is taken whole even when it contains that literal (`2345 6789 0124` before `" "`).
///
/// # Errors
/// Fails if a literal cannot be found in order, two slots are adjacent, or a slot's replacement is empty.
pub fn align(
    segments: &[InputSegment],
    masked: &str,
    pii: &[String],
) -> Result<Vec<SlotText>, AlignError> {
    let mut found = Vec::new();
    let mut rest = masked;
    let mut pending_slot: Option<usize> = None;
    for segment in segments {
        match segment {
            InputSegment::Literal { text } if text.is_empty() => {}
            InputSegment::Literal { text } => {
                let not_found = || AlignError::LiteralNotFound {
                    literal: text.clone(),
                    masked: masked.to_owned(),
                };
                match pending_slot.take() {
                    Some(slot) => {
                        let unmasked = pii.get(slot).filter(|v| {
                            rest.strip_prefix(v.as_str())
                                .is_some_and(|after| after.starts_with(text.as_str()))
                        });
                        let at = match unmasked {
                            Some(value) => value.len(),
                            None => rest.find(text.as_str()).ok_or_else(not_found)?,
                        };
                        found.push(non_empty(slot, &rest[..at])?);
                        rest = &rest[at + text.len()..];
                    }
                    None => rest = rest.strip_prefix(text.as_str()).ok_or_else(not_found)?,
                }
            }
            InputSegment::Slot { slot } => {
                if let Some(first) = pending_slot.replace(*slot) {
                    return Err(AlignError::AdjacentSlots {
                        first,
                        second: *slot,
                    });
                }
            }
        }
    }
    match pending_slot {
        Some(slot) => found.push(non_empty(slot, rest)?),
        None if !rest.is_empty() => {
            return Err(AlignError::LiteralNotFound {
                literal: "<end of text>".to_owned(),
                masked: masked.to_owned(),
            });
        }
        None => {}
    }
    Ok(found)
}

fn non_empty(slot: usize, text: &str) -> Result<SlotText, AlignError> {
    if text.is_empty() {
        Err(AlignError::EmptyPlaceholder { slot })
    } else {
        Ok((slot, text.to_owned()))
    }
}

/// The real text of an input: literals with slots replaced by their values.
#[must_use]
pub fn render_input(segments: &[InputSegment], pii: &[String]) -> String {
    segments
        .iter()
        .map(|s| match s {
            InputSegment::Literal { text } => text.as_str(),
            InputSegment::Slot { slot } => pii[*slot].as_str(),
        })
        .collect()
}

/// What the mock sends for one chunk: literals, and (fragments of, possibly mangled) placeholders.
///
/// # Errors
/// Returns the first slot whose placeholder has not been learned from an input.
pub fn render_upstream_chunk(
    segments: &[ReplySegment],
    placeholders: &BTreeMap<usize, String>,
) -> Result<String, usize> {
    segments
        .iter()
        .map(|s| match s {
            ReplySegment::Literal { text } => Ok(text.clone()),
            ReplySegment::Slot {
                slot,
                from,
                to,
                mangle,
            } => {
                let placeholder = placeholders.get(slot).ok_or(*slot)?;
                let shaped =
                    mangle.map_or_else(|| placeholder.clone(), |m| apply_mangle(placeholder, m));
                Ok(char_range(&shaped, *from, *to))
            }
        })
        .collect()
}

/// What the client should see for a sequence of chunks: each slot's real value once, at its first fragment.
#[must_use]
pub fn render_expected(chunks: &[Vec<ReplySegment>], pii: &[String]) -> String {
    chunks
        .iter()
        .flatten()
        .map(|s| match s {
            ReplySegment::Literal { text } => text.as_str(),
            ReplySegment::Slot { slot, from, .. } if from.unwrap_or(0) == 0 => pii[*slot].as_str(),
            ReplySegment::Slot { .. } => "",
        })
        .collect()
}

/// Alters a placeholder the way language models do when copying it.
#[must_use]
pub fn apply_mangle(placeholder: &str, mangle: Mangle) -> String {
    match mangle {
        Mangle::Lowercase => placeholder.to_lowercase(),
        Mangle::Uppercase => placeholder.to_uppercase(),
        Mangle::StripDelimiters => core(placeholder).2.to_owned(),
        Mangle::SpacePadded => {
            let (open, close, inner) = core(placeholder);
            if open.is_empty() && close.is_empty() {
                format!(" {inner} ")
            } else {
                format!("{open} {inner} {close}")
            }
        }
    }
}

/// Splits a placeholder into leading delimiters, trailing delimiters, and its alphanumeric core.
#[must_use]
pub fn core(placeholder: &str) -> (&str, &str, &str) {
    let start = placeholder
        .find(char::is_alphanumeric)
        .unwrap_or(placeholder.len());
    let end = placeholder
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map_or(start, |(i, c)| i + c.len_utf8());
    (
        &placeholder[..start],
        &placeholder[end..],
        &placeholder[start..end],
    )
}

/// Characters `from..to` of `s`, clamped to its length.
fn char_range(s: &str, from: Option<usize>, to: Option<usize>) -> String {
    let len = s.chars().count();
    let to = to.unwrap_or(len).min(len);
    let from = from.unwrap_or(0).min(to);
    s.chars().skip(from).take(to - from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(t: &str) -> InputSegment {
        InputSegment::Literal { text: t.to_owned() }
    }

    fn slot(n: usize) -> InputSegment {
        InputSegment::Slot { slot: n }
    }

    fn rslot(
        n: usize,
        from: Option<usize>,
        to: Option<usize>,
        mangle: Option<Mangle>,
    ) -> ReplySegment {
        ReplySegment::Slot {
            slot: n,
            from,
            to,
            mangle,
        }
    }

    fn rlit(t: &str) -> ReplySegment {
        ReplySegment::Literal { text: t.to_owned() }
    }

    #[test]
    fn finds_placeholders_between_literals() {
        let segs = [
            lit("Aadhaar "),
            slot(0),
            lit(" and PAN "),
            slot(1),
            lit("."),
        ];
        let got = align(&segs, "Aadhaar <AADHAAR_1> and PAN [[PAN]].", &[]).unwrap();
        assert_eq!(
            got,
            vec![(0, "<AADHAAR_1>".to_owned()), (1, "[[PAN]]".to_owned())]
        );
    }

    #[test]
    fn slot_at_start_and_end() {
        let got = align(&[slot(0), lit(" is "), slot(1)], "P1 is P2", &[]).unwrap();
        assert_eq!(got, vec![(0, "P1".to_owned()), (1, "P2".to_owned())]);
    }

    #[test]
    fn unmasked_value_is_its_own_placeholder() {
        let got = align(&[lit("Hi "), slot(0)], "Hi Rahul Sharma", &[]).unwrap();
        assert_eq!(got, vec![(0, "Rahul Sharma".to_owned())]);
    }

    #[test]
    fn altered_literal_fails() {
        let err = align(&[lit("My PAN "), slot(0)], "my PAN X", &[]).unwrap_err();
        assert!(matches!(err, AlignError::LiteralNotFound { .. }));
    }

    #[test]
    fn trailing_text_after_last_literal_fails() {
        assert!(align(&[lit("hello")], "hello world", &[]).is_err());
    }

    #[test]
    fn adjacent_slots_are_rejected() {
        assert_eq!(
            align(&[slot(0), slot(1)], "AB", &[]),
            Err(AlignError::AdjacentSlots {
                first: 0,
                second: 1
            })
        );
    }

    #[test]
    fn removed_value_is_reported() {
        assert_eq!(
            align(&[lit("a "), slot(0), lit(" b")], "a  b", &[]),
            Err(AlignError::EmptyPlaceholder { slot: 0 })
        );
    }

    #[test]
    fn unmasked_value_containing_the_next_literal_is_taken_whole() {
        let segs = [lit("IDs: "), slot(0), lit(" "), slot(1)];
        let pii = ["2345 6789 0124".to_owned(), "ABCPS1234K".to_owned()];
        let got = align(&segs, "IDs: 2345 6789 0124 ABCPS1234K", &pii).unwrap();
        assert_eq!(got, vec![(0, pii[0].clone()), (1, pii[1].clone())]);
        let masked = align(&segs, "IDs: [AADHAAR_1] [PAN_1]", &pii).unwrap();
        assert_eq!(masked[0], (0, "[AADHAAR_1]".to_owned()));
    }

    #[test]
    fn devanagari_literals_align() {
        let got = align(
            &[lit("मेरा नाम "), slot(0), lit(" है।")],
            "मेरा नाम <PERSON_1> है।",
            &[],
        )
        .unwrap();
        assert_eq!(got, vec![(0, "<PERSON_1>".to_owned())]);
    }

    #[test]
    fn mangles_like_models_do() {
        assert_eq!(apply_mangle("<PERSON_1>", Mangle::Lowercase), "<person_1>");
        assert_eq!(apply_mangle("[[pan_2]]", Mangle::Uppercase), "[[PAN_2]]");
        assert_eq!(
            apply_mangle("<<PERSON_1>>", Mangle::StripDelimiters),
            "PERSON_1"
        );
        assert_eq!(
            apply_mangle("<PERSON_1>", Mangle::SpacePadded),
            "< PERSON_1 >"
        );
        assert_eq!(apply_mangle("PERSON_1", Mangle::SpacePadded), " PERSON_1 ");
    }

    #[test]
    fn split_placeholder_renders_fragments_and_expects_value_once() {
        let placeholders = BTreeMap::from([(0, "<PAN_1>".to_owned())]);
        let chunks = vec![
            vec![rlit("PAN: "), rslot(0, None, Some(3), None)],
            vec![rslot(0, Some(3), None, None), rlit(".")],
        ];
        assert_eq!(
            render_upstream_chunk(&chunks[0], &placeholders),
            Ok("PAN: <PA".to_owned())
        );
        assert_eq!(
            render_upstream_chunk(&chunks[1], &placeholders),
            Ok("N_1>.".to_owned())
        );
        assert_eq!(
            render_expected(&chunks, &["ABCPS1234K".to_owned()]),
            "PAN: ABCPS1234K."
        );
    }

    #[test]
    fn fragment_bounds_are_clamped_to_the_placeholder() {
        let placeholders = BTreeMap::from([(0, "P1".to_owned())]);
        assert_eq!(
            render_upstream_chunk(&[rslot(0, Some(5), Some(9), None)], &placeholders),
            Ok(String::new())
        );
    }

    #[test]
    fn unknown_placeholder_is_an_error() {
        assert_eq!(
            render_upstream_chunk(&[rslot(3, None, None, None)], &BTreeMap::new()),
            Err(3)
        );
    }

    #[test]
    fn renders_real_input() {
        let segs = [lit("Call "), slot(0), lit(" now")];
        assert_eq!(
            render_input(&segs, &["+91 98765 43210".to_owned()]),
            "Call +91 98765 43210 now"
        );
    }
}
