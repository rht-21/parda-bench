//! Placeholder discovery and scripted-reply rendering.
//!
//! The mock never assumes a placeholder format. It finds each slot's placeholder by anchoring the scenario's
//! literal input segments in the masked text the tool sent: whatever sits where a slot's value was is the
//! tool's placeholder for it.

use std::collections::BTreeMap;

use parda_spec::scenario::{InputSegment, Mangle, ReplySegment};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AlignError {
    #[error("the masked text does not contain the script's literal text in order: {masked:?}")]
    Unaligned { masked: String },
    #[error(
        "slots {first} and {second} are adjacent with no literal text between them, so they cannot be told apart"
    )]
    AdjacentSlots { first: usize, second: usize },
    #[error("slot {slot} was replaced by nothing")]
    EmptyPlaceholder { slot: usize },
}

/// One slot occurrence and the text the tool put in its place.
pub type SlotText = (usize, String);

/// An input as a leading literal, then each slot with the literal that follows it (empty only after the last).
struct Script<'a> {
    lead: &'a str,
    slots: Vec<(usize, &'a str)>,
}

/// Splits `masked` into the scenario's literals and per-slot replacements, in input order.
///
/// A slot's text runs to an occurrence of the following literal such that the rest of the input still matches;
/// the earliest such occurrence wins. A value that reached the upstream unmasked is preferred whole, even when
/// it contains that literal (`2345 6789 0124` before `" "`).
///
/// # Errors
/// Fails if the literals cannot be matched in order, two slots are adjacent, or a slot's replacement is empty.
pub fn align(
    segments: &[InputSegment],
    masked: &str,
    pii: &[String],
) -> Result<Vec<SlotText>, AlignError> {
    let script = script(segments)?;
    let found = masked
        .strip_prefix(script.lead)
        .and_then(|rest| search(&script.slots, rest, pii))
        .ok_or_else(|| AlignError::Unaligned {
            masked: masked.to_owned(),
        })?;
    match found.iter().find(|(_, text)| text.is_empty()) {
        Some((slot, _)) => Err(AlignError::EmptyPlaceholder { slot: *slot }),
        None => Ok(found),
    }
}

fn script(segments: &[InputSegment]) -> Result<Script<'_>, AlignError> {
    let mut lead = "";
    let mut slots: Vec<(usize, &str)> = Vec::new();
    for segment in segments {
        match segment {
            InputSegment::Literal { text } if text.is_empty() => {}
            InputSegment::Literal { text } => match slots.last_mut() {
                Some((_, following)) => *following = text,
                None => lead = text,
            },
            InputSegment::Slot { slot } => {
                if let Some((first, "")) = slots.last() {
                    return Err(AlignError::AdjacentSlots {
                        first: *first,
                        second: *slot,
                    });
                }
                slots.push((*slot, ""));
            }
        }
    }
    Ok(Script { lead, slots })
}

/// Matches `rest` against the remaining slots, backtracking over where each slot's text ends.
fn search(slots: &[(usize, &str)], rest: &str, pii: &[String]) -> Option<Vec<SlotText>> {
    match slots {
        [] => rest.is_empty().then(Vec::new),
        [(slot, "")] => Some(vec![(*slot, rest.to_owned())]),
        [(slot, literal), tail @ ..] => split_points(rest, literal, pii.get(*slot))
            .into_iter()
            .find_map(|at| {
                let mut found = search(tail, &rest[at + literal.len()..], pii)?;
                found.insert(0, (*slot, rest[..at].to_owned()));
                Some(found)
            }),
    }
}

/// Where a slot's text may end: the unmasked value first, then every occurrence of `literal`, empty text last.
fn split_points(rest: &str, literal: &str, unmasked: Option<&String>) -> Vec<usize> {
    let whole_value = unmasked
        .filter(|v| {
            rest.strip_prefix(v.as_str())
                .is_some_and(|after| after.starts_with(literal))
        })
        .map(String::len);
    let occurrences =
        (1..rest.len()).filter(|&i| rest.is_char_boundary(i) && rest[i..].starts_with(literal));
    let empty = rest.starts_with(literal).then_some(0);
    whole_value
        .into_iter()
        .chain(occurrences)
        .chain(empty)
        .collect()
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
        assert!(matches!(err, AlignError::Unaligned { .. }));
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
    fn placeholder_containing_the_next_literal_backtracks_to_a_later_match() {
        let segs = [lit("My email is "), slot(0), lit(".")];
        let got = align(&segs, "My email is blair@example.com.", &[]).unwrap();
        assert_eq!(got, vec![(0, "blair@example.com".to_owned())]);
    }

    #[test]
    fn backtracking_keeps_later_literals_in_place() {
        let segs = [lit("Name: "), slot(0), lit(", email "), slot(1), lit(".")];
        let got = align(&segs, "Name: Rhonda Smith, email r.smith@example.org.", &[]).unwrap();
        assert_eq!(
            got,
            vec![
                (0, "Rhonda Smith".to_owned()),
                (1, "r.smith@example.org".to_owned())
            ]
        );
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
