use rand::RngExt;

use super::{
    Grouping, digits_to_string, is_digits, is_upper_alpha, pick, random_digits, random_letters,
    ungrouped,
};
use crate::DataRng;

/// Passport series letters; Q, X and Z are not issued.
const PASSPORT_LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPRSTUVWY";

const VEHICLE_STATES: [&str; 15] = [
    "MH", "DL", "KA", "TN", "UP", "GJ", "RJ", "WB", "TS", "KL", "HR", "PB", "MP", "AP", "BR",
];

/// Passport number: series letter, then 7 digits with non-zero first and last digit.
#[must_use]
pub fn passport(rng: &mut DataRng) -> String {
    format!(
        "{}{}",
        char::from(pick(rng, PASSPORT_LETTERS)),
        passport_digits(rng)
    )
}

/// Voter ID (EPIC): 3 letters, 7 digits.
#[must_use]
pub fn voter_id(rng: &mut DataRng) -> String {
    format!(
        "{}{}",
        random_letters(rng, 3),
        digits_to_string(&random_digits(rng, 7))
    )
}

/// Vehicle registration: state, RTO number, 1–2 series letters, 4-digit number.
#[must_use]
pub fn vehicle(rng: &mut DataRng, grouping: Grouping) -> String {
    let state = pick(rng, &VEHICLE_STATES);
    let rto = format!("{:02}", rng.random_range(1..100));
    let series_len = rng.random_range(1..=2);
    let series = random_letters(rng, series_len);
    let number = format!("{:04}", rng.random_range(1..10_000));
    grouping.join(&[state, &rto, &series, &number])
}

#[must_use]
pub fn is_valid_passport(text: &str) -> bool {
    let t = text.to_ascii_uppercase();
    t.len() == 8
        && t.is_ascii()
        && PASSPORT_LETTERS.contains(&t.as_bytes()[0])
        && is_digits(&t[1..])
        && t.as_bytes()[1] != b'0'
        && t.as_bytes()[7] != b'0'
}

#[must_use]
pub fn is_valid_voter_id(text: &str) -> bool {
    let t = text.to_ascii_uppercase();
    t.len() == 10 && t.is_ascii() && is_upper_alpha(&t[0..3]) && is_digits(&t[3..])
}

#[must_use]
pub fn is_valid_vehicle(text: &str) -> bool {
    let t = ungrouped(text).to_ascii_uppercase();
    if !t.is_ascii() || !(9..=10).contains(&t.len()) {
        return false;
    }
    let series_len = t.len() - 8;
    is_upper_alpha(&t[0..2])
        && is_digits(&t[2..4])
        && is_upper_alpha(&t[4..4 + series_len])
        && is_digits(&t[4 + series_len..])
}

fn passport_digits(rng: &mut DataRng) -> String {
    let mut digits = random_digits(rng, 7);
    digits[0] = rng.random_range(1..10);
    digits[6] = rng.random_range(1..10);
    digits_to_string(&digits)
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn generated_documents_validate() {
        let mut rng = DataRng::seed_from_u64(1);
        for _ in 0..200 {
            let p = passport(&mut rng);
            assert!(is_valid_passport(&p), "{p}");
            let v = voter_id(&mut rng);
            assert!(is_valid_voter_id(&v), "{v}");
            for grouping in Grouping::ALL {
                let r = vehicle(&mut rng, grouping);
                assert!(is_valid_vehicle(&r), "{r}");
            }
        }
    }

    #[test]
    fn passport_series_q_is_rejected() {
        assert!(!is_valid_passport("Q1234567"));
        assert!(is_valid_passport("J8369854"));
    }
}
