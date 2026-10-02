import random
from collections.abc import Sequence
from typing import TypeVar

T = TypeVar("T")


class DataRng:
    """Seeded randomness built only on `random.random()`, the one method Python guarantees to keep producing the
    same sequence across versions, so a dataset rebuilds byte for byte on any Python."""

    def __init__(self, seed: int) -> None:
        self._random = random.Random(seed)

    def below(self, n: int) -> int:
        """Uniform integer in `0..n-1`."""
        return int(self._random.random() * n)

    def between(self, low: int, high: int) -> int:
        """Uniform integer in `low..=high`."""
        return low + self.below(high - low + 1)

    def pick(self, items: Sequence[T]) -> T:
        if not items:
            raise ValueError("pick from an empty sequence")
        return items[self.below(len(items))]

    def chance(self, probability: float) -> bool:
        return self._random.random() < probability
