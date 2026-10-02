"""Template parsing. A template is text with `{kind}` or `{kind:style}` fillers; `{{` and `}}` are literal braces.
`{=ENTITY:text}` is a hand-labeled span and `{!ENTITY:text}` a literal decoy, for free-form samples.
A template holds either only real entities, or exactly one decoy (a hard negative)."""

import tomllib
from dataclasses import dataclass
from enum import Enum

from parda_bench.data.ids import Case, Grouping
from parda_bench.data.ids.contact import EmailCase, PhoneStyle
from parda_bench.data.ids.date import DateStyle
from parda_bench.spec.entity import EntityType
from parda_bench.spec.sample import Difficulty, Lang, Source


class TemplateError(ValueError):
    def __init__(self, template_id: str, message: str) -> None:
        super().__init__(f"template {template_id}: {message}")


class Script(Enum):
    LATIN = "latin"
    DEVANAGARI = "deva"


class NameStyle(Enum):
    FULL = "full"
    FIRST = "first"
    UPPER = "upper"
    """`RAHUL SHARMA`, as on forms and ID cards."""
    DEVA = "deva"
    DEVA_FIRST = "deva_first"


class UpiStyle(Enum):
    NAME = "name"
    PHONE = "phone"


_GROUPING = {"plain": Grouping.PLAIN, "spaced": Grouping.SPACED, "hyphen": Grouping.HYPHENATED}
_CASE = {"upper": Case.UPPER, "lower": Case.LOWER}
_STYLES: dict[str, dict[str, Enum]] = {
    "grouping": dict(_GROUPING),
    "case": dict(_CASE),
    "upi": {s.value: s for s in UpiStyle},
    "phone": {s.value: s for s in PhoneStyle},
    "email": {s.value: s for s in EmailCase},
    "name": {s.value: s for s in NameStyle},
    "script": {s.value: s for s in Script},
    "date": {s.value: s for s in DateStyle},
}

FILLERS: dict[str, tuple[EntityType, str | None]] = {
    "aadhaar": (EntityType.AADHAAR, "grouping"),
    "vid": (EntityType.AADHAAR_VID, "grouping"),
    "pan": (EntityType.PAN, "case"),
    "gstin": (EntityType.GSTIN, "case"),
    "ifsc": (EntityType.IFSC, "case"),
    "upi": (EntityType.UPI_ID, "upi"),
    "phone": (EntityType.PHONE, "phone"),
    "email": (EntityType.EMAIL, "email"),
    "card": (EntityType.PAYMENT_CARD, "grouping"),
    "passport": (EntityType.PASSPORT, None),
    "voter": (EntityType.VOTER_ID, None),
    "vehicle": (EntityType.VEHICLE_REGISTRATION, "grouping"),
    "name": (EntityType.PERSON_NAME, "name"),
    "other_name": (EntityType.PERSON_NAME, "name"),
    "address": (EntityType.ADDRESS, "script"),
    "dob": (EntityType.DATE_OF_BIRTH, "date"),
}
"""Real-entity fillers: kind to (entity, style family or `None` when the filler takes no style)."""

DECOYS: dict[str, tuple[EntityType, str | None]] = {
    "decoy_aadhaar": (EntityType.AADHAAR, "grouping"),
    "decoy_card": (EntityType.PAYMENT_CARD, "grouping"),
    "decoy_pan": (EntityType.PAN, None),
    "decoy_pnr": (EntityType.PHONE, None),
    "decoy_date": (EntityType.DATE_OF_BIRTH, "date"),
    "decoy_place": (EntityType.PERSON_NAME, None),
    "decoy_passport": (EntityType.PASSPORT, None),
    "decoy_gstin": (EntityType.GSTIN, None),
    "decoy_ifsc": (EntityType.IFSC, None),
    "decoy_handle": (EntityType.UPI_ID, None),
    "decoy_service_account": (EntityType.EMAIL, None),
}
"""Look-alikes that must not be flagged, rendered by `render.decoy`."""


@dataclass(frozen=True)
class Text:
    text: str


@dataclass(frozen=True)
class Fill:
    kind: str
    style: Enum | None


@dataclass(frozen=True)
class Decoy:
    kind: str
    style: Enum | None


@dataclass(frozen=True)
class Labeled:
    """Fixed text labeled as `entity`."""

    entity: EntityType
    text: str


@dataclass(frozen=True)
class LiteralDecoy:
    """Fixed text that resembles `entity` but is not personal data."""

    entity: EntityType
    text: str


Part = Text | Fill | Decoy | Labeled | LiteralDecoy


@dataclass(frozen=True)
class Template:
    id: str
    lang: Lang
    difficulty: Difficulty
    source: Source
    """`template` entries render many times; any other source is one fixed text, rendered once."""
    noisy: bool
    parts: tuple[Part, ...]

    def decoy_entity(self) -> EntityType | None:
        """The entity type a hard negative imitates, `None` for a positive template."""
        for part in self.parts:
            if isinstance(part, Decoy):
                return DECOYS[part.kind][0]
            if isinstance(part, LiteralDecoy):
                return part.entity
        return None


_FIELDS = {"id", "lang", "difficulty", "text", "noisy"}


def parse_file(source: str) -> list[Template]:
    """Parses a TOML file of `[[template]]` tables (or a `template = [...]` array) with an optional `source`.

    Raises `TemplateError` on invalid TOML, unknown fields, fillers or styles, unbalanced braces, or a decoy
    mixed with other fillers.
    """
    try:
        doc = tomllib.loads(source)
    except tomllib.TOMLDecodeError as e:
        raise TemplateError("<file>", str(e)) from e
    unknown = set(doc) - {"source", "template"}
    if unknown:
        raise TemplateError("<file>", f"unknown keys {sorted(unknown)}")
    file_source = Source(doc.get("source", "template"))
    return [_parse_def(d, file_source) for d in doc.get("template", [])]


def _parse_def(definition: dict[str, object], source: Source) -> Template:
    template_id = str(definition.get("id", "<missing id>"))
    unknown = set(definition) - _FIELDS
    if unknown:
        raise TemplateError(template_id, f"unknown fields {sorted(unknown)}")
    try:
        parts = tuple(_parse_text(str(definition["text"])))
        lang, difficulty = Lang(str(definition["lang"])), Difficulty(str(definition["difficulty"]))
    except (KeyError, ValueError) as e:
        raise TemplateError(template_id, str(e)) from e
    decoys = sum(isinstance(p, Decoy | LiteralDecoy) for p in parts)
    fills = sum(isinstance(p, Fill | Labeled) for p in parts)
    if decoys > 1 or (decoys == 1 and fills > 0):
        raise TemplateError(template_id, "a hard negative holds exactly one decoy and no other fillers")
    noisy = definition.get("noisy", False)
    if not isinstance(noisy, bool):
        raise TemplateError(template_id, "`noisy` must be true or false")
    return Template(template_id, lang, difficulty, source, noisy, parts)


def _parse_text(text: str) -> list[Part]:
    parts: list[Part] = []
    literal: list[str] = []
    i = 0
    while i < len(text):
        c = text[i]
        if text.startswith("{{", i) or text.startswith("}}", i):
            literal.append(c)
            i += 2
        elif c == "{":
            end = text.find("}", i)
            if end < 0:
                raise ValueError(f"unclosed `{text[i:]}`")
            if literal:
                parts.append(Text("".join(literal)))
                literal = []
            parts.append(_parse_slot(text[i + 1 : end]))
            i = end + 1
        elif c == "}":
            raise ValueError("unmatched `}`; write `}}` for a literal brace")
        else:
            literal.append(c)
            i += 1
    if literal:
        parts.append(Text("".join(literal)))
    return parts


def _parse_slot(spec: str) -> Part:
    if spec.startswith("="):
        return Labeled(*_literal_span(spec[1:]))
    if spec.startswith("!"):
        return LiteralDecoy(*_literal_span(spec[1:]))
    kind, _, style_name = spec.partition(":")
    table, cls = (FILLERS, Fill) if kind in FILLERS else (DECOYS, Decoy)
    if kind not in table:
        raise ValueError(f"unknown filler `{kind}`")
    family = table[kind][1]
    if not style_name:
        return cls(kind, None)
    if family is None or style_name not in _STYLES[family]:
        raise ValueError(f"unknown style `{style_name}` for `{kind}`")
    return cls(kind, _STYLES[family][style_name])


def _literal_span(spec: str) -> tuple[EntityType, str]:
    """`ENTITY:text` with `ENTITY` a wire name such as `PERSON_NAME`."""
    name, colon, text = spec.partition(":")
    if not colon:
        raise ValueError(f"`{spec}` should be ENTITY:text")
    try:
        entity = EntityType(name)
    except ValueError as e:
        raise ValueError(f"unknown entity type `{name}`") from e
    if not text or text.strip() != text:
        raise ValueError(f"labeled text `{text}` is empty or padded with spaces")
    return entity, text
