use rand::RngExt;

use super::{Grouping, group_by_four, random_digits, ungrouped};
use crate::DataRng;
use crate::checksum::{ascii_digits, verhoeff_check_digit, verhoeff_is_valid};

/// 12-digit Aadhaar: first digit 2–9, Verhoeff check digit last.
#[must_use]
pub fn aadhaar(rng: &mut DataRng, grouping: Grouping) -> String {
    let digits = with_check_digit(leading_nonzero(rng, 2, 11));
    group_by_four(&digits, grouping)
}

/// 16-digit Virtual ID with a Verhoeff check digit.
#[must_use]
pub fn vid(rng: &mut DataRng, grouping: Grouping) -> String {
    let digits = with_check_digit(leading_nonzero(rng, 1, 15));
    group_by_four(&digits, grouping)
}

/// Aadhaar-shaped number whose check digit is wrong, for hard negatives.
#[must_use]
pub fn invalid_aadhaar(rng: &mut DataRng, grouping: Grouping) -> String {
    let mut digits = with_check_digit(leading_nonzero(rng, 2, 11));
    let last = digits.len() - 1;
    digits[last] = (digits[last] + rng.random_range(1..10)) % 10;
    group_by_four(&digits, grouping)
}

#[must_use]
pub fn is_valid_aadhaar(text: &str) -> bool {
    ascii_digits(&ungrouped(text))
        .is_some_and(|d| d.len() == 12 && d[0] >= 2 && verhoeff_is_valid(&d))
}

#[must_use]
pub fn is_valid_vid(text: &str) -> bool {
    ascii_digits(&ungrouped(text)).is_some_and(|d| d.len() == 16 && verhoeff_is_valid(&d))
}

fn leading_nonzero(rng: &mut DataRng, min_first: u8, len: usize) -> Vec<u8> {
    let mut digits = random_digits(rng, len);
    digits[0] = rng.random_range(min_first..10);
    digits
}

fn with_check_digit(mut digits: Vec<u8>) -> Vec<u8> {
    digits.push(verhoeff_check_digit(&digits));
    digits
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;
    use crate::ids::digits_to_string;

    #[test]
    fn generated_aadhaar_validates_in_every_grouping() {
        let mut rng = DataRng::seed_from_u64(1);
        for grouping in Grouping::ALL {
            for _ in 0..200 {
                let a = aadhaar(&mut rng, grouping);
                assert!(is_valid_aadhaar(&a), "{a}");
            }
        }
    }

    #[test]
    fn spaced_aadhaar_has_three_groups() {
        let mut rng = DataRng::seed_from_u64(2);
        let a = aadhaar(&mut rng, Grouping::Spaced);
        assert_eq!(a.split(' ').map(str::len).collect::<Vec<_>>(), [4, 4, 4]);
    }

    #[test]
    fn invalid_aadhaar_never_validates() {
        let mut rng = DataRng::seed_from_u64(3);
        for _ in 0..500 {
            let a = invalid_aadhaar(&mut rng, Grouping::Plain);
            assert!(!is_valid_aadhaar(&a), "{a}");
        }
    }

    #[test]
    fn aadhaar_starting_with_one_is_rejected() {
        let mut d = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1];
        d.push(verhoeff_check_digit(&d));
        assert!(!is_valid_aadhaar(&digits_to_string(&d)));
    }

    #[test]
    fn generated_vid_validates() {
        let mut rng = DataRng::seed_from_u64(4);
        for _ in 0..200 {
            let v = vid(&mut rng, Grouping::Spaced);
            assert!(is_valid_vid(&v), "{v}");
        }
    }
}
