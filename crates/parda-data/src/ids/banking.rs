use rand::RngExt;

use super::{Case, digits_to_string, is_upper_alpha, pick, random_digits};
use crate::DataRng;

const BANK_CODES: [&str; 15] = [
    "SBIN", "HDFC", "ICIC", "UTIB", "PUNB", "BARB", "CNRB", "KKBK", "YESB", "IDIB", "UBIN", "BKID",
    "IOBA", "INDB", "FDRL",
];

/// UPI payment service provider handles.
const UPI_HANDLES: [&str; 9] = [
    "okaxis",
    "oksbi",
    "okhdfcbank",
    "okicici",
    "ybl",
    "paytm",
    "ibl",
    "axl",
    "apl",
];

/// IFSC: 4-letter bank code, `0`, 6-character branch code.
#[must_use]
pub fn ifsc(rng: &mut DataRng, case: Case) -> String {
    let bank = pick(rng, &BANK_CODES);
    case.apply(&format!(
        "{bank}0{}",
        digits_to_string(&random_digits(rng, 6))
    ))
}

/// UPI ID `local@handle`; `local` is a name-derived or mobile-number user part.
#[must_use]
pub fn upi(rng: &mut DataRng, local: &str) -> String {
    format!("{local}@{}", pick(rng, &UPI_HANDLES))
}

/// Name-derived UPI user part, e.g. `rahul.sharma` or `rahulsharma92`.
#[must_use]
pub fn upi_local_from_name(rng: &mut DataRng, first: &str, last: &str) -> String {
    let (first, last) = (ascii_lower(first), ascii_lower(last));
    match rng.random_range(0..3) {
        0 => format!("{first}.{last}"),
        1 => format!("{first}{last}{}", rng.random_range(10..100)),
        _ => format!("{first}{}", rng.random_range(100..1000)),
    }
}

#[must_use]
pub fn is_valid_ifsc(text: &str) -> bool {
    let t = text.to_ascii_uppercase();
    t.len() == 11
        && t.is_ascii()
        && is_upper_alpha(&t[0..4])
        && t.as_bytes()[4] == b'0'
        && t[5..].bytes().all(|b| b.is_ascii_alphanumeric())
}

#[must_use]
pub fn is_valid_upi(text: &str) -> bool {
    let Some((local, handle)) = text.split_once('@') else {
        return false;
    };
    (2..=256).contains(&local.len())
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && (2..=64).contains(&handle.len())
        && handle.bytes().all(|b| b.is_ascii_alphabetic())
}

/// Lowercase ASCII letters of `s` only, for building handles from names like `D'Souza`.
pub(crate) fn ascii_lower(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn generated_ifsc_validates() {
        let mut rng = DataRng::seed_from_u64(1);
        for _ in 0..100 {
            let i = ifsc(&mut rng, Case::Upper);
            assert!(is_valid_ifsc(&i), "{i}");
        }
    }

    #[test]
    fn generated_upi_validates_and_has_no_dot_in_handle() {
        let mut rng = DataRng::seed_from_u64(2);
        for _ in 0..100 {
            let local = upi_local_from_name(&mut rng, "Joseph", "D'Souza");
            let u = upi(&mut rng, &local);
            assert!(is_valid_upi(&u), "{u}");
        }
    }

    #[test]
    fn email_is_not_a_upi_id() {
        assert!(!is_valid_upi("rahul@gmail.com"));
    }
}
