"""One-to-one matching of predicted spans against gold spans of the same entity type."""

from collections import defaultdict
from enum import StrEnum

from parda_bench.eval.metrics import Counts
from parda_bench.spec.entity import EntityType
from parda_bench.spec.sample import Span


class MatchMode(StrEnum):
    STRICT = "strict"
    """Same entity type and identical boundaries."""
    RELAXED = "relaxed"
    """Same entity type and any character overlap; pairs with the largest overlap are matched first."""


def overlap(a: Span, b: Span) -> int:
    return max(0, min(a.end, b.end) - max(a.start, b.start))


def match_spans(gold: list[Span], predicted: list[Span], mode: MatchMode) -> dict[EntityType, Counts]:
    """Per-entity counts for one sample. Every gold and every predicted span is counted exactly once."""
    pairs = sorted(
        (
            (-overlap(g, p), gi, pi)
            for gi, g in enumerate(gold)
            for pi, p in enumerate(predicted)
            if g.entity == p.entity
            and ((g.start, g.end) == (p.start, p.end) if mode is MatchMode.STRICT else overlap(g, p) > 0)
        )
    )
    gold_used: set[int] = set()
    pred_used: set[int] = set()
    for _, gi, pi in pairs:
        if gi not in gold_used and pi not in pred_used:
            gold_used.add(gi)
            pred_used.add(pi)
    counts: dict[EntityType, Counts] = defaultdict(Counts)
    for i, span in enumerate(gold):
        if i in gold_used:
            counts[span.entity].tp += 1
        else:
            counts[span.entity].fn += 1
    for i, span in enumerate(predicted):
        if i not in pred_used:
            counts[span.entity].fp += 1
    return dict(counts)
