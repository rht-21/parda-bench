//! Template parsing. A template is text with `{kind}` or `{kind:style}` fillers; `{{` and `}}` are literal braces.
//! A template holds either only real-entity fillers, or exactly one `decoy_*` filler (a hard negative).

use parda_spec::entity::EntityType;
use parda_spec::sample::{Difficulty, Lang};
use serde::Deserialize;

use crate::ids::Case;
use crate::ids::Grouping;
use crate::ids::contact::{EmailCase, PhoneStyle};
use crate::ids::date::DateStyle;

#[derive(Debug, thiserror::Error)]
#[error("template {id}: {message}")]
pub struct TemplateError {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Latin,
    Devanagari,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStyle {
    Full,
    First,
    /// `RAHUL SHARMA`, as on forms and ID cards.
    Upper,
    Deva,
    DevaFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpiStyle {
    Name,
    Phone,
}

/// A slot that renders a real entity of the sample's persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filler {
    Aadhaar(Option<Grouping>),
    AadhaarVid(Option<Grouping>),
    Pan(Option<Case>),
    Gstin(Option<Case>),
    Ifsc(Option<Case>),
    Upi(Option<UpiStyle>),
    Phone(Option<PhoneStyle>),
    Email(Option<EmailCase>),
    Card(Option<Grouping>),
    Passport,
    VoterId,
    Vehicle(Option<Grouping>),
    Name(Option<NameStyle>),
    OtherName(Option<NameStyle>),
    Address(Option<Script>),
    Dob(Option<DateStyle>),
}

/// A slot that renders something resembling `entity()` that must not be flagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoy {
    /// Order or tracking number shaped like an Aadhaar, with a failing check digit.
    Aadhaar(Option<Grouping>),
    /// Invoice number shaped like a card, failing Luhn.
    Card(Option<Grouping>),
    /// Product code shaped like a PAN with an impossible holder type.
    Pan,
    /// Railway PNR: ten digits, often starting 6–9 like a mobile.
    Pnr,
    /// A date that is not anyone's date of birth.
    Date(Option<DateStyle>),
    /// A place or institution named after a person.
    NamedPlace,
    /// Booking reference shaped like a passport number.
    Passport,
}

impl Filler {
    #[must_use]
    pub fn entity(self) -> EntityType {
        match self {
            Self::Aadhaar(_) => EntityType::Aadhaar,
            Self::AadhaarVid(_) => EntityType::AadhaarVid,
            Self::Pan(_) => EntityType::Pan,
            Self::Gstin(_) => EntityType::Gstin,
            Self::Ifsc(_) => EntityType::Ifsc,
            Self::Upi(_) => EntityType::UpiId,
            Self::Phone(_) => EntityType::Phone,
            Self::Email(_) => EntityType::Email,
            Self::Card(_) => EntityType::PaymentCard,
            Self::Passport => EntityType::Passport,
            Self::VoterId => EntityType::VoterId,
            Self::Vehicle(_) => EntityType::VehicleRegistration,
            Self::Name(_) | Self::OtherName(_) => EntityType::PersonName,
            Self::Address(_) => EntityType::Address,
            Self::Dob(_) => EntityType::DateOfBirth,
        }
    }
}

impl Decoy {
    #[must_use]
    pub fn entity(self) -> EntityType {
        match self {
            Self::Aadhaar(_) => EntityType::Aadhaar,
            Self::Card(_) => EntityType::PaymentCard,
            Self::Pan => EntityType::Pan,
            Self::Pnr => EntityType::Phone,
            Self::Date(_) => EntityType::DateOfBirth,
            Self::NamedPlace => EntityType::PersonName,
            Self::Passport => EntityType::Passport,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Fill(Filler),
    Decoy(Decoy),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub id: String,
    pub lang: Lang,
    pub difficulty: Difficulty,
    pub parts: Vec<Part>,
}

impl Template {
    /// The decoy of a hard-negative template, `None` for a positive one.
    #[must_use]
    pub fn decoy(&self) -> Option<Decoy> {
        self.parts.iter().find_map(|p| match p {
            Part::Decoy(d) => Some(*d),
            Part::Text(_) | Part::Fill(_) => None,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateFile {
    template: Vec<TemplateDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateDef {
    id: String,
    lang: Lang,
    difficulty: Difficulty,
    text: String,
}

/// Parses a TOML file of `[[template]]` tables.
///
/// # Errors
/// Fails on invalid TOML, an unknown filler or style, unbalanced braces, or a decoy mixed with other fillers.
pub fn parse_file(source: &str) -> Result<Vec<Template>, TemplateError> {
    let file: TemplateFile = toml::from_str(source).map_err(|e| TemplateError {
        id: "<file>".to_owned(),
        message: e.to_string(),
    })?;
    file.template.into_iter().map(parse_def).collect()
}

fn parse_def(def: TemplateDef) -> Result<Template, TemplateError> {
    let err = |message: String| TemplateError {
        id: def.id.clone(),
        message,
    };
    let parts = parse_text(&def.text).map_err(err)?;
    let decoys = parts.iter().filter(|p| matches!(p, Part::Decoy(_))).count();
    let fills = parts.iter().filter(|p| matches!(p, Part::Fill(_))).count();
    if decoys > 1 || (decoys == 1 && fills > 0) {
        return Err(err(
            "a hard negative holds exactly one decoy and no other fillers".to_owned(),
        ));
    }
    Ok(Template {
        id: def.id,
        lang: def.lang,
        difficulty: def.difficulty,
        parts,
    })
}

fn parse_text(text: &str) -> Result<Vec<Part>, String> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '{' => {
                let mut spec = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => spec.push(c),
                        None => return Err(format!("unclosed `{{{spec}`")),
                    }
                }
                if !literal.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut literal)));
                }
                parts.push(parse_slot(&spec)?);
            }
            '}' => return Err("unmatched `}`; write `}}` for a literal brace".to_owned()),
            c => literal.push(c),
        }
    }
    if !literal.is_empty() {
        parts.push(Part::Text(literal));
    }
    Ok(parts)
}

fn parse_slot(spec: &str) -> Result<Part, String> {
    let (kind, style) = match spec.split_once(':') {
        Some((k, s)) => (k, Some(s)),
        None => (spec, None),
    };
    let part = match (kind, style) {
        ("aadhaar", _) => Part::Fill(Filler::Aadhaar(styled(kind, style, grouping)?)),
        ("vid", _) => Part::Fill(Filler::AadhaarVid(styled(kind, style, grouping)?)),
        ("pan", _) => Part::Fill(Filler::Pan(styled(kind, style, case)?)),
        ("gstin", _) => Part::Fill(Filler::Gstin(styled(kind, style, case)?)),
        ("ifsc", _) => Part::Fill(Filler::Ifsc(styled(kind, style, case)?)),
        ("upi", _) => Part::Fill(Filler::Upi(styled(kind, style, upi_style)?)),
        ("phone", _) => Part::Fill(Filler::Phone(styled(kind, style, phone_style)?)),
        ("email", _) => Part::Fill(Filler::Email(styled(kind, style, email_case)?)),
        ("card", _) => Part::Fill(Filler::Card(styled(kind, style, grouping)?)),
        ("vehicle", _) => Part::Fill(Filler::Vehicle(styled(kind, style, grouping)?)),
        ("name", _) => Part::Fill(Filler::Name(styled(kind, style, name_style)?)),
        ("other_name", _) => Part::Fill(Filler::OtherName(styled(kind, style, name_style)?)),
        ("address", _) => Part::Fill(Filler::Address(styled(kind, style, script)?)),
        ("dob", _) => Part::Fill(Filler::Dob(styled(kind, style, date_style)?)),
        ("decoy_aadhaar", _) => Part::Decoy(Decoy::Aadhaar(styled(kind, style, grouping)?)),
        ("decoy_card", _) => Part::Decoy(Decoy::Card(styled(kind, style, grouping)?)),
        ("decoy_date", _) => Part::Decoy(Decoy::Date(styled(kind, style, date_style)?)),
        ("passport", None) => Part::Fill(Filler::Passport),
        ("voter", None) => Part::Fill(Filler::VoterId),
        ("decoy_pan", None) => Part::Decoy(Decoy::Pan),
        ("decoy_pnr", None) => Part::Decoy(Decoy::Pnr),
        ("decoy_place", None) => Part::Decoy(Decoy::NamedPlace),
        ("decoy_passport", None) => Part::Decoy(Decoy::Passport),
        (_, Some(s)) => return Err(format!("unknown filler `{kind}` or style `{s}`")),
        (_, None) => return Err(format!("unknown filler `{kind}`")),
    };
    Ok(part)
}

fn styled<T>(
    kind: &str,
    style: Option<&str>,
    parse: fn(&str) -> Option<T>,
) -> Result<Option<T>, String> {
    style
        .map(|s| parse(s).ok_or_else(|| format!("unknown style `{s}` for `{kind}`")))
        .transpose()
}

fn grouping(s: &str) -> Option<Grouping> {
    match s {
        "plain" => Some(Grouping::Plain),
        "spaced" => Some(Grouping::Spaced),
        "hyphen" => Some(Grouping::Hyphenated),
        _ => None,
    }
}

fn case(s: &str) -> Option<Case> {
    match s {
        "upper" => Some(Case::Upper),
        "lower" => Some(Case::Lower),
        _ => None,
    }
}

fn upi_style(s: &str) -> Option<UpiStyle> {
    match s {
        "name" => Some(UpiStyle::Name),
        "phone" => Some(UpiStyle::Phone),
        _ => None,
    }
}

fn phone_style(s: &str) -> Option<PhoneStyle> {
    match s {
        "intl" => Some(PhoneStyle::Intl),
        "intl_hyphen" => Some(PhoneStyle::IntlHyphen),
        "zero" => Some(PhoneStyle::TrunkZero),
        "plain" => Some(PhoneStyle::Plain),
        "split" => Some(PhoneStyle::Split),
        _ => None,
    }
}

fn email_case(s: &str) -> Option<EmailCase> {
    match s {
        "lower" => Some(EmailCase::Lower),
        "title" => Some(EmailCase::Title),
        _ => None,
    }
}

fn name_style(s: &str) -> Option<NameStyle> {
    match s {
        "full" => Some(NameStyle::Full),
        "first" => Some(NameStyle::First),
        "upper" => Some(NameStyle::Upper),
        "deva" => Some(NameStyle::Deva),
        "deva_first" => Some(NameStyle::DevaFirst),
        _ => None,
    }
}

fn script(s: &str) -> Option<Script> {
    match s {
        "latin" => Some(Script::Latin),
        "deva" => Some(Script::Devanagari),
        _ => None,
    }
}

fn date_style(s: &str) -> Option<DateStyle> {
    match s {
        "slashed" => Some(DateStyle::Slashed),
        "hyphen" => Some(DateStyle::Hyphenated),
        "dotted" => Some(DateStyle::Dotted),
        "long" => Some(DateStyle::Long),
        "month_first" => Some(DateStyle::MonthFirst),
        "iso" => Some(DateStyle::Iso),
        "deva" => Some(DateStyle::Devanagari),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> Result<Template, TemplateError> {
        let src = format!(
            "[[template]]\nid = \"t\"\nlang = \"en\"\ndifficulty = \"easy\"\ntext = '''{text}'''\n"
        );
        parse_file(&src).map(|mut v| v.remove(0))
    }

    #[test]
    fn parses_fillers_styles_and_literal_braces() {
        let t = one("{{\"id\": \"{aadhaar:spaced}\"}} by {name}").unwrap();
        assert_eq!(
            t.parts,
            vec![
                Part::Text("{\"id\": \"".to_owned()),
                Part::Fill(Filler::Aadhaar(Some(Grouping::Spaced))),
                Part::Text("\"} by ".to_owned()),
                Part::Fill(Filler::Name(None)),
            ]
        );
    }

    #[test]
    fn rejects_unknown_filler_and_style() {
        assert!(one("{ssn}").is_err());
        assert!(one("{pan:spaced}").is_err());
        assert!(one("{passport:upper}").is_err());
    }

    #[test]
    fn rejects_decoy_mixed_with_entities() {
        assert!(one("PNR {decoy_pnr} for {name}").is_err());
        assert!(one("{decoy_pnr} {decoy_pan}").is_err());
    }

    #[test]
    fn rejects_unbalanced_braces() {
        assert!(one("oops } here").is_err());
        assert!(one("open {name forever").is_err());
    }

    #[test]
    fn hard_negative_reports_its_decoy() {
        let t = one("Your PNR is {decoy_pnr}.").unwrap();
        assert_eq!(t.decoy(), Some(Decoy::Pnr));
    }
}
