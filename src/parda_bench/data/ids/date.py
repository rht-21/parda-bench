from dataclasses import dataclass
from enum import Enum

from parda_bench.data.rng import DataRng

MONTHS = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
]
MONTHS_DEVA = [
    "जनवरी",
    "फ़रवरी",
    "मार्च",
    "अप्रैल",
    "मई",
    "जून",
    "जुलाई",
    "अगस्त",
    "सितंबर",
    "अक्टूबर",
    "नवंबर",
    "दिसंबर",
]


class DateStyle(Enum):
    SLASHED = "slashed"
    """`07/03/1991`"""
    HYPHENATED = "hyphen"
    """`07-03-1991`"""
    DOTTED = "dotted"
    """`07.03.1991`"""
    LONG = "long"
    """`7 March 1991`"""
    MONTH_FIRST = "month_first"
    """`March 7, 1991`"""
    ISO = "iso"
    """`1991-03-07`"""
    DEVANAGARI = "deva"
    """`7 मार्च 1991`"""


LATIN_STYLES = tuple(s for s in DateStyle if s is not DateStyle.DEVANAGARI)
"""Styles that read naturally in Latin-script text."""


def days_in_month(year: int, month: int) -> int:
    if month == 2:
        return 29 if year % 4 == 0 and (year % 100 != 0 or year % 400 == 0) else 28
    return 30 if month in (4, 6, 9, 11) else 31


@dataclass(frozen=True)
class Date:
    year: int
    month: int
    day: int

    @staticmethod
    def random(rng: DataRng, first_year: int, last_year: int) -> "Date":
        """Uniformly random day within `first_year..=last_year`."""
        year = rng.between(first_year, last_year)
        month = rng.between(1, 12)
        return Date(year, month, rng.between(1, days_in_month(year, month)))

    def format(self, style: DateStyle) -> str:
        y, m, d = self.year, self.month, self.day
        name = MONTHS[m - 1]
        return {
            DateStyle.SLASHED: f"{d:02}/{m:02}/{y}",
            DateStyle.HYPHENATED: f"{d:02}-{m:02}-{y}",
            DateStyle.DOTTED: f"{d:02}.{m:02}.{y}",
            DateStyle.LONG: f"{d} {name} {y}",
            DateStyle.MONTH_FIRST: f"{name} {d}, {y}",
            DateStyle.ISO: f"{y}-{m:02}-{d:02}",
            DateStyle.DEVANAGARI: f"{d} {MONTHS_DEVA[m - 1]} {y}",
        }[style]
