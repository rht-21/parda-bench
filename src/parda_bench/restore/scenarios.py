"""Loading `scenarios/<category>/<id>.toml` and rejecting scenarios the mock could not run."""

import itertools
import tomllib
from pathlib import Path

from pydantic import ValidationError

from parda_bench.spec.scenario import (
    CheckKind,
    HttpErrorReply,
    InputSegment,
    LiteralInput,
    Scenario,
    SlotInput,
    SlotReply,
    TextReply,
    ToolCallReply,
    ToolResultInput,
    segments,
)


class ScenarioError(ValueError):
    pass


def load_dir(root: Path) -> list[Scenario]:
    """All scenarios under `root`, sorted by id; raises `ScenarioError` naming the file of the first bad one."""
    scenarios: list[Scenario] = []
    for path in sorted(root.glob("*/*.toml")):
        try:
            scenario = Scenario.model_validate(tomllib.loads(path.read_text(encoding="utf-8")))
            problem = _location_problem(scenario, path) or check(scenario)
        except (tomllib.TOMLDecodeError, ValidationError) as e:
            problem = str(e)
        if problem:
            raise ScenarioError(f"{path}: {problem}")
        scenarios.append(scenario)
    scenarios.sort(key=lambda s: s.id)
    for a, b in itertools.pairwise(scenarios):
        if a.id == b.id:
            raise ScenarioError(f"{root}: duplicate scenario id {a.id}")
    return scenarios


def check(s: Scenario) -> str | None:
    """The first rule that makes the scenario unrunnable or its expectations ill-defined, or `None`."""
    if not s.turns or not s.checks or not s.pii:
        return "a scenario needs at least one turn, one check and one pii value"
    if any(not v.strip() for v in s.pii):
        return "a pii value is empty"
    introduced: set[int] = set()
    open_call: str | None = None
    for t, turn in enumerate(s.turns):
        if isinstance(turn.input, ToolResultInput) and turn.input.call_id != open_call:
            return f"turn {t}: tool result {turn.input.call_id} does not answer the previous turn's call"
        problem = _input_problem(segments(turn.input), len(s.pii))
        if problem:
            return f"turn {t}: {problem}"
        introduced |= {seg.slot for seg in segments(turn.input) if isinstance(seg, SlotInput)}
        open_call = None
        reply = turn.reply
        if isinstance(reply, HttpErrorReply):
            if not 400 <= reply.status <= 599:
                return f"turn {t}: http_error status {reply.status} is not a 4xx or 5xx code"
            continue
        if isinstance(reply, ToolCallReply):
            if reply.name not in {tool.name for tool in s.tools}:
                return f"turn {t}: tool call to {reply.name}, which is not in `tools`"
            open_call = reply.call_id
        chunks = reply.chunks if isinstance(reply, TextReply) else reply.argument_chunks
        unseen = {seg.slot for chunk in chunks for seg in chunk if isinstance(seg, SlotReply)} - introduced
        if unseen:
            return f"turn {t}: reply uses slot {min(unseen)} before any input contains it"
    has_error_reply = any(isinstance(turn.reply, HttpErrorReply) for turn in s.turns)
    if (CheckKind.FAILS_CLOSED in s.checks) != has_error_reply:
        return "`fails_closed` goes with, and only with, an `http_error` reply"
    return None


def _input_problem(segs: list[InputSegment], pii_len: int) -> str | None:
    previous_was_slot = False
    for seg in segs:
        if isinstance(seg, LiteralInput):
            previous_was_slot &= not seg.text
            continue
        if seg.slot >= pii_len:
            return f"slot {seg.slot} has no pii value"
        if previous_was_slot:
            return "two slots need literal text between them for the mock to tell them apart"
        previous_was_slot = True
    return None


def _location_problem(s: Scenario, path: Path) -> str | None:
    if path.stem != s.id:
        return f"id {s.id} does not match file name {path.stem}"
    if path.parent.name != s.category.value:
        return f"category {s.category.value} does not match directory {path.parent.name}"
    return None
