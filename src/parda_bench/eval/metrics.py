from dataclasses import dataclass


def ratio(numerator: int, denominator: int) -> float | None:
    return numerator / denominator if denominator else None


@dataclass
class Counts:
    """True positives, false positives and false negatives for one bucket."""

    tp: int = 0
    fp: int = 0
    fn: int = 0

    def precision(self) -> float | None:
        """`None` when nothing was predicted."""
        return ratio(self.tp, self.tp + self.fp)

    def recall(self) -> float | None:
        """`None` when there was nothing to find."""
        return ratio(self.tp, self.tp + self.fn)

    def f1(self) -> float | None:
        """`None` when there was nothing to find and nothing was predicted."""
        return ratio(2 * self.tp, 2 * self.tp + self.fp + self.fn)

    def add(self, other: "Counts") -> None:
        self.tp += other.tp
        self.fp += other.fp
        self.fn += other.fn
