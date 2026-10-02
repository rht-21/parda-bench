"""Seeded dataset build: every sample's randomness is derived from (seed, template id, index), so adding or editing
one template never changes the samples of another. Free-form entries (any source but `template`) render once."""

import hashlib
from importlib.resources import files

from parda_bench.data import GENERATOR_VERSION
from parda_bench.data.render import render
from parda_bench.data.rng import DataRng
from parda_bench.data.template import Template, TemplateError, parse_file
from parda_bench.spec.sample import HardNegative, Positive, Sample, Source

V0_1 = ("en.toml", "hi_latn.toml", "hi_deva.toml", "mixed.toml", "negatives.toml")
DATASETS: dict[str, tuple[str, ...]] = {
    "v0.1.0": V0_1,
    "v0.2.0": (
        *V0_1,
        "negatives_extra.toml",
        "noisy.toml",
        "freeform/en.toml",
        "freeform/hi_latn.toml",
        "freeform/hi_deva.toml",
        "freeform/mixed.toml",
        "freeform/negatives.toml",
    ),
}
"""Template files of each published dataset version. A published version's list never changes."""
DATASET_VERSION = "v0.2.0"
"""Version `data build` writes by default."""
SEED = 42
PER_TEMPLATE = 25


def templates_for(version: str) -> list[Template]:
    """The templates of a dataset version, in file order. Raises `TemplateError` on a bad file or duplicate id."""
    if version not in DATASETS:
        raise KeyError(f"unknown dataset version {version}; known: {sorted(DATASETS)}")
    root = files("parda_bench.data") / "templates"
    templates: list[Template] = []
    for name in DATASETS[version]:
        try:
            templates.extend(parse_file(root.joinpath(name).read_text(encoding="utf-8")))
        except TemplateError as e:
            raise TemplateError(name, str(e)) from e
    seen: set[str] = set()
    for t in templates:
        if t.id in seen:
            raise TemplateError(t.id, "duplicate template id")
        seen.add(t.id)
    return templates


def build(templates: list[Template], seed: int, per_template: int) -> list[Sample]:
    """`per_template` samples from each template and one from each free-form entry."""
    return [_sample(t, seed, n) for t in templates for n in range(per_template if t.source is Source.TEMPLATE else 1)]


def _sample(template: Template, seed: int, n: int) -> Sample:
    rendered = render(template, DataRng(_sample_seed(seed, template.id, n)))
    decoy = template.decoy_entity()
    return Sample(
        id=f"{template.id}-{n:04}",
        text=rendered.text,
        labels=HardNegative(decoy=decoy) if decoy is not None else Positive(spans=rendered.spans),
        lang=template.lang,
        difficulty=template.difficulty,
        source=template.source,
        generator_version=GENERATOR_VERSION,
    )


def _sample_seed(seed: int, template_id: str, n: int) -> int:
    digest = hashlib.sha256(f"{seed}/{template_id}/{n}".encode()).digest()
    return int.from_bytes(digest[:8], "little")
