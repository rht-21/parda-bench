"""Dataset integrity checks: span geometry, structural validity of labeled identifiers, unique ids."""

from dataclasses import dataclass

from parda_bench.data.ids import is_valid
from parda_bench.spec.sample import Positive, Sample


@dataclass(frozen=True)
class Issue:
    sample_id: str
    message: str


def validate(samples: list[Sample]) -> list[Issue]:
    """Every problem found in `samples`; empty means the dataset is consistent."""
    issues: list[Issue] = []
    seen: set[str] = set()
    for sample in samples:
        if sample.id in seen:
            issues.append(Issue(sample.id, "duplicate id"))
        seen.add(sample.id)
        issues.extend(Issue(sample.id, m) for m in _sample_problems(sample))
    return issues


def _sample_problems(sample: Sample) -> list[str]:
    problems = [] if sample.text.strip() else ["empty text"]
    if not isinstance(sample.labels, Positive):
        return problems
    previous_end = 0
    for span in sample.labels.spans:
        if span.start >= span.end or span.end > len(sample.text):
            problems.append(f"span {span.start}..{span.end} out of bounds for {len(sample.text)} chars")
            continue
        if span.start < previous_end:
            problems.append(f"span {span.start}..{span.end} overlaps or is out of order")
        previous_end = span.end
        covered = sample.text[span.start : span.end]
        if covered.strip() != covered:
            problems.append(f"span {covered!r} has surrounding whitespace")
        if is_valid(span.entity, covered) is False:
            problems.append(f"{covered!r} is not a valid {span.entity}")
    return problems
