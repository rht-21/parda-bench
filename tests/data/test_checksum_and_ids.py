from parda_bench.data.checksum import (
    ascii_digits,
    gstin_check_char,
    luhn_check_digit,
    luhn_is_valid,
    verhoeff_check_digit,
    verhoeff_is_valid,
)
from parda_bench.data.ids import (
    GROUPINGS,
    Case,
    Grouping,
    aadhaar,
    banking,
    card,
    contact,
    documents,
    is_valid,
    tax,
)
from parda_bench.data.ids.contact import EmailCase, PhoneStyle
from parda_bench.data.ids.date import Date, DateStyle, days_in_month
from parda_bench.data.rng import DataRng
from parda_bench.spec.entity import EntityType


def digits(s: str) -> list[int]:
    d = ascii_digits(s)
    assert d is not None
    return d


def test_verhoeff_matches_reference_example() -> None:
    assert verhoeff_check_digit(digits("236")) == 3
    assert verhoeff_is_valid(digits("2363"))


def test_verhoeff_rejects_single_digit_error_and_adjacent_swap() -> None:
    assert not verhoeff_is_valid(digits("2364"))
    assert not verhoeff_is_valid(digits("3263"))


def test_luhn_matches_reference_numbers() -> None:
    assert luhn_is_valid(digits("4111111111111111"))
    assert luhn_check_digit(digits("411111111111111")) == 1
    assert luhn_check_digit(digits("7992739871")) == 3
    assert not luhn_is_valid(digits("4111111111111112"))


def test_gstin_matches_published_example() -> None:
    assert gstin_check_char("27AAPFU0939F1Z") == "V"
    assert gstin_check_char("27aapfu0939f1z") is None


def test_ascii_digits_rejects_non_digits() -> None:
    assert ascii_digits("12a") is None
    assert ascii_digits("१२") is None


def test_generated_identifiers_validate_in_every_style() -> None:
    rng = DataRng(1)
    for _ in range(200):
        for g in GROUPINGS:
            assert aadhaar.is_valid_aadhaar(aadhaar.aadhaar(rng, g))
            assert aadhaar.is_valid_vid(aadhaar.vid(rng, g))
            assert card.is_valid_card(card.card(rng, g))
            assert documents.is_valid_vehicle(documents.vehicle(rng, g))
        for case in Case:
            assert tax.is_valid_pan(tax.pan(rng, "S", case))
            assert tax.is_valid_gstin(tax.gstin(rng, case))
            assert banking.is_valid_ifsc(banking.ifsc(rng, case))
        for style in PhoneStyle:
            assert contact.is_valid_phone(contact.format_phone(contact.mobile_digits(rng), style))
        for email_case in EmailCase:
            assert contact.is_valid_email(contact.email(rng, "Ananya", "D'Souza", email_case))
        assert banking.is_valid_upi(banking.upi(rng, banking.upi_local_from_name(rng, "Joseph", "D'Souza")))
        assert documents.is_valid_passport(documents.passport(rng))
        assert documents.is_valid_voter_id(documents.voter_id(rng))


def test_decoys_never_validate_as_the_real_thing() -> None:
    rng = DataRng(2)
    for _ in range(300):
        assert not aadhaar.is_valid_aadhaar(aadhaar.invalid_aadhaar(rng, Grouping.PLAIN))
        assert not card.is_valid_card(card.invalid_card(rng, Grouping.SPACED))
        assert not tax.is_valid_pan(tax.invalid_pan(rng))
        assert not tax.is_valid_gstin(tax.invalid_gstin(rng))
        code = banking.ifsc_like_code(rng)
        assert banking.is_valid_ifsc(code) and code[:4] not in banking.BANK_CODES
        handle = contact.social_handle(rng, "Ananya", "Iyer")
        assert handle.startswith("@") and not banking.is_valid_upi(handle)
    for account in contact.SERVICE_ACCOUNTS:
        assert not contact.is_valid_email(account)


def test_pan_encodes_holder_type_and_surname_initial() -> None:
    assert tax.pan(DataRng(3), "s", Case.UPPER)[3:5] == "PS"


def test_structural_edge_cases() -> None:
    assert tax.is_valid_gstin("27AAPFU0939F1ZV") and not tax.is_valid_gstin("27AAPFU0939F1ZW")
    assert not tax.is_valid_pan("ABCPक1234A")
    assert not documents.is_valid_passport("Q1234567") and documents.is_valid_passport("J8369854")
    assert not contact.is_valid_phone("2234567890")
    assert not banking.is_valid_upi("rahul@gmail.com")
    assert aadhaar.is_valid_aadhaar("2345 6789 0124") and not aadhaar.is_valid_aadhaar("2345 6789 0125")
    assert is_valid(EntityType.PERSON_NAME, "anything") is None


def test_dates() -> None:
    assert (days_in_month(2000, 2), days_in_month(1900, 2), days_in_month(1996, 2)) == (29, 28, 29)
    d = Date(1991, 3, 7)
    assert d.format(DateStyle.SLASHED) == "07/03/1991"
    assert d.format(DateStyle.MONTH_FIRST) == "March 7, 1991"
    assert d.format(DateStyle.DEVANAGARI) == "7 मार्च 1991"
    rng = DataRng(4)
    for _ in range(500):
        r = Date.random(rng, 1950, 2006)
        assert 1950 <= r.year <= 2006 and 1 <= r.day <= days_in_month(r.year, r.month)
