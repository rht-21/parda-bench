from parda_bench.data.checksum import ascii_digits, luhn_check_digit, luhn_is_valid
from parda_bench.data.ids import Grouping, group_by_four, ungrouped
from parda_bench.data.rng import DataRng

PREFIXES = ([6, 0], [6, 5], [8, 1], [5, 0, 8], [4], [5, 1], [5, 4], [2, 2, 2, 1])
"""Issuer prefixes: RuPay, Visa, Mastercard."""


def _valid_digits(rng: DataRng) -> list[int]:
    digits = list(rng.pick(PREFIXES))
    digits += [rng.below(10) for _ in range(15 - len(digits))]
    return [*digits, luhn_check_digit(digits)]


def card(rng: DataRng, grouping: Grouping) -> str:
    """16-digit payment card number with a valid Luhn check digit."""
    return group_by_four("".join(map(str, _valid_digits(rng))), grouping)


def invalid_card(rng: DataRng, grouping: Grouping) -> str:
    """Card-shaped number that fails Luhn, for hard negatives."""
    digits = _valid_digits(rng)
    digits[-1] = (digits[-1] + rng.between(1, 9)) % 10
    return group_by_four("".join(map(str, digits)), grouping)


def is_valid_card(text: str) -> bool:
    d = ascii_digits(ungrouped(text))
    return d is not None and 13 <= len(d) <= 19 and luhn_is_valid(d)
