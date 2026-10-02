import string

from parda_bench.data.ids import Grouping, is_digits, is_upper_alpha, ungrouped
from parda_bench.data.rng import DataRng

PASSPORT_LETTERS = "ABCDEFGHIJKLMNOPRSTUVWY"
"""Passport series letters; Q, X and Z are not issued."""
VEHICLE_STATES = ["MH", "DL", "KA", "TN", "UP", "GJ", "RJ", "WB", "TS", "KL", "HR", "PB", "MP", "AP", "BR"]


def _digits(rng: DataRng, n: int) -> str:
    return "".join(str(rng.below(10)) for _ in range(n))


def passport(rng: DataRng) -> str:
    """Passport number: series letter, then 7 digits with non-zero first and last digit."""
    return f"{rng.pick(PASSPORT_LETTERS)}{rng.between(1, 9)}{_digits(rng, 5)}{rng.between(1, 9)}"


def voter_id(rng: DataRng) -> str:
    """Voter ID (EPIC): 3 letters, 7 digits."""
    letters = "".join(rng.pick(string.ascii_uppercase) for _ in range(3))
    return letters + _digits(rng, 7)


def vehicle(rng: DataRng, grouping: Grouping) -> str:
    """Vehicle registration: state, RTO number, 1-2 series letters, 4-digit number."""
    state = rng.pick(VEHICLE_STATES)
    rto = f"{rng.between(1, 99):02}"
    series = "".join(rng.pick(string.ascii_uppercase) for _ in range(rng.between(1, 2)))
    number = f"{rng.between(1, 9999):04}"
    return grouping.value.join([state, rto, series, number])


def is_valid_passport(text: str) -> bool:
    t = text.upper()
    return len(t) == 8 and t.isascii() and t[0] in PASSPORT_LETTERS and is_digits(t[1:]) and t[1] != "0" and t[7] != "0"


def is_valid_voter_id(text: str) -> bool:
    t = text.upper()
    return len(t) == 10 and is_upper_alpha(t[:3]) and is_digits(t[3:])


def is_valid_vehicle(text: str) -> bool:
    t = ungrouped(text).upper()
    if not t.isascii() or len(t) not in (9, 10):
        return False
    series_end = len(t) - 4
    return is_upper_alpha(t[:2]) and is_digits(t[2:4]) and is_upper_alpha(t[4:series_end]) and is_digits(t[series_end:])
