use rand::RngExt;

use super::{digits_to_string, pick, random_digits, ungrouped};
use crate::DataRng;
use crate::checksum::ascii_digits;
use crate::ids::banking::ascii_lower;

const EMAIL_DOMAINS: [&str; 6] = [
    "gmail.com",
    "yahoo.co.in",
    "rediffmail.com",
    "outlook.com",
    "hotmail.com",
    "yahoo.com",
];

/// How a mobile number is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneStyle {
    /// `+91 98765 43210`
    Intl,
    /// `+91-9876543210`
    IntlHyphen,
    /// `09876543210`
    TrunkZero,
    /// `9876543210`
    Plain,
    /// `98765 43210`
    Split,
}

impl PhoneStyle {
    pub const ALL: [Self; 5] = [
        Self::Intl,
        Self::IntlHyphen,
        Self::TrunkZero,
        Self::Plain,
        Self::Split,
    ];
}

/// Ten mobile digits, first digit 6–9.
#[must_use]
pub fn mobile_digits(rng: &mut DataRng) -> String {
    let mut digits = random_digits(rng, 10);
    digits[0] = rng.random_range(6..10);
    digits_to_string(&digits)
}

/// Writes ten mobile digits in `style`.
#[must_use]
pub fn format_phone(digits: &str, style: PhoneStyle) -> String {
    let (a, b) = digits.split_at(5);
    match style {
        PhoneStyle::Intl => format!("+91 {a} {b}"),
        PhoneStyle::IntlHyphen => format!("+91-{digits}"),
        PhoneStyle::TrunkZero => format!("0{digits}"),
        PhoneStyle::Plain => digits.to_owned(),
        PhoneStyle::Split => format!("{a} {b}"),
    }
}

/// Ten digits that are not a mobile number in this context (a railway PNR), for hard negatives.
#[must_use]
pub fn pnr(rng: &mut DataRng) -> String {
    let mut digits = random_digits(rng, 10);
    digits[0] = rng.random_range(2..9);
    digits_to_string(&digits)
}

/// Letter case of the name parts in an email's user part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailCase {
    Lower,
    /// `Rahul.Sharma@…`, as people sometimes type it.
    Title,
}

/// Email derived from a person's name.
#[must_use]
pub fn email(rng: &mut DataRng, first: &str, last: &str, case: EmailCase) -> String {
    let (f, l) = match case {
        EmailCase::Lower => (ascii_lower(first), ascii_lower(last)),
        EmailCase::Title => (title(first), title(last)),
    };
    let local = match rng.random_range(0..4) {
        0 => format!("{f}.{l}"),
        1 => format!("{f}{l}{}", rng.random_range(1..100)),
        2 => format!("{f}_{l}{}", rng.random_range(70..100)),
        _ => format!("{}{l}", f.chars().take(1).collect::<String>()),
    };
    format!("{local}@{}", pick(rng, &EMAIL_DOMAINS))
}

#[must_use]
pub fn is_valid_phone(text: &str) -> bool {
    let t = ungrouped(text);
    let national = t
        .strip_prefix("+91")
        .or_else(|| t.strip_prefix("91").filter(|r| r.len() == 10))
        .or_else(|| t.strip_prefix('0').filter(|r| r.len() == 10))
        .unwrap_or(&t);
    ascii_digits(national).is_some_and(|d| d.len() == 10 && d[0] >= 6)
}

#[must_use]
pub fn is_valid_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !local.contains(char::is_whitespace)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
}

fn title(s: &str) -> String {
    let lower = ascii_lower(s);
    let mut chars = lower.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn every_phone_style_validates() {
        let mut rng = DataRng::seed_from_u64(1);
        for style in PhoneStyle::ALL {
            for _ in 0..50 {
                let p = format_phone(&mobile_digits(&mut rng), style);
                assert!(is_valid_phone(&p), "{p}");
            }
        }
    }

    #[test]
    fn landline_style_number_is_not_a_mobile() {
        assert!(!is_valid_phone("2234567890"));
    }

    #[test]
    fn generated_email_validates() {
        let mut rng = DataRng::seed_from_u64(2);
        for case in [EmailCase::Lower, EmailCase::Title] {
            for _ in 0..50 {
                let e = email(&mut rng, "Ananya", "D'Souza", case);
                assert!(is_valid_email(&e), "{e}");
            }
        }
    }
}
