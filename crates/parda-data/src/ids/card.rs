use rand::RngExt;

use super::{Grouping, group_by_four, pick, random_digits, ungrouped};
use crate::DataRng;
use crate::checksum::{ascii_digits, luhn_check_digit, luhn_is_valid};

/// Issuer prefixes: RuPay, Visa, Mastercard.
const PREFIXES: [&[u8]; 8] = [
    &[6, 0],
    &[6, 5],
    &[8, 1],
    &[5, 0, 8],
    &[4],
    &[5, 1],
    &[5, 4],
    &[2, 2, 2, 1],
];

/// 16-digit payment card number with a valid Luhn check digit.
#[must_use]
pub fn card(rng: &mut DataRng, grouping: Grouping) -> String {
    group_by_four(&valid_digits(rng), grouping)
}

/// Card-shaped number that fails Luhn, for hard negatives.
#[must_use]
pub fn invalid_card(rng: &mut DataRng, grouping: Grouping) -> String {
    let mut digits = valid_digits(rng);
    let last = digits.len() - 1;
    digits[last] = (digits[last] + rng.random_range(1..10)) % 10;
    group_by_four(&digits, grouping)
}

#[must_use]
pub fn is_valid_card(text: &str) -> bool {
    ascii_digits(&ungrouped(text))
        .is_some_and(|d| (13..=19).contains(&d.len()) && luhn_is_valid(&d))
}

fn valid_digits(rng: &mut DataRng) -> Vec<u8> {
    let prefix = pick(rng, &PREFIXES);
    let mut digits = prefix.to_vec();
    digits.extend(random_digits(rng, 15 - prefix.len()));
    digits.push(luhn_check_digit(&digits));
    digits
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn generated_card_validates() {
        let mut rng = DataRng::seed_from_u64(1);
        for grouping in Grouping::ALL {
            for _ in 0..200 {
                let c = card(&mut rng, grouping);
                assert!(is_valid_card(&c), "{c}");
            }
        }
    }

    #[test]
    fn invalid_card_never_validates() {
        let mut rng = DataRng::seed_from_u64(2);
        for _ in 0..500 {
            let c = invalid_card(&mut rng, Grouping::Spaced);
            assert!(!is_valid_card(&c), "{c}");
        }
    }
}
