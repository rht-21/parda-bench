//! Renders a template into text and gold spans, with one consistent persona per sample.

use parda_spec::sample::{Lang, Span};
use rand::RngExt;

use crate::DataRng;
use crate::ids::contact::{EmailCase, PhoneStyle, format_phone, mobile_digits};
use crate::ids::date::{Date, DateStyle};
use crate::ids::{Case, Grouping, aadhaar, banking, card, contact, documents, pick, tax};
use crate::lexicon::{
    BUILDINGS, Bilingual, CITIES, FIRST_NAMES, LANDMARKS, LOCALITIES, NAMED_PLACES, SURNAMES,
};
use crate::template::{Decoy, Filler, NameStyle, Part, Script, Template, UpiStyle};

const DOB_YEARS: std::ops::RangeInclusive<u16> = 1950..=2006;
const EVENT_YEARS: std::ops::RangeInclusive<u16> = 2023..=2026;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    /// Spans of real entities, in order; empty for hard negatives.
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy)]
struct Person {
    first: Bilingual,
    last: Bilingual,
}

impl Person {
    fn random(rng: &mut DataRng) -> Self {
        Self {
            first: pick(rng, &FIRST_NAMES),
            last: pick(rng, &SURNAMES),
        }
    }

    fn name(self, style: NameStyle) -> String {
        match style {
            NameStyle::Full => format!("{} {}", self.first.latin, self.last.latin),
            NameStyle::First => self.first.latin.to_owned(),
            NameStyle::Upper => format!("{} {}", self.first.latin, self.last.latin).to_uppercase(),
            NameStyle::Deva => format!("{} {}", self.first.deva, self.last.deva),
            NameStyle::DevaFirst => self.first.deva.to_owned(),
        }
    }
}

/// The sample's main person, a second person, and the main person's mobile number.
struct Persona {
    main: Person,
    other: Person,
    mobile: String,
}

#[must_use]
pub fn render(template: &Template, rng: &mut DataRng) -> Rendered {
    let persona = Persona {
        main: Person::random(rng),
        other: Person::random(rng),
        mobile: mobile_digits(rng),
    };
    let mut text = String::new();
    let mut chars = 0;
    let mut spans = Vec::new();
    for part in &template.parts {
        let piece = match part {
            Part::Text(t) => t.clone(),
            Part::Fill(f) => fill(*f, template.lang, &persona, rng),
            Part::Decoy(d) => decoy(*d, rng),
        };
        let len = piece.chars().count();
        if let Part::Fill(f) = part {
            spans.push(Span {
                start: chars,
                end: chars + len,
                entity: f.entity(),
            });
        }
        chars += len;
        text.push_str(&piece);
    }
    Rendered { text, spans }
}

fn fill(filler: Filler, lang: Lang, persona: &Persona, rng: &mut DataRng) -> String {
    let deva = lang == Lang::HiDeva;
    match filler {
        Filler::Aadhaar(g) => {
            let choice = or_pick(rng, g, &Grouping::ALL);
            aadhaar::aadhaar(rng, choice)
        }
        Filler::AadhaarVid(g) => {
            let choice = or_pick(rng, g, &Grouping::ALL);
            aadhaar::vid(rng, choice)
        }
        Filler::Pan(c) => {
            let initial = persona.main.last.latin.chars().next().unwrap_or('A');
            tax::pan(rng, initial, c.unwrap_or(Case::Upper))
        }
        Filler::Gstin(c) => tax::gstin(rng, c.unwrap_or(Case::Upper)),
        Filler::Ifsc(c) => banking::ifsc(rng, c.unwrap_or(Case::Upper)),
        Filler::Upi(style) => {
            let local = match or_pick(rng, style, &[UpiStyle::Name, UpiStyle::Phone]) {
                UpiStyle::Name => banking::upi_local_from_name(
                    rng,
                    persona.main.first.latin,
                    persona.main.last.latin,
                ),
                UpiStyle::Phone => persona.mobile.clone(),
            };
            banking::upi(rng, &local)
        }
        Filler::Phone(style) => {
            format_phone(&persona.mobile, or_pick(rng, style, &PhoneStyle::ALL))
        }
        Filler::Email(c) => contact::email(
            rng,
            persona.main.first.latin,
            persona.main.last.latin,
            c.unwrap_or(EmailCase::Lower),
        ),
        Filler::Card(g) => {
            let choice = or_pick(rng, g, &Grouping::ALL);
            card::card(rng, choice)
        }
        Filler::Passport => documents::passport(rng),
        Filler::VoterId => documents::voter_id(rng),
        Filler::Vehicle(g) => {
            let choice = or_pick(rng, g, &Grouping::ALL);
            documents::vehicle(rng, choice)
        }
        Filler::Name(s) => persona.main.name(s.unwrap_or(if deva {
            NameStyle::Deva
        } else {
            NameStyle::Full
        })),
        Filler::OtherName(s) => persona.other.name(s.unwrap_or(if deva {
            NameStyle::Deva
        } else {
            NameStyle::Full
        })),
        Filler::Address(s) => address(
            rng,
            s.unwrap_or(if deva {
                Script::Devanagari
            } else {
                Script::Latin
            }),
        ),
        Filler::Dob(s) => {
            let style = s.unwrap_or_else(|| {
                if deva {
                    DateStyle::Devanagari
                } else {
                    pick(rng, &DateStyle::LATIN)
                }
            });
            Date::random(rng, DOB_YEARS).format(style)
        }
    }
}

fn decoy(decoy: Decoy, rng: &mut DataRng) -> String {
    let short_groupings = [Grouping::Plain, Grouping::Spaced];
    match decoy {
        Decoy::Aadhaar(g) => {
            let choice = or_pick(rng, g, &short_groupings);
            aadhaar::invalid_aadhaar(rng, choice)
        }
        Decoy::Card(g) => {
            let choice = or_pick(rng, g, &short_groupings);
            card::invalid_card(rng, choice)
        }
        Decoy::Pan => tax::invalid_pan(rng),
        Decoy::Pnr => contact::pnr(rng),
        Decoy::Date(s) => {
            let style = s.unwrap_or_else(|| pick(rng, &DateStyle::LATIN));
            Date::random(rng, EVENT_YEARS).format(style)
        }
        Decoy::NamedPlace => pick(rng, &NAMED_PLACES).to_owned(),
        Decoy::Passport => documents::passport(rng),
    }
}

/// The template's explicit choice, or a random one of `options`.
fn or_pick<T: Copy>(rng: &mut DataRng, chosen: Option<T>, options: &[T]) -> T {
    chosen.unwrap_or_else(|| pick(rng, options))
}

fn address(rng: &mut DataRng, script: Script) -> String {
    let city = pick(rng, &CITIES);
    let locality = pick(rng, &LOCALITIES);
    let building = pick(rng, &BUILDINGS);
    let landmark = pick(rng, &LANDMARKS);
    let pin = format!("{}{:03}", city.pin_prefix, rng.random_range(1..100));
    let (n, m) = (rng.random_range(1..400), rng.random_range(1..60));
    match (script, rng.random_range(0..3)) {
        (Script::Latin, 0) => format!(
            "Flat {n}, {}, {}, {}, {} {pin}",
            building.latin, locality.latin, city.name.latin, city.state.latin
        ),
        (Script::Latin, 1) => format!("{n}/{m}, {}, {} - {pin}", locality.latin, city.name.latin),
        (Script::Latin, _) => format!(
            "House No. {n}, {}, Near {}, {}, {} - {pin}",
            locality.latin, landmark.latin, city.name.latin, city.state.latin
        ),
        (Script::Devanagari, 0) => {
            format!(
                "मकान नं. {n}, {}, {}, {} - {pin}",
                locality.deva, city.name.deva, city.state.deva
            )
        }
        (Script::Devanagari, 1) => {
            format!(
                "फ्लैट {n}, {}, {}, {} {pin}",
                building.deva, locality.deva, city.name.deva
            )
        }
        (Script::Devanagari, _) => {
            format!(
                "{n}/{m}, {}, {} के पास, {} - {pin}",
                locality.deva, landmark.deva, city.name.deva
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use parda_spec::entity::EntityType;
    use parda_spec::sample::Difficulty;
    use rand::SeedableRng;

    use super::*;

    fn template(parts: Vec<Part>, lang: Lang) -> Template {
        Template {
            id: "t".to_owned(),
            lang,
            difficulty: Difficulty::Easy,
            parts,
        }
    }

    fn span_text(r: &Rendered, i: usize) -> String {
        r.text
            .chars()
            .skip(r.spans[i].start)
            .take(r.spans[i].end - r.spans[i].start)
            .collect()
    }

    #[test]
    fn spans_count_chars_not_bytes_after_devanagari() {
        let t = template(
            vec![
                Part::Text("मेरा आधार ".to_owned()),
                Part::Fill(Filler::Aadhaar(Some(Grouping::Plain))),
            ],
            Lang::HiDeva,
        );
        let r = render(&t, &mut DataRng::seed_from_u64(1));
        assert_eq!(r.spans[0].start, 10);
        assert!(crate::ids::aadhaar::is_valid_aadhaar(&span_text(&r, 0)));
    }

    #[test]
    fn phone_and_phone_upi_share_the_persona_number() {
        let t = template(
            vec![
                Part::Fill(Filler::Phone(Some(PhoneStyle::Plain))),
                Part::Text(" / ".to_owned()),
                Part::Fill(Filler::Upi(Some(UpiStyle::Phone))),
            ],
            Lang::En,
        );
        let r = render(&t, &mut DataRng::seed_from_u64(2));
        assert!(span_text(&r, 1).starts_with(&span_text(&r, 0)));
    }

    #[test]
    fn devanagari_template_defaults_names_to_devanagari() {
        let t = template(vec![Part::Fill(Filler::Name(None))], Lang::HiDeva);
        let r = render(&t, &mut DataRng::seed_from_u64(3));
        assert!(
            r.text
                .chars()
                .any(|c| ('\u{0900}'..='\u{097F}').contains(&c)),
            "{}",
            r.text
        );
        assert_eq!(r.spans[0].entity, EntityType::PersonName);
    }

    #[test]
    fn decoys_produce_no_spans() {
        let t = template(
            vec![Part::Text("PNR ".to_owned()), Part::Decoy(Decoy::Pnr)],
            Lang::En,
        );
        let r = render(&t, &mut DataRng::seed_from_u64(4));
        assert!(r.spans.is_empty());
        assert_eq!(r.text.chars().count(), 14);
    }
}
