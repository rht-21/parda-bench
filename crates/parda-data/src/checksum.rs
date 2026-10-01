//! Check-digit schemes used by Indian identifiers: Verhoeff (Aadhaar, VID), Luhn (payment cards), GSTIN mod 36.

const VERHOEFF_D: [[u8; 10]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 2, 3, 4, 0, 6, 7, 8, 9, 5],
    [2, 3, 4, 0, 1, 7, 8, 9, 5, 6],
    [3, 4, 0, 1, 2, 8, 9, 5, 6, 7],
    [4, 0, 1, 2, 3, 9, 5, 6, 7, 8],
    [5, 9, 8, 7, 6, 0, 4, 3, 2, 1],
    [6, 5, 9, 8, 7, 1, 0, 4, 3, 2],
    [7, 6, 5, 9, 8, 2, 1, 0, 4, 3],
    [8, 7, 6, 5, 9, 3, 2, 1, 0, 4],
    [9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
];

const VERHOEFF_P: [[u8; 10]; 8] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    [1, 5, 7, 6, 2, 8, 3, 0, 9, 4],
    [5, 8, 0, 3, 7, 9, 6, 1, 4, 2],
    [8, 9, 1, 6, 0, 4, 3, 5, 2, 7],
    [9, 4, 5, 3, 1, 2, 6, 8, 7, 0],
    [4, 2, 8, 6, 5, 7, 3, 9, 0, 1],
    [2, 7, 9, 3, 8, 0, 6, 4, 1, 5],
    [7, 0, 4, 6, 9, 1, 3, 2, 5, 8],
];

const VERHOEFF_INV: [u8; 10] = [0, 4, 3, 2, 1, 5, 6, 7, 8, 9];

const GSTIN_CHARSET: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Folds digits right to left through the Verhoeff tables; `offset` is 1 when computing a missing check digit.
fn verhoeff_fold(digits: &[u8], offset: usize) -> u8 {
    digits.iter().rev().enumerate().fold(0, |c, (i, &d)| {
        VERHOEFF_D[usize::from(c)][usize::from(VERHOEFF_P[(i + offset) % 8][usize::from(d)])]
    })
}

/// Check digit to append to `digits` (each 0–9).
#[must_use]
pub fn verhoeff_check_digit(digits: &[u8]) -> u8 {
    VERHOEFF_INV[usize::from(verhoeff_fold(digits, 1))]
}

/// Whether `digits` (each 0–9), including the trailing check digit, pass Verhoeff.
#[must_use]
pub fn verhoeff_is_valid(digits: &[u8]) -> bool {
    !digits.is_empty() && verhoeff_fold(digits, 0) == 0
}

/// Check digit to append to `digits` (each 0–9).
#[must_use]
pub fn luhn_check_digit(digits: &[u8]) -> u8 {
    const COMPLEMENT: [u8; 10] = [0, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    COMPLEMENT[luhn_sum(digits, 1) % 10]
}

/// Whether `digits` (each 0–9), including the trailing check digit, pass Luhn.
#[must_use]
pub fn luhn_is_valid(digits: &[u8]) -> bool {
    !digits.is_empty() && luhn_sum(digits, 0).is_multiple_of(10)
}

/// Luhn digit sum right to left; `offset` is 1 when computing a missing check digit.
fn luhn_sum(digits: &[u8], offset: usize) -> usize {
    digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            let d = usize::from(d);
            if (i + offset) % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum()
}

/// GSTIN check character for the first 14 characters; `None` if any is outside `0-9A-Z`.
#[must_use]
pub fn gstin_check_char(first14: &str) -> Option<char> {
    let mut sum = 0usize;
    for (i, ch) in first14.bytes().enumerate() {
        let value = GSTIN_CHARSET.iter().position(|&c| c == ch)?;
        let product = value * if i % 2 == 0 { 1 } else { 2 };
        sum += product / 36 + product % 36;
    }
    Some(char::from(GSTIN_CHARSET[(36 - sum % 36) % 36]))
}

/// Digits of `s` as values 0–9; `None` if `s` contains anything but ASCII digits.
#[must_use]
pub fn ascii_digits(s: &str) -> Option<Vec<u8>> {
    s.bytes()
        .map(|b| b.is_ascii_digit().then(|| b - b'0'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digits(s: &str) -> Vec<u8> {
        ascii_digits(s).unwrap_or_default()
    }

    #[test]
    fn verhoeff_matches_reference_example() {
        assert_eq!(verhoeff_check_digit(&digits("236")), 3);
        assert!(verhoeff_is_valid(&digits("2363")));
    }

    #[test]
    fn verhoeff_rejects_single_digit_error_and_adjacent_swap() {
        assert!(!verhoeff_is_valid(&digits("2364")));
        assert!(!verhoeff_is_valid(&digits("3263")));
    }

    #[test]
    fn verhoeff_generated_digit_always_validates() {
        for n in 0..2000u32 {
            let mut d = digits(&format!("{:011}", n * 7919));
            d.push(verhoeff_check_digit(&d));
            assert!(verhoeff_is_valid(&d), "{d:?}");
        }
    }

    #[test]
    fn luhn_matches_reference_card_number() {
        assert!(luhn_is_valid(&digits("4111111111111111")));
        assert_eq!(luhn_check_digit(&digits("411111111111111")), 1);
        assert_eq!(luhn_check_digit(&digits("7992739871")), 3);
        assert!(!luhn_is_valid(&digits("4111111111111112")));
    }

    #[test]
    fn gstin_matches_published_example() {
        assert_eq!(gstin_check_char("27AAPFU0939F1Z"), Some('V'));
    }

    #[test]
    fn gstin_rejects_lowercase_input() {
        assert_eq!(gstin_check_char("27aapfu0939f1z"), None);
    }

    #[test]
    fn ascii_digits_rejects_non_digits() {
        assert_eq!(ascii_digits("12a"), None);
        assert_eq!(ascii_digits("१२"), None);
    }
}
