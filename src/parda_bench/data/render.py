"""Renders a template into text and gold spans, with one consistent persona per sample."""

from collections.abc import Sequence
from dataclasses import dataclass
from enum import Enum
from typing import TypeVar

from parda_bench.data.ids import GROUPINGS, Case, Grouping, aadhaar, banking, card, contact, documents, tax
from parda_bench.data.ids.contact import EmailCase, PhoneStyle
from parda_bench.data.ids.date import LATIN_STYLES, Date, DateStyle
from parda_bench.data.lexicon import (
    BUILDINGS,
    CITIES,
    FIRST_NAMES,
    LANDMARKS,
    LOCALITIES,
    NAMED_PLACES,
    SURNAMES,
    Bilingual,
)
from parda_bench.data.noise import add_noise
from parda_bench.data.rng import DataRng
from parda_bench.data.template import (
    DECOYS,
    FILLERS,
    Decoy,
    Fill,
    Labeled,
    LiteralDecoy,
    NameStyle,
    Script,
    Template,
    Text,
    UpiStyle,
)
from parda_bench.spec.sample import Lang, Span

S = TypeVar("S", bound=Enum)

DOB_YEARS = (1950, 2006)
EVENT_YEARS = (2023, 2026)


@dataclass(frozen=True)
class Rendered:
    text: str
    spans: list[Span]
    """Spans of real entities, in order; empty for hard negatives."""


@dataclass(frozen=True)
class Person:
    first: Bilingual
    last: Bilingual

    @staticmethod
    def random(rng: DataRng) -> "Person":
        return Person(rng.pick(FIRST_NAMES), rng.pick(SURNAMES))

    def name(self, style: NameStyle) -> str:
        return {
            NameStyle.FULL: f"{self.first.latin} {self.last.latin}",
            NameStyle.FIRST: self.first.latin,
            NameStyle.UPPER: f"{self.first.latin} {self.last.latin}".upper(),
            NameStyle.DEVA: f"{self.first.deva} {self.last.deva}",
            NameStyle.DEVA_FIRST: self.first.deva,
        }[style]


@dataclass(frozen=True)
class Persona:
    """The sample's main person, a second person, and the main person's mobile number."""

    main: Person
    other: Person
    mobile: str


def render(template: Template, rng: DataRng) -> Rendered:
    persona = Persona(Person.random(rng), Person.random(rng), contact.mobile_digits(rng))
    pieces: list[str] = []
    spans: list[Span] = []
    offset = 0
    for part in template.parts:
        if isinstance(part, Text):
            piece = add_noise(part.text, rng) if template.noisy else part.text
        elif isinstance(part, Labeled | LiteralDecoy):
            piece = part.text
        elif isinstance(part, Fill):
            piece = fill(part, template.lang, persona, rng)
        else:
            piece = decoy(part, rng)
        if isinstance(part, Fill | Labeled):
            entity = FILLERS[part.kind][0] if isinstance(part, Fill) else part.entity
            spans.append(Span(start=offset, end=offset + len(piece), entity=entity))
        offset += len(piece)
        pieces.append(piece)
    return Rendered("".join(pieces), spans)


def _style(rng: DataRng, chosen: Enum | None, options: Sequence[S]) -> S:
    """The template's explicit choice (which must be one of `options`), or a random one of them."""
    if chosen is None:
        return rng.pick(options)
    for option in options:
        if option is chosen:
            return option
    raise AssertionError(f"style {chosen} is not one of {list(options)}")


def fill(part: Fill, lang: Lang, persona: Persona, rng: DataRng) -> str:
    deva = lang is Lang.HI_DEVA
    kind, style = part.kind, part.style
    if kind == "aadhaar":
        return aadhaar.aadhaar(rng, _style(rng, style, GROUPINGS))
    if kind == "vid":
        return aadhaar.vid(rng, _style(rng, style, GROUPINGS))
    if kind == "pan":
        return tax.pan(rng, persona.main.last.latin[0], _style(rng, style or Case.UPPER, [Case.UPPER, Case.LOWER]))
    if kind == "gstin":
        return tax.gstin(rng, _style(rng, style or Case.UPPER, [Case.UPPER, Case.LOWER]))
    if kind == "ifsc":
        return banking.ifsc(rng, _style(rng, style or Case.UPPER, [Case.UPPER, Case.LOWER]))
    if kind == "upi":
        upi_style = _style(rng, style, [UpiStyle.NAME, UpiStyle.PHONE])
        local = (
            banking.upi_local_from_name(rng, persona.main.first.latin, persona.main.last.latin)
            if upi_style is UpiStyle.NAME
            else persona.mobile
        )
        return banking.upi(rng, local)
    if kind == "phone":
        return contact.format_phone(persona.mobile, _style(rng, style, list(PhoneStyle)))
    if kind == "email":
        email_case = _style(rng, style or EmailCase.LOWER, list(EmailCase))
        return contact.email(rng, persona.main.first.latin, persona.main.last.latin, email_case)
    if kind == "card":
        return card.card(rng, _style(rng, style, GROUPINGS))
    if kind == "passport":
        return documents.passport(rng)
    if kind == "voter":
        return documents.voter_id(rng)
    if kind == "vehicle":
        return documents.vehicle(rng, _style(rng, style, GROUPINGS))
    if kind in ("name", "other_name"):
        person = persona.main if kind == "name" else persona.other
        default = NameStyle.DEVA if deva else NameStyle.FULL
        return person.name(_style(rng, style or default, list(NameStyle)))
    if kind == "address":
        default_script = Script.DEVANAGARI if deva else Script.LATIN
        return address(rng, _style(rng, style or default_script, list(Script)))
    if kind == "dob":
        if style is None and deva:
            style = DateStyle.DEVANAGARI
        options = list(DateStyle) if style is not None else LATIN_STYLES
        return Date.random(rng, *DOB_YEARS).format(_style(rng, style, options))
    raise AssertionError(f"unhandled filler {kind}")


def decoy(part: Decoy, rng: DataRng) -> str:
    kind, style = part.kind, part.style
    short = [Grouping.PLAIN, Grouping.SPACED]
    if kind == "decoy_aadhaar":
        return aadhaar.invalid_aadhaar(rng, _style(rng, style, GROUPINGS if style else short))
    if kind == "decoy_card":
        return card.invalid_card(rng, _style(rng, style, GROUPINGS if style else short))
    if kind == "decoy_pan":
        return tax.invalid_pan(rng)
    if kind == "decoy_pnr":
        return contact.pnr(rng)
    if kind == "decoy_date":
        options = list(DateStyle) if style is not None else LATIN_STYLES
        return Date.random(rng, *EVENT_YEARS).format(_style(rng, style, options))
    if kind == "decoy_place":
        return rng.pick(NAMED_PLACES)
    if kind == "decoy_passport":
        return documents.passport(rng)
    if kind == "decoy_gstin":
        return tax.invalid_gstin(rng)
    if kind == "decoy_ifsc":
        return banking.ifsc_like_code(rng)
    if kind == "decoy_handle":
        person = Person.random(rng)
        return contact.social_handle(rng, person.first.latin, person.last.latin)
    if kind == "decoy_service_account":
        return contact.service_account(rng)
    raise AssertionError(f"unhandled decoy {kind}; known: {sorted(DECOYS)}")


def address(rng: DataRng, script: Script) -> str:
    city = rng.pick(CITIES)
    locality, building, landmark = rng.pick(LOCALITIES), rng.pick(BUILDINGS), rng.pick(LANDMARKS)
    pin = f"{city.pin_prefix}{rng.between(1, 99):03}"
    n, m = rng.between(1, 399), rng.between(1, 59)
    variant = rng.below(3)
    if script is Script.LATIN:
        return (
            f"Flat {n}, {building.latin}, {locality.latin}, {city.name.latin}, {city.state.latin} {pin}",
            f"{n}/{m}, {locality.latin}, {city.name.latin} - {pin}",
            f"House No. {n}, {locality.latin}, Near {landmark.latin}, {city.name.latin}, {city.state.latin} - {pin}",
        )[variant]
    return (
        f"मकान नं. {n}, {locality.deva}, {city.name.deva}, {city.state.deva} - {pin}",
        f"फ्लैट {n}, {building.deva}, {locality.deva}, {city.name.deva} {pin}",
        f"{n}/{m}, {locality.deva}, {landmark.deva} के पास, {city.name.deva} - {pin}",
    )[variant]
