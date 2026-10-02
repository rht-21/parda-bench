from pathlib import Path

import pytest

from parda_bench.data.build import DATASETS, build, templates_for
from parda_bench.data.ids import Grouping, tax
from parda_bench.data.io import (
    SAMPLES_FILE,
    DataIoError,
    DatasetManifest,
    file_sha256,
    read_manifest,
    read_samples,
    samples_sha256,
    write_dataset,
)
from parda_bench.data.noise import add_noise
from parda_bench.data.render import Rendered, render
from parda_bench.data.rng import DataRng
from parda_bench.data.stats import stats
from parda_bench.data.template import Fill, Labeled, Part, Template, TemplateError, Text, parse_file
from parda_bench.data.validate import validate
from parda_bench.spec.entity import EntityType
from parda_bench.spec.sample import Difficulty, Lang, Positive, Sample, Source, Span


def one(text: str) -> Template:
    return parse_file(f"[[template]]\nid = \"t\"\nlang = \"en\"\ndifficulty = \"easy\"\ntext = '''{text}'''\n")[0]


def template(parts: list[Part], lang: Lang = Lang.EN, noisy: bool = False) -> Template:
    return Template("t", lang, Difficulty.EASY, Source.TEMPLATE, noisy, tuple(parts))


def span_text(r: Rendered, i: int) -> str:
    return r.text[r.spans[i].start : r.spans[i].end]


def test_parses_fillers_styles_and_literal_braces() -> None:
    t = one('{{"id": "{aadhaar:spaced}"}} by {name}')
    assert t.parts == (Text('{"id": "'), Fill("aadhaar", Grouping.SPACED), Text('"} by '), Fill("name", None))


@pytest.mark.parametrize(
    "text",
    [
        "{ssn}",
        "{pan:spaced}",
        "{passport:upper}",
        "oops } here",
        "open {name forever",
        "PNR {decoy_pnr} {name}",
        "{decoy_pnr} {decoy_pan}",
        "{=NAME:Rahul}",
        "{=PERSON_NAME: Rahul}",
        "{=PERSON_NAME}",
    ],
)
def test_rejects_bad_templates(text: str) -> None:
    with pytest.raises(TemplateError):
        one(text)


def test_labeled_spans_literal_decoys_and_file_source() -> None:
    assert one("Ask {=PERSON_NAME:Rahul}").parts[1] == Labeled(EntityType.PERSON_NAME, "Rahul")
    assert one("Follow {!UPI_ID:@chai.point}").decoy_entity() is EntityType.UPI_ID
    t = parse_file(
        'source = "llm"\n[[template]]\nid = "f"\nlang = "en"\ndifficulty = "hard"\nnoisy = true\ntext = "hi"\n'
    )
    assert (t[0].source, t[0].noisy) == (Source.LLM, True)


def test_spans_count_code_points_after_devanagari() -> None:
    r = render(template([Text("मेरा आधार "), Fill("aadhaar", Grouping.PLAIN)], Lang.HI_DEVA), DataRng(1))
    assert r.spans[0].start == 10 and len(span_text(r, 0)) == 12


def test_phone_and_phone_upi_share_the_persona_number() -> None:
    from parda_bench.data.ids.contact import PhoneStyle
    from parda_bench.data.template import UpiStyle

    r = render(template([Fill("phone", PhoneStyle.PLAIN), Text(" / "), Fill("upi", UpiStyle.PHONE)]), DataRng(2))
    assert span_text(r, 1).startswith(span_text(r, 0))


def test_devanagari_template_defaults_names_to_devanagari() -> None:
    r = render(template([Fill("name", None)], Lang.HI_DEVA), DataRng(3))
    assert any("ऀ" <= c <= "ॿ" for c in r.text)


def test_noise_keeps_ends_is_deterministic_and_never_moves_spans() -> None:
    text = " Please, Sir: send the Original Bill to my email ID before Monday. "
    assert add_noise(text, DataRng(2)) == add_noise(text, DataRng(2)) != text
    assert all(add_noise(text, DataRng(s))[0] == " " == add_noise(text, DataRng(s))[-1] for s in range(100))
    t = template(
        [
            Text("Please, Sir: my PAN is "),
            Fill("pan", None),
            Text(" and the Bill is lost. "),
            Labeled(EntityType.PERSON_NAME, "Lalita"),
        ],
        noisy=True,
    )
    for seed in range(100):
        r = render(t, DataRng(seed))
        assert tax.is_valid_pan(span_text(r, 0)) and span_text(r, 1) == "Lalita"


def test_build_is_deterministic_and_templates_are_independent() -> None:
    t = templates_for("v0.2.0")
    assert build(t, 7, 3) == build(t, 7, 3)
    assert build(t, 7, 1)[0].text != build(t, 8, 1)[0].text
    assert build(t[3:4], 7, 2) == build(t, 7, 2)[6:8]


@pytest.mark.parametrize("version", sorted(DATASETS))
def test_published_versions_are_consistent(version: str) -> None:
    samples = build(templates_for(version), 42, 2)
    assert validate(samples) == []
    s = stats(samples)
    assert len(s["spans_by_entity"]) == 15 and len(s["by_lang"]) == 4


def test_v0_2_adds_free_form_rendered_once_and_new_decoys() -> None:
    samples = build(templates_for("v0.2.0"), 1, 3)
    llm = [s for s in samples if s.source is Source.LLM]
    assert len(llm) == 300 and all(s.id.endswith("-0000") for s in llm)
    assert {"GSTIN", "IFSC", "UPI_ID", "EMAIL"} <= set(stats(samples)["hard_negatives_by_decoy"])


def test_v0_1_samples_are_unchanged_inside_v0_2() -> None:
    old = build(templates_for("v0.1.0"), 42, 2)
    new = {s.id: s for s in build(templates_for("v0.2.0"), 42, 2)}
    assert all(new[s.id] == s for s in old)


def sample(sample_id: str, text: str, spans: list[Span]) -> Sample:
    return Sample(
        id=sample_id,
        text=text,
        labels=Positive(spans=spans),
        lang=Lang.EN,
        difficulty=Difficulty.EASY,
        source=Source.HANDWRITTEN,
        generator_version="test",
    )


def test_validate_flags_each_kind_of_problem() -> None:
    name = EntityType.PERSON_NAME
    assert validate([sample("a", "PAN ABCPS1234K ok", [Span(start=4, end=14, entity=EntityType.PAN)])]) == []
    assert (
        len(validate([sample("b", "Aadhaar 2345 6789 0125", [Span(start=8, end=22, entity=EntityType.AADHAAR)])])) == 1
    )
    assert len(validate([sample("a", "short", [Span(start=2, end=9, entity=name)]), sample("a", "short", [])])) == 2
    assert (
        len(
            validate(
                [
                    sample(
                        "a",
                        "Rahul Sharma",
                        [Span(start=0, end=5, entity=name), Span(start=3, end=12, entity=name)],
                    )
                ]
            )
        )
        == 1
    )
    assert len(validate([sample("a", "Rahul Sharma", [Span(start=0, end=6, entity=name)])])) == 1


def test_dataset_files_round_trip_and_report_bad_lines(tmp_path: Path) -> None:
    samples = build(templates_for("v0.2.0"), 1, 1)
    digest = samples_sha256(samples)
    manifest = DatasetManifest(
        version="t",
        seed=1,
        per_template=1,
        generator_version="g",
        sample_count=len(samples),
        samples_sha256=digest,
    )
    write_dataset(tmp_path, samples, manifest)
    assert read_samples(tmp_path / SAMPLES_FILE) == samples
    assert file_sha256(tmp_path / SAMPLES_FILE) == digest
    assert read_manifest(tmp_path) == manifest
    (tmp_path / SAMPLES_FILE).write_text("\n{not json}\n")
    with pytest.raises(DataIoError, match=":2:"):
        read_samples(tmp_path / SAMPLES_FILE)
