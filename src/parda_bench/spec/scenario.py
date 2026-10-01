from enum import StrEnum
from typing import Annotated, Any, Literal

from pydantic import Field

from parda_bench.spec.base import Strict


class Category(StrEnum):
    BASIC = "basic"
    STREAMING_SPLIT = "streaming_split"
    TOOL_CALLS = "tool_calls"
    MULTI_TURN = "multi_turn"
    PLACEHOLDER_MANGLING = "placeholder_mangling"
    REPEATS_ADJACENCY = "repeats_adjacency"
    UNICODE = "unicode"
    STRUCTURED_OUTPUT = "structured_output"
    FAILURE_BEHAVIOUR = "failure_behaviour"


class Api(StrEnum):
    OPENAI_CHAT = "openai_chat"
    ANTHROPIC_MESSAGES = "anthropic_messages"


class Mangle(StrEnum):
    LOWERCASE = "lowercase"
    UPPERCASE = "uppercase"
    STRIP_DELIMITERS = "strip_delimiters"
    SPACE_PADDED = "space_padded"


class CheckKind(StrEnum):
    NO_LEAK_UPSTREAM = "no_leak_upstream"
    """No `pii` value reaches the upstream, verbatim or with separators and case changed."""
    RESTORED_EXACT = "restored_exact"
    """The client sees the scripted reply with every placeholder replaced by its real value."""
    NO_PLACEHOLDER_FRAGMENT = "no_placeholder_fragment"
    """The client sees no whole or partial placeholder."""
    PLACEHOLDER_STABLE_ACROSS_TURNS = "placeholder_stable_across_turns"
    """A value gets the same placeholder every time it is sent, within and across turns."""
    JSON_VALID = "json_valid"
    """Tool-call arguments, and text replies that are JSON as scripted, are still valid JSON after restoring."""
    FAILS_CLOSED = "fails_closed"
    """An upstream error reaches the client as an error with the same status."""


class ToolDefinition(Strict):
    name: str
    description: str
    parameters: dict[str, Any]


class LiteralInput(Strict):
    text: str


class SlotInput(Strict):
    slot: int


InputSegment = LiteralInput | SlotInput


class LiteralReply(Strict):
    text: str


class SlotReply(Strict):
    """(Part of) the placeholder the tool chose for `slot`, optionally altered the way models do."""

    slot: int
    from_: int | None = Field(default=None, alias="from")
    """Char range of the placeholder to emit; omitted bounds mean start/end of the placeholder."""
    to: int | None = None
    mangle: Mangle | None = None


ReplySegment = LiteralReply | SlotReply


class UserInput(Strict):
    kind: Literal["user"]
    message: list[InputSegment]


class ToolResultInput(Strict):
    kind: Literal["tool_result"]
    call_id: str
    content: list[InputSegment]


TurnInput = Annotated[UserInput | ToolResultInput, Field(discriminator="kind")]


class TextReply(Strict):
    """Each inner list is one SSE chunk when streaming."""

    kind: Literal["text"]
    chunks: list[list[ReplySegment]]


class ToolCallReply(Strict):
    kind: Literal["tool_call"]
    call_id: str
    name: str
    argument_chunks: list[list[ReplySegment]]


class HttpErrorReply(Strict):
    kind: Literal["http_error"]
    status: int
    body: str


Reply = Annotated[TextReply | ToolCallReply | HttpErrorReply, Field(discriminator="kind")]


class Turn(Strict):
    input: TurnInput
    reply: Reply


class Scenario(Strict):
    """One restore test, authored as `scenarios/<category>/<id>.toml`.

    `pii` holds the real values; segments refer to them by index (`slot`). The mock upstream learns each slot's
    placeholder by aligning the masked request it receives against the input's literal segments.
    """

    id: str
    category: Category
    description: str
    api: Api
    stream: bool
    pii: list[str]
    tools: list[ToolDefinition] = Field(default_factory=list)
    turns: list[Turn]
    checks: list[CheckKind]


def segments(turn_input: UserInput | ToolResultInput) -> list[InputSegment]:
    """The text segments of a turn's input, whichever kind it is."""
    return turn_input.message if isinstance(turn_input, UserInput) else turn_input.content
