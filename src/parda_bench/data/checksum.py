"""Check-digit schemes used by Indian identifiers: Verhoeff (Aadhaar, VID), Luhn (payment cards), GSTIN mod 36."""

_D = (
    (0, 1, 2, 3, 4, 5, 6, 7, 8, 9),
    (1, 2, 3, 4, 0, 6, 7, 8, 9, 5),
    (2, 3, 4, 0, 1, 7, 8, 9, 5, 6),
    (3, 4, 0, 1, 2, 8, 9, 5, 6, 7),
    (4, 0, 1, 2, 3, 9, 5, 6, 7, 8),
    (5, 9, 8, 7, 6, 0, 4, 3, 2, 1),
    (6, 5, 9, 8, 7, 1, 0, 4, 3, 2),
    (7, 6, 5, 9, 8, 2, 1, 0, 4, 3),
    (8, 7, 6, 5, 9, 3, 2, 1, 0, 4),
    (9, 8, 7, 6, 5, 4, 3, 2, 1, 0),
)
_P = (
    (0, 1, 2, 3, 4, 5, 6, 7, 8, 9),
    (1, 5, 7, 6, 2, 8, 3, 0, 9, 4),
    (5, 8, 0, 3, 7, 9, 6, 1, 4, 2),
    (8, 9, 1, 6, 0, 4, 3, 5, 2, 7),
    (9, 4, 5, 3, 1, 2, 6, 8, 7, 0),
    (4, 2, 8, 6, 5, 7, 3, 9, 0, 1),
    (2, 7, 9, 3, 8, 0, 6, 4, 1, 5),
    (7, 0, 4, 6, 9, 1, 3, 2, 5, 8),
)
_INV = (0, 4, 3, 2, 1, 5, 6, 7, 8, 9)
GSTIN_CHARSET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"


def _verhoeff_fold(digits: list[int], offset: int) -> int:
    c = 0
    for i, d in enumerate(reversed(digits)):
        c = _D[c][_P[(i + offset) % 8][d]]
    return c


def verhoeff_check_digit(digits: list[int]) -> int:
    return _INV[_verhoeff_fold(digits, 1)]


def verhoeff_is_valid(digits: list[int]) -> bool:
    return bool(digits) and _verhoeff_fold(digits, 0) == 0


def _luhn_sum(digits: list[int], offset: int) -> int:
    total = 0
    for i, d in enumerate(reversed(digits)):
        if (i + offset) % 2 == 1:
            d *= 2
            if d > 9:
                d -= 9
        total += d
    return total


def luhn_check_digit(digits: list[int]) -> int:
    return (10 - _luhn_sum(digits, 1) % 10) % 10


def luhn_is_valid(digits: list[int]) -> bool:
    return bool(digits) and _luhn_sum(digits, 0) % 10 == 0


def gstin_check_char(first14: str) -> str | None:
    """Check character for the first 14 characters; `None` if any is outside `0-9A-Z`."""
    total = 0
    for i, ch in enumerate(first14):
        value = GSTIN_CHARSET.find(ch)
        if value < 0:
            return None
        product = value * (2 if i % 2 else 1)
        total += product // 36 + product % 36
    return GSTIN_CHARSET[(36 - total % 36) % 36]


def ascii_digits(s: str) -> list[int] | None:
    """Digits of `s` as integers; `None` if `s` holds anything but ASCII digits."""
    if not s.isascii() or not s.isdigit():
        return None if s else []
    return [int(c) for c in s]
