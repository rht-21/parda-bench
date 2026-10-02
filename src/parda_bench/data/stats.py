"""Dataset composition counts."""

from collections import Counter
from typing import Any

from parda_bench.spec.sample import HardNegative, Positive, Sample


def stats(samples: list[Sample]) -> dict[str, Any]:
    positives = [s.labels for s in samples if isinstance(s.labels, Positive)]
    negatives = [s.labels for s in samples if isinstance(s.labels, HardNegative)]
    spans = Counter(span.entity.value for labels in positives for span in labels.spans)

    def by(values: list[str]) -> dict[str, int]:
        return dict(sorted(Counter(values).items()))

    return {
        "samples": len(samples),
        "positive_samples": len(positives),
        "empty_samples": sum(1 for labels in positives if not labels.spans),
        "hard_negatives": len(negatives),
        "spans_by_entity": dict(sorted(spans.items())),
        "hard_negatives_by_decoy": by([n.decoy.value for n in negatives]),
        "by_lang": by([s.lang.value for s in samples]),
        "by_difficulty": by([s.difficulty.value for s in samples]),
        "by_source": by([s.source.value for s in samples]),
    }
