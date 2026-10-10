"""The mock upstream's scripted behaviour for one scenario, independent of HTTP."""

from dataclasses import dataclass, field
from typing import Any

from parda_bench.restore.align import AlignError, SlotText, align, render_upstream_chunk
from parda_bench.spec.scenario import ReplySegment, Scenario, TextReply, ToolCallReply, segments


@dataclass(frozen=True)
class TextScript:
    chunks: list[str]


@dataclass(frozen=True)
class ToolCallScript:
    call_id: str
    name: str
    argument_chunks: list[str]


@dataclass(frozen=True)
class HttpErrorScript:
    status: int
    body: str


@dataclass(frozen=True)
class MockError:
    """The request could not be matched to the script (tool altered literal text, unexpected turn, ...)."""

    message: str


ScriptedReply = TextScript | ToolCallScript | HttpErrorScript | MockError


@dataclass
class UpstreamLog:
    """Everything the upstream saw and learned during one scenario."""

    payloads: list[Any] = field(default_factory=list)
    """Request payloads exactly as received."""
    alignments: list[tuple[int, list[SlotText]]] = field(default_factory=list)
    """Per turn, the text found in place of each slot occurrence."""
    errors: list[str] = field(default_factory=list)

    def placeholders(self) -> dict[int, str]:
        """The first placeholder seen for each slot."""
        found: dict[int, str] = {}
        for _, slots in self.alignments:
            for slot, text in slots:
                found.setdefault(slot, text)
        return found


class Engine:
    def __init__(self, scenario: Scenario) -> None:
        self.scenario = scenario
        self.log = UpstreamLog()

    def record_payload(self, payload: Any) -> None:
        self.log.payloads.append(payload)

    def reply(self, turn: int, masked_input: str) -> ScriptedReply:
        """Learns placeholders from turn `turn`'s masked input and renders the scripted reply with them."""
        try:
            return self._reply(turn, masked_input)
        except AlignError as e:
            self.log.errors.append(f"turn {turn}: {e}")
            return MockError(str(e))

    def _reply(self, turn: int, masked_input: str) -> ScriptedReply:
        turns = self.scenario.turns
        if turn >= len(turns):
            raise AlignError(f"request for turn {turn}, but the scenario has {len(turns)}")
        script = turns[turn]
        self.log.alignments.append((turn, align(segments(script.input), masked_input, self.scenario.pii)))
        placeholders = self.log.placeholders()

        def rendered(chunks: list[list[ReplySegment]]) -> list[str]:
            return [render_upstream_chunk(c, placeholders) for c in chunks]

        reply = script.reply
        if isinstance(reply, TextReply):
            return TextScript(rendered(reply.chunks))
        if isinstance(reply, ToolCallReply):
            return ToolCallScript(reply.call_id, reply.name, rendered(reply.argument_chunks))
        return HttpErrorScript(reply.status, reply.body)
