"""Pattern recognizers. Overlapping matches are resolved in favour of the longer one."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Callable

from regex_baseline.checksums import digits_only, gstin_valid, luhn_valid, verhoeff_valid

_NOT_ALNUM_BEFORE = r"(?<![A-Za-z0-9])"
_NOT_ALNUM_AFTER = r"(?![A-Za-z0-9])"


@dataclass(frozen=True)
class Match:
    start: int
    end: int
    label: str


@dataclass(frozen=True)
class Recognizer:
    label: str
    pattern: re.Pattern[str]
    is_valid: Callable[[str], bool] = lambda _: True


def _r(pattern: str) -> re.Pattern[str]:
    return re.compile(pattern)


RECOGNIZERS: tuple[Recognizer, ...] = (
    Recognizer("AADHAAR_VID", _r(r"(?<!\d)\d{4}[ -]?\d{4}[ -]?\d{4}[ -]?\d{4}(?!\d)"),
               lambda s: verhoeff_valid(digits_only(s))),
    Recognizer("CARD", _r(r"(?<!\d)\d{4}[ -]?\d{4}[ -]?\d{4}[ -]?\d{4}(?!\d)"), lambda s: luhn_valid(digits_only(s))),
    Recognizer("AADHAAR", _r(r"(?<!\d)[2-9]\d{3}[ -]?\d{4}[ -]?\d{4}(?!\d)"), lambda s: verhoeff_valid(digits_only(s))),
    Recognizer("PHONE", _r(r"(?<![\w+])(?:\+91[ -]?|0)?[6-9]\d{4}[ -]?\d{5}(?!\d)")),
    Recognizer("GSTIN", _r(_NOT_ALNUM_BEFORE + r"\d{2}[A-Za-z]{5}\d{4}[A-Za-z][1-9A-Za-z][Zz][0-9A-Za-z]" + _NOT_ALNUM_AFTER),
               gstin_valid),
    Recognizer("PAN", _r(_NOT_ALNUM_BEFORE + r"[A-Za-z]{3}[PCHFATBLJGpchfatbljg][A-Za-z]\d{4}[A-Za-z]" + _NOT_ALNUM_AFTER)),
    Recognizer("IFSC", _r(_NOT_ALNUM_BEFORE + r"[A-Za-z]{4}0[A-Za-z0-9]{6}" + _NOT_ALNUM_AFTER)),
    Recognizer("EMAIL", _r(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")),
    # A dot may end the sentence; only a dot followed by more domain marks an email instead.
    Recognizer("UPI", _r(r"[A-Za-z0-9._-]{2,256}@[A-Za-z]{2,64}(?![A-Za-z0-9-]|\.[A-Za-z0-9])")),
    Recognizer("PASSPORT", _r(_NOT_ALNUM_BEFORE + r"[A-PR-WYa-pr-wy][1-9]\d{5}[1-9]" + _NOT_ALNUM_AFTER)),
    Recognizer("VOTER_ID", _r(_NOT_ALNUM_BEFORE + r"[A-Za-z]{3}\d{7}" + _NOT_ALNUM_AFTER)),
    Recognizer("VEHICLE", _r(_NOT_ALNUM_BEFORE + r"[A-Za-z]{2}[ -]?\d{2}[ -]?[A-Za-z]{1,2}[ -]?\d{4}" + _NOT_ALNUM_AFTER)),
)


def detect(text: str) -> list[Match]:
    candidates = [
        Match(m.start(), m.end(), r.label)
        for r in RECOGNIZERS
        for m in r.pattern.finditer(text)
        if r.is_valid(m.group())
    ]
    kept: list[Match] = []
    for c in sorted(candidates, key=lambda m: (-(m.end - m.start), m.start)):
        if all(c.end <= k.start or k.end <= c.start for k in kept):
            kept.append(c)
    return sorted(kept, key=lambda m: m.start)
