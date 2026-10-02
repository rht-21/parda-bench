"""Generators and structural validators for each identifier type. Validators accept the formatted forms the
generators emit (spaces, hyphens, any letter case) so they can check gold spans taken verbatim from text."""

from enum import Enum

from parda_bench.spec.entity import EntityType


class Grouping(Enum):
    """How a long digit string is broken up when written."""

    PLAIN = ""
    SPACED = " "
    HYPHENATED = "-"


class Case(Enum):
    UPPER = "upper"
    LOWER = "lower"

    def apply(self, s: str) -> str:
        return s.upper() if self is Case.UPPER else s.lower()


GROUPINGS = (Grouping.PLAIN, Grouping.SPACED, Grouping.HYPHENATED)


def group_by_four(digits: str, grouping: Grouping) -> str:
    return grouping.value.join(digits[i : i + 4] for i in range(0, len(digits), 4))


def ungrouped(text: str) -> str:
    return text.replace(" ", "").replace("-", "")


def is_upper_alpha(s: str) -> bool:
    return s.isascii() and s.isalpha() and s.isupper()


def is_digits(s: str) -> bool:
    return s.isascii() and s.isdigit()


def is_valid(entity: EntityType, text: str) -> bool | None:
    """Structural validity of `text` as `entity`; `None` for types without a checkable structure."""
    from parda_bench.data.ids import aadhaar, banking, card, contact, documents, tax

    validators = {
        EntityType.AADHAAR: aadhaar.is_valid_aadhaar,
        EntityType.AADHAAR_VID: aadhaar.is_valid_vid,
        EntityType.PAN: tax.is_valid_pan,
        EntityType.GSTIN: tax.is_valid_gstin,
        EntityType.IFSC: banking.is_valid_ifsc,
        EntityType.UPI_ID: banking.is_valid_upi,
        EntityType.PHONE: contact.is_valid_phone,
        EntityType.EMAIL: contact.is_valid_email,
        EntityType.PAYMENT_CARD: card.is_valid_card,
        EntityType.PASSPORT: documents.is_valid_passport,
        EntityType.VOTER_ID: documents.is_valid_voter_id,
        EntityType.VEHICLE_REGISTRATION: documents.is_valid_vehicle,
    }
    validator = validators.get(entity)
    return None if validator is None else validator(text)
