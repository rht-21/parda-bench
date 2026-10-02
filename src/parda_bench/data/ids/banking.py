import string

from parda_bench.data.ids import Case, is_upper_alpha
from parda_bench.data.rng import DataRng

BANK_CODES = [
    "SBIN",
    "HDFC",
    "ICIC",
    "UTIB",
    "PUNB",
    "BARB",
    "CNRB",
    "KKBK",
    "YESB",
    "IDIB",
    "UBIN",
    "BKID",
    "IOBA",
    "INDB",
    "FDRL",
]
UPI_HANDLES = ["okaxis", "oksbi", "okhdfcbank", "okicici", "ybl", "paytm", "ibl", "axl", "apl"]
"""UPI payment service provider handles."""


def ascii_lower(s: str) -> str:
    """Lowercase ASCII letters of `s` only, for building handles from names like `D'Souza`."""
    return "".join(c.lower() for c in s if c.isascii() and c.isalpha())


def ifsc(rng: DataRng, case: Case) -> str:
    """IFSC: 4-letter bank code, `0`, 6-digit branch code."""
    branch = "".join(str(rng.below(10)) for _ in range(6))
    return case.apply(f"{rng.pick(BANK_CODES)}0{branch}")


def ifsc_like_code(rng: DataRng) -> str:
    """IFSC-shaped code whose prefix is no bank's, for hard negatives."""
    while True:
        prefix = "".join(rng.pick(string.ascii_uppercase) for _ in range(4))
        if prefix not in BANK_CODES:
            break
    tail = "".join(rng.pick("ABCDEFGHJKLMNPQRSTUVWXYZ0123456789") for _ in range(6))
    return f"{prefix}0{tail}"


def upi(rng: DataRng, local: str) -> str:
    return f"{local}@{rng.pick(UPI_HANDLES)}"


def upi_local_from_name(rng: DataRng, first: str, last: str) -> str:
    """Name-derived UPI user part, e.g. `rahul.sharma` or `rahulsharma92`."""
    given, family = ascii_lower(first), ascii_lower(last)
    variant = rng.below(3)
    if variant == 0:
        return f"{given}.{family}"
    if variant == 1:
        return f"{given}{family}{rng.between(10, 99)}"
    return f"{given}{rng.between(100, 999)}"


def is_valid_ifsc(text: str) -> bool:
    t = text.upper()
    return len(t) == 11 and t.isascii() and is_upper_alpha(t[:4]) and t[4] == "0" and t[5:].isalnum()


def is_valid_upi(text: str) -> bool:
    local, at, handle = text.partition("@")
    return (
        bool(at)
        and 2 <= len(local) <= 256
        and local.isascii()
        and all(c.isalnum() or c in "._-" for c in local)
        and 2 <= len(handle) <= 64
        and handle.isascii()
        and handle.isalpha()
    )
