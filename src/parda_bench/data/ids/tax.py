import string

from parda_bench.data.checksum import GSTIN_CHARSET, gstin_check_char
from parda_bench.data.ids import Case, is_digits, is_upper_alpha
from parda_bench.data.rng import DataRng

PAN_HOLDER_TYPES = "PCHFATBLJG"
"""Valid 4th PAN characters: person, company, HUF, firm, AOP, trust, BOI, local authority, AJP, government."""
NOT_PAN_HOLDER_TYPES = "DEIKMNOQRSUVWXYZ"
GST_STATE_CODES = [
    "01",
    "02",
    "03",
    "04",
    "05",
    "06",
    "07",
    "08",
    "09",
    "10",
    "11",
    "12",
    "13",
    "14",
    "15",
    "16",
    "17",
    "18",
    "19",
    "20",
    "21",
    "22",
    "23",
    "24",
    "26",
    "27",
    "29",
    "30",
    "32",
    "33",
    "34",
    "35",
    "36",
    "37",
    "38",
    "97",
]


def _letters(rng: DataRng, n: int) -> str:
    return "".join(rng.pick(string.ascii_uppercase) for _ in range(n))


def _pan_with_holder(rng: DataRng, holder: str, initial: str) -> str:
    serial = "".join(str(rng.below(10)) for _ in range(4))
    return f"{_letters(rng, 3)}{holder}{initial.upper()}{serial}{_letters(rng, 1)}"


def pan(rng: DataRng, surname_initial: str, case: Case) -> str:
    """PAN for an individual whose surname starts with `surname_initial`."""
    return case.apply(_pan_with_holder(rng, "P", surname_initial))


def invalid_pan(rng: DataRng) -> str:
    """PAN-shaped code with an impossible holder-type character, for hard negatives."""
    holder = rng.pick(NOT_PAN_HOLDER_TYPES)
    return _pan_with_holder(rng, holder, rng.pick(string.ascii_uppercase))


def gstin(rng: DataRng, case: Case) -> str:
    """15-character GSTIN: state code, the business PAN, entity number, `Z`, check character."""
    state = rng.pick(GST_STATE_CODES)
    business_pan = _pan_with_holder(rng, rng.pick("CFPT"), rng.pick(string.ascii_uppercase))
    first14 = f"{state}{business_pan}{rng.between(1, 9)}Z"
    check = gstin_check_char(first14)
    assert check is not None, f"generated GSTIN {first14} is not 0-9A-Z"
    return case.apply(first14 + check)


def invalid_gstin(rng: DataRng) -> str:
    """GSTIN-shaped code whose check character is wrong, for hard negatives."""
    valid = gstin(rng, Case.UPPER)
    wrong = rng.pick([c for c in GSTIN_CHARSET if c != valid[-1]])
    return valid[:14] + wrong


def is_valid_pan(text: str) -> bool:
    t = text.upper()
    return (
        len(t) == 10
        and t.isascii()
        and is_upper_alpha(t[:5])
        and t[3] in PAN_HOLDER_TYPES
        and is_digits(t[5:9])
        and is_upper_alpha(t[9])
    )


def is_valid_gstin(text: str) -> bool:
    t = text.upper()
    return (
        len(t) == 15
        and t.isascii()
        and t[:2] in GST_STATE_CODES
        and is_valid_pan(t[2:12])
        and gstin_check_char(t[:14]) == t[14]
    )
