use rand::RngExt;

use super::{Case, digits_to_string, is_digits, is_upper_alpha, pick, random_letters};
use crate::DataRng;
use crate::checksum::gstin_check_char;

/// Valid 4th PAN characters: the holder type (person, company, HUF, firm, AOP, trust, BOI, local authority, AJP, government).
const PAN_HOLDER_TYPES: &[u8] = b"PCHFATBLJG";
const NOT_PAN_HOLDER_TYPES: &[u8] = b"DEIKMNOQRSUVWXYZ";

/// GST state codes in use.
const GST_STATE_CODES: [&str; 36] = [
    "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24", "26", "27", "29", "30", "32", "33", "34", "35",
    "36", "37", "38", "97",
];

/// PAN for an individual whose surname starts with `surname_initial` (A–Z).
#[must_use]
pub fn pan(rng: &mut DataRng, surname_initial: char, case: Case) -> String {
    case.apply(&pan_with_holder(rng, b'P', surname_initial))
}

/// PAN-shaped code with an impossible holder-type character, for hard negatives.
#[must_use]
pub fn invalid_pan(rng: &mut DataRng) -> String {
    let holder = pick(rng, NOT_PAN_HOLDER_TYPES);
    let initial = char::from(rng.random_range(b'A'..=b'Z'));
    pan_with_holder(rng, holder, initial)
}

/// 15-character GSTIN: state code, the business PAN, entity number, `Z`, check character.
///
/// # Panics
/// Only on a generator bug: every character it produces is in `0-9A-Z`.
#[must_use]
pub fn gstin(rng: &mut DataRng, case: Case) -> String {
    let state = pick(rng, &GST_STATE_CODES);
    let holder = pick(rng, b"CFPT");
    let initial = char::from(rng.random_range(b'A'..=b'Z'));
    let pan = pan_with_holder(rng, holder, initial);
    let entity = char::from(rng.random_range(b'1'..=b'9'));
    let first14 = format!("{state}{pan}{entity}Z");
    let check = gstin_check_char(&first14)
        .unwrap_or_else(|| panic!("generated GSTIN {first14} is not 0-9A-Z"));
    case.apply(&format!("{first14}{check}"))
}

#[must_use]
pub fn is_valid_pan(text: &str) -> bool {
    let t = text.to_ascii_uppercase();
    t.len() == 10
        && t.is_ascii()
        && is_upper_alpha(&t[0..5])
        && PAN_HOLDER_TYPES.contains(&t.as_bytes()[3])
        && is_digits(&t[5..9])
        && is_upper_alpha(&t[9..10])
}

#[must_use]
pub fn is_valid_gstin(text: &str) -> bool {
    let t = text.to_ascii_uppercase();
    t.len() == 15
        && t.is_ascii()
        && GST_STATE_CODES.contains(&&t[0..2])
        && is_valid_pan(&t[2..12])
        && gstin_check_char(&t[0..14]).is_some_and(|c| t.ends_with(c))
}

fn pan_with_holder(rng: &mut DataRng, holder: u8, initial: char) -> String {
    let serial: Vec<u8> = (0..4).map(|_| rng.random_range(0..10)).collect();
    format!(
        "{}{}{}{}{}",
        random_letters(rng, 3),
        char::from(holder),
        initial.to_ascii_uppercase(),
        digits_to_string(&serial),
        random_letters(rng, 1),
    )
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn generated_pan_validates_in_both_cases() {
        let mut rng = DataRng::seed_from_u64(1);
        for case in [Case::Upper, Case::Lower] {
            for _ in 0..200 {
                let p = pan(&mut rng, 'S', case);
                assert!(is_valid_pan(&p), "{p}");
            }
        }
    }

    #[test]
    fn pan_encodes_holder_type_and_surname_initial() {
        let mut rng = DataRng::seed_from_u64(2);
        let p = pan(&mut rng, 's', Case::Upper);
        assert_eq!(&p[3..5], "PS");
    }

    #[test]
    fn invalid_pan_never_validates() {
        let mut rng = DataRng::seed_from_u64(3);
        for _ in 0..200 {
            let p = invalid_pan(&mut rng);
            assert!(!is_valid_pan(&p), "{p}");
        }
    }

    #[test]
    fn published_gstin_example_validates() {
        assert!(is_valid_gstin("27AAPFU0939F1ZV"));
        assert!(!is_valid_gstin("27AAPFU0939F1ZW"));
    }

    #[test]
    fn generated_gstin_validates() {
        let mut rng = DataRng::seed_from_u64(4);
        for _ in 0..200 {
            let g = gstin(&mut rng, Case::Upper);
            assert!(is_valid_gstin(&g), "{g}");
        }
    }

    #[test]
    fn non_ascii_input_does_not_panic() {
        assert!(!is_valid_pan("ABCPक1234A"));
        assert!(!is_valid_gstin("27AAPFक0939F1Z"));
    }
}
