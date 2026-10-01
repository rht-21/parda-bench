from enum import StrEnum
from typing import Annotated, Literal

from pydantic import Field

from parda_bench.spec.base import Strict
from parda_bench.spec.entity import EntityType


class Span(Strict):
    """A labeled region of `Sample.text`.

    Offsets count Unicode code points (a Python `str` index), end exclusive: not bytes, not UTF-16 units.
    """

    start: int
    end: int
    entity: EntityType


class Lang(StrEnum):
    EN = "en"
    HI_LATN = "hi-Latn"
    HI_DEVA = "hi-Deva"
    MIXED = "mixed"


class Difficulty(StrEnum):
    EASY = "easy"
    MEDIUM = "medium"
    HARD = "hard"


class Source(StrEnum):
    TEMPLATE = "template"
    LLM = "llm"
    HANDWRITTEN = "handwritten"


class Positive(Strict):
    kind: Literal["positive"] = "positive"
    spans: list[Span]


class HardNegative(Strict):
    """A decoy that looks like `decoy` but must not be flagged; it carries no spans."""

    kind: Literal["hard_negative"] = "hard_negative"
    decoy: EntityType


Labels = Annotated[Positive | HardNegative, Field(discriminator="kind")]


class Sample(Strict):
    """One JSONL record of the dataset."""

    id: str
    text: str
    labels: Labels
    lang: Lang
    difficulty: Difficulty
    source: Source
    generator_version: str
