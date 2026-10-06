import pytest

from parda_bench.eval.detection import EvalError, NegativeCounts, UnmappedCounts, evaluate
from parda_bench.eval.latency import Percentiles
from parda_bench.eval.matching import MatchMode, match_spans
from parda_bench.eval.metrics import Counts
from parda_bench.spec.entity import EntityType
from parda_bench.spec.record import Detected, DetectionRecord, Failed, Mapped, PredictedSpan, Unmapped
from parda_bench.spec.sample import Difficulty, HardNegative, Lang, Positive, Sample, Source, Span

NAME, PAN = EntityType.PERSON_NAME, EntityType.PAN


def s(start: int, end: int, entity: EntityType) -> Span:
    return Span(start=start, end=end, entity=entity)


def test_precision_recall_f1_and_empty_buckets() -> None:
    c = Counts(6, 2, 4)
    assert (c.precision(), c.recall(), c.f1()) == (0.75, 0.6, 12 / 18)
    assert (Counts().precision(), Counts().recall(), Counts().f1()) == (None, None, None)
    assert (Counts(0, 0, 3).precision(), Counts(0, 0, 3).recall()) == (None, 0.0)


def test_strict_needs_exact_bounds_relaxed_needs_overlap() -> None:
    gold, pred = [s(0, 12, NAME)], [s(0, 5, NAME)]
    assert match_spans(gold, pred, MatchMode.STRICT)[NAME] == Counts(0, 1, 1)
    assert match_spans(gold, pred, MatchMode.RELAXED)[NAME] == Counts(1, 0, 0)


def test_wrong_type_is_a_miss_and_a_false_alarm() -> None:
    r = match_spans([s(0, 10, PAN)], [s(0, 10, NAME)], MatchMode.RELAXED)
    assert (r[PAN], r[NAME]) == (Counts(0, 0, 1), Counts(0, 1, 0))


def test_one_to_one_matching() -> None:
    assert match_spans([s(0, 5, NAME), s(6, 12, NAME)], [s(0, 12, NAME)], MatchMode.RELAXED)[NAME] == Counts(1, 0, 1)
    assert match_spans([s(0, 5, PAN)], [s(0, 5, PAN), s(0, 5, PAN)], MatchMode.STRICT)[PAN] == Counts(1, 1, 0)
    gold = [s(0, 10, NAME), s(10, 12, NAME)]
    assert match_spans(gold, [s(8, 12, NAME), s(0, 4, NAME)], MatchMode.RELAXED)[NAME] == Counts(2, 0, 0)


def test_nearest_rank_percentiles() -> None:
    p = Percentiles.of(list(range(100, 0, -1)))
    assert p is not None and (p.p50, p.p95, p.p99, p.max, p.mean) == (50, 95, 99, 100, 50)
    one = Percentiles.of([7])
    assert one is not None and (one.p50, one.p99) == (7, 7)
    assert Percentiles.of([]) is None


def sample(sample_id: str, labels: Positive | HardNegative, lang: Lang = Lang.EN) -> Sample:
    return Sample(
        id=sample_id,
        text="x" * 40,
        labels=labels,
        lang=lang,
        difficulty=Difficulty.EASY,
        source=Source.HANDWRITTEN,
        generator_version="t",
    )


def detected(sample_id: str, predicted: list[PredictedSpan], ns: int) -> DetectionRecord:
    return DetectionRecord(sample_id=sample_id, result=Detected(predicted=predicted, elapsed_ns=ns, wall_ns=ns + 10))


def pred(start: int, end: int, entity: Mapped | Unmapped) -> PredictedSpan:
    return PredictedSpan(start=start, end=end, entity=entity)


def test_scores_a_small_run_end_to_end() -> None:
    samples = [
        sample("a", Positive(spans=[s(0, 10, PAN)])),
        sample("b", HardNegative(decoy=EntityType.PHONE), Lang.HI_LATN),
        sample("c", Positive(spans=[])),
    ]
    records = [
        detected("a", [pred(0, 10, Mapped(mapped=PAN))], 100),
        detected("b", [pred(5, 15, Mapped(mapped=EntityType.PHONE))], 300),
        detected("c", [pred(1, 3, Unmapped(unmapped="ORG"))], 200),
    ]
    r = evaluate(samples, records)
    assert r.evaluated_samples == 3
    assert r.scores[MatchMode.STRICT].by_entity[PAN] == Counts(1, 0, 0)
    assert r.scores[MatchMode.STRICT].micro == Counts(1, 1, 0)
    assert r.hard_negatives[EntityType.PHONE] == NegativeCounts(1, 1, 1)
    assert r.unmapped["ORG"] == UnmappedCounts(1, 0)
    assert r.by_lang[Lang.HI_LATN][MatchMode.RELAXED] == Counts(0, 1, 0)
    assert r.by_source[Source.HANDWRITTEN][MatchMode.STRICT] == Counts(1, 1, 0)
    assert r.latency is not None and (r.latency.tool_ns.p50, r.latency.wall_ns.max) == (200, 310)


def test_failed_detection_counts_gold_as_missed() -> None:
    samples = [sample("a", Positive(spans=[s(0, 10, PAN)]))]
    r = evaluate(samples, [DetectionRecord(sample_id="a", result=Failed(message="boom"))])
    assert (r.failed_samples, r.scores[MatchMode.RELAXED].micro, r.latency) == (1, Counts(0, 0, 1), None)


def test_unmapped_overlap_and_partial_runs() -> None:
    samples = [sample("a", Positive(spans=[s(0, 10, PAN)])), sample("b", Positive(spans=[]))]
    r = evaluate(samples, [detected("a", [pred(2, 8, Unmapped(unmapped="IN_PAN"))], 1)])
    assert r.unmapped["IN_PAN"].overlapping_gold == 1
    assert (r.evaluated_samples, r.missing_samples) == (1, 1)


def test_rejects_records_from_another_dataset_and_duplicates() -> None:
    samples = [sample("a", Positive(spans=[]))]
    with pytest.raises(EvalError, match="not in the dataset"):
        evaluate(samples, [detected("zzz", [], 1)])
    with pytest.raises(EvalError, match="more than once"):
        evaluate(samples, [detected("a", [], 1), detected("a", [], 1)])
