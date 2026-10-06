import math
from dataclasses import dataclass


@dataclass(frozen=True)
class Percentiles:
    """Nearest-rank percentiles of a set of durations, in nanoseconds."""

    count: int
    p50: int
    p95: int
    p99: int
    max: int
    mean: int

    @staticmethod
    def of(values: list[int]) -> "Percentiles | None":
        """`None` for an empty set."""
        if not values:
            return None
        s = sorted(values)

        def rank(pct: int) -> int:
            return s[max(1, math.ceil(pct * len(s) / 100)) - 1]

        return Percentiles(len(s), rank(50), rank(95), rank(99), s[-1], sum(s) // len(s))
