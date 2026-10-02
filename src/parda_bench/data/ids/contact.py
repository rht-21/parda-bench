from enum import Enum

from parda_bench.data.checksum import ascii_digits
from parda_bench.data.ids import ungrouped
from parda_bench.data.ids.banking import ascii_lower
from parda_bench.data.rng import DataRng

EMAIL_DOMAINS = ("gmail.com", "yahoo.co.in", "rediffmail.com", "outlook.com", "hotmail.com", "yahoo.com")
SERVICE_ACCOUNTS = (
    "root@localhost",
    "admin@localhost",
    "postgres@db-primary",
    "deploy@build-agent-3",
    "jenkins@ci-runner",
    "noreply@localhost",
)
"""Email-shaped, but they belong to no person and have no domain."""


class PhoneStyle(Enum):
    INTL = "intl"
    """`+91 98765 43210`"""
    INTL_HYPHEN = "intl_hyphen"
    """`+91-9876543210`"""
    TRUNK_ZERO = "zero"
    """`09876543210`"""
    PLAIN = "plain"
    """`9876543210`"""
    SPLIT = "split"
    """`98765 43210`"""


class EmailCase(Enum):
    LOWER = "lower"
    TITLE = "title"
    """`Rahul.Sharma@…`, as people sometimes type it."""


def mobile_digits(rng: DataRng) -> str:
    """Ten mobile digits, first digit 6-9."""
    return str(rng.between(6, 9)) + "".join(str(rng.below(10)) for _ in range(9))


def format_phone(digits: str, style: PhoneStyle) -> str:
    a, b = digits[:5], digits[5:]
    return {
        PhoneStyle.INTL: f"+91 {a} {b}",
        PhoneStyle.INTL_HYPHEN: f"+91-{digits}",
        PhoneStyle.TRUNK_ZERO: f"0{digits}",
        PhoneStyle.PLAIN: digits,
        PhoneStyle.SPLIT: f"{a} {b}",
    }[style]


def pnr(rng: DataRng) -> str:
    """Ten digits that are not a mobile number in this context (a railway PNR), for hard negatives."""
    return str(rng.between(2, 8)) + "".join(str(rng.below(10)) for _ in range(9))


def email(rng: DataRng, first: str, last: str, case: EmailCase) -> str:
    """Email derived from a person's name."""
    given, family = ascii_lower(first), ascii_lower(last)
    if case is EmailCase.TITLE:
        given, family = given.capitalize(), family.capitalize()
    variant = rng.below(4)
    if variant == 0:
        local = f"{given}.{family}"
    elif variant == 1:
        local = f"{given}{family}{rng.between(1, 99)}"
    elif variant == 2:
        local = f"{given}_{family}{rng.between(70, 99)}"
    else:
        local = f"{given[:1]}{family}"
    return f"{local}@{rng.pick(EMAIL_DOMAINS)}"


def social_handle(rng: DataRng, first: str, last: str) -> str:
    """Social-media handle such as `@rahul.sharma_92`, for hard negatives against UPI IDs."""
    given, family = ascii_lower(first), ascii_lower(last)
    variant = rng.below(3)
    if variant == 0:
        return f"@{given}.{family}"
    if variant == 1:
        return f"@{given}_{family}{rng.between(10, 99)}"
    return f"@the{given}{family}"


def service_account(rng: DataRng) -> str:
    return rng.pick(SERVICE_ACCOUNTS)


def is_valid_phone(text: str) -> bool:
    t = ungrouped(text)
    if t.startswith("+91"):
        t = t[3:]
    elif (len(t) == 12 and t.startswith("91")) or (len(t) == 11 and t.startswith("0")):
        t = t[len(t) - 10 :]
    d = ascii_digits(t)
    return d is not None and len(d) == 10 and d[0] >= 6


def is_valid_email(text: str) -> bool:
    local, at, domain = text.partition("@")
    return (
        bool(at)
        and bool(local)
        and not any(c.isspace() for c in local)
        and "." in domain
        and not domain.startswith(".")
        and not domain.endswith(".")
        and domain.isascii()
        and all(c.isalnum() or c in ".-" for c in domain)
    )
