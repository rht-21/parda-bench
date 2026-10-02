from parda_bench.data.checksum import ascii_digits, verhoeff_check_digit, verhoeff_is_valid
from parda_bench.data.ids import Grouping, group_by_four, ungrouped
from parda_bench.data.rng import DataRng


def _with_check_digit(rng: DataRng, first_min: int, length: int) -> list[int]:
    digits = [rng.between(first_min, 9)] + [rng.below(10) for _ in range(length - 1)]
    return [*digits, verhoeff_check_digit(digits)]


def _text(digits: list[int]) -> str:
    return "".join(map(str, digits))


def aadhaar(rng: DataRng, grouping: Grouping) -> str:
    """12-digit Aadhaar: first digit 2-9, Verhoeff check digit last."""
    return group_by_four(_text(_with_check_digit(rng, 2, 11)), grouping)


def vid(rng: DataRng, grouping: Grouping) -> str:
    """16-digit Virtual ID with a Verhoeff check digit."""
    return group_by_four(_text(_with_check_digit(rng, 1, 15)), grouping)


def invalid_aadhaar(rng: DataRng, grouping: Grouping) -> str:
    """Aadhaar-shaped number whose check digit is wrong, for hard negatives."""
    digits = _with_check_digit(rng, 2, 11)
    digits[-1] = (digits[-1] + rng.between(1, 9)) % 10
    return group_by_four(_text(digits), grouping)


def is_valid_aadhaar(text: str) -> bool:
    d = ascii_digits(ungrouped(text))
    return d is not None and len(d) == 12 and d[0] >= 2 and verhoeff_is_valid(d)


def is_valid_vid(text: str) -> bool:
    d = ascii_digits(ungrouped(text))
    return d is not None and len(d) == 16 and verhoeff_is_valid(d)
