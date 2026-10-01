//! Generators and structural validators for each identifier type. Validators accept the formatted forms the
//! generators emit (spaces, hyphens, any letter case) so they can check gold spans taken verbatim from text.

pub mod aadhaar;
pub mod banking;
pub mod card;
pub mod contact;
pub mod date;
pub mod documents;
pub mod tax;

use parda_spec::entity::EntityType;
use rand::RngExt;
use rand::seq::IndexedRandom;

use crate::DataRng;

/// How a long digit string is broken up when written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouping {
    Plain,
    Spaced,
    Hyphenated,
}

impl Grouping {
    pub const ALL: [Self; 3] = [Self::Plain, Self::Spaced, Self::Hyphenated];

    fn separator(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Spaced => " ",
            Self::Hyphenated => "-",
        }
    }

    /// Joins `parts` with this grouping's separator.
    #[must_use]
    pub fn join(self, parts: &[&str]) -> String {
        parts.join(self.separator())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Upper,
    Lower,
}

impl Case {
    #[must_use]
    pub fn apply(self, s: &str) -> String {
        match self {
            Self::Upper => s.to_uppercase(),
            Self::Lower => s.to_lowercase(),
        }
    }
}

/// Structural validity of `text` as `entity`; `None` for types without a checkable structure (names, addresses, dates).
#[must_use]
pub fn is_valid(entity: EntityType, text: &str) -> Option<bool> {
    let valid = match entity {
        EntityType::Aadhaar => aadhaar::is_valid_aadhaar(text),
        EntityType::AadhaarVid => aadhaar::is_valid_vid(text),
        EntityType::Pan => tax::is_valid_pan(text),
        EntityType::Gstin => tax::is_valid_gstin(text),
        EntityType::Ifsc => banking::is_valid_ifsc(text),
        EntityType::UpiId => banking::is_valid_upi(text),
        EntityType::Phone => contact::is_valid_phone(text),
        EntityType::Email => contact::is_valid_email(text),
        EntityType::PaymentCard => card::is_valid_card(text),
        EntityType::Passport => documents::is_valid_passport(text),
        EntityType::VoterId => documents::is_valid_voter_id(text),
        EntityType::VehicleRegistration => documents::is_valid_vehicle(text),
        EntityType::PersonName | EntityType::Address | EntityType::DateOfBirth => return None,
    };
    Some(valid)
}

fn random_digits(rng: &mut DataRng, len: usize) -> Vec<u8> {
    (0..len).map(|_| rng.random_range(0..10)).collect()
}

fn digits_to_string(digits: &[u8]) -> String {
    digits.iter().map(|d| char::from(b'0' + d)).collect()
}

/// Digits written in groups of four (Aadhaar, VID, cards).
fn group_by_four(digits: &[u8], grouping: Grouping) -> String {
    let s = digits_to_string(digits);
    let parts: Vec<&str> = (0..s.len())
        .step_by(4)
        .map(|i| &s[i..(i + 4).min(s.len())])
        .collect();
    grouping.join(&parts)
}

fn random_letters(rng: &mut DataRng, len: usize) -> String {
    (0..len)
        .map(|_| char::from(rng.random_range(b'A'..=b'Z')))
        .collect()
}

pub(crate) fn pick<T: Copy>(rng: &mut DataRng, items: &[T]) -> T {
    *items
        .choose(rng)
        .unwrap_or_else(|| panic!("pick called with an empty list"))
}

/// `text` without spaces and hyphens, for validating grouped forms.
fn ungrouped(text: &str) -> String {
    text.chars().filter(|c| *c != ' ' && *c != '-').collect()
}

fn is_upper_alpha(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_uppercase())
}

fn is_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}
