"""Restore checks: compares what the upstream received and what the client saw against the scenario."""

import json
from dataclasses import dataclass
from typing import Any

from parda_bench.restore.align import core, render_expected
from parda_bench.restore.engine import UpstreamLog
from parda_bench.spec.protocol import Completed, Errored, Observation
from parda_bench.spec.record import CheckFailure, Fail, Pass, RestoreOutcome
from parda_bench.spec.scenario import CheckKind, HttpErrorReply, Scenario, TextReply, ToolCallReply

EXCERPT_CHARS = 600
MIN_FRAGMENT_CHARS = 3
"""Placeholder fragments shorter than this are too common in ordinary text to count."""
MIN_NORMALIZED_CHARS = 6
"""Values with at least this many letters and digits are also matched with separators removed."""


@dataclass(frozen=True)
class ExpectedText:
    text: str


@dataclass(frozen=True)
class ExpectedCall:
    name: str
    arguments: str


@dataclass(frozen=True)
class ExpectedError:
    status: int


Expected = ExpectedText | ExpectedCall | ExpectedError


def evaluate(scenario: Scenario, log: UpstreamLog, observations: list[Observation]) -> RestoreOutcome:
    expected = _expected_turns(scenario)
    failures = [
        CheckFailure(check=CheckKind.RESTORED_EXACT, detail=f"mock upstream could not follow the script: {e}")
        for e in log.errors
    ]
    if len(observations) != len(expected):
        failures.append(
            CheckFailure(
                check=CheckKind.RESTORED_EXACT,
                detail=f"client reported {len(observations)} turns, scenario has {len(expected)}",
            )
        )
    pairs = list(zip(expected, observations, strict=False))
    for check in scenario.checks:
        if check is CheckKind.NO_LEAK_UPSTREAM:
            details = _leaks(scenario.pii, log.payloads)
        elif check is CheckKind.RESTORED_EXACT:
            details = _restore_mismatches(pairs)
        elif check is CheckKind.NO_PLACEHOLDER_FRAGMENT:
            details = _fragments(pairs, log.placeholders(), scenario.pii)
        elif check is CheckKind.PLACEHOLDER_STABLE_ACROSS_TURNS:
            details = _unstable_placeholders(log)
        elif check is CheckKind.JSON_VALID:
            details = _invalid_json(pairs)
        else:
            details = _open_failures(pairs)
        failures += [CheckFailure(check=check, detail=d) for d in details]
    if not failures:
        return Pass()
    last_payload = compact(log.payloads[-1]) if log.payloads else ""
    client = repr([o.model_dump(mode="json") for o in observations])
    return Fail(failures=failures, upstream_excerpt=_excerpt(last_payload), client_excerpt=_excerpt(client))


def compact(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def _expected_turns(scenario: Scenario) -> list[Expected]:
    out: list[Expected] = []
    for turn in scenario.turns:
        reply = turn.reply
        if isinstance(reply, TextReply):
            out.append(ExpectedText(render_expected(reply.chunks, scenario.pii)))
        elif isinstance(reply, ToolCallReply):
            out.append(ExpectedCall(reply.name, render_expected(reply.argument_chunks, scenario.pii)))
        else:
            assert isinstance(reply, HttpErrorReply)
            out.append(ExpectedError(reply.status))
    return out


def _strings(value: Any) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [s for v in value for s in _strings(v)]
    if isinstance(value, dict):
        return [s for v in value.values() for s in _strings(v)]
    return []


def _alnum(s: str) -> str:
    return "".join(c for c in s if c.isalnum()).lower()


def _leaks(pii: list[str], payloads: list[Any]) -> list[str]:
    strings = [s for p in payloads for s in _strings(p)]
    normalized = [_alnum(s) for s in strings]
    leaked = []
    for i, value in enumerate(pii):
        norm = _alnum(value)
        if any(value in s for s in strings) or (
            len(norm) >= MIN_NORMALIZED_CHARS and any(norm in s for s in normalized)
        ):
            leaked.append(f"upstream received pii[{i}] {_quote(value)}")
    return leaked


def _same_json(a: str, b: str) -> bool:
    """JSON-equal when both sides parse (key order and whitespace may change in transit), byte-equal otherwise."""
    try:
        return bool(json.loads(a) == json.loads(b))
    except json.JSONDecodeError:
        return a == b


def _restore_mismatches(pairs: list[tuple[Expected, Observation]]) -> list[str]:
    out = []
    for turn, (exp, obs) in enumerate(pairs):
        if isinstance(exp, ExpectedError):
            continue
        if isinstance(obs, Errored):
            out.append(f"turn {turn}: client got an error ({obs.http_status}): {obs.message}")
        elif isinstance(exp, ExpectedText) and obs.text != exp.text:
            out.append(f"turn {turn}: expected {_quote(exp.text)}, client saw {_quote(obs.text)}")
        elif isinstance(exp, ExpectedCall):
            calls = obs.tool_calls
            if not (len(calls) == 1 and calls[0].name == exp.name and _same_json(calls[0].arguments, exp.arguments)):
                seen = [c.model_dump() for c in calls]
                out.append(f"turn {turn}: expected call {exp.name}({exp.arguments}), client saw {seen}")
    return out


def _observed_texts(obs: Observation) -> list[str]:
    if isinstance(obs, Completed):
        return [obs.text, *(c.arguments for c in obs.tool_calls)]
    return [obs.message]


def _expected_output(exp: Expected) -> str:
    if isinstance(exp, ExpectedText):
        return exp.text
    return exp.arguments if isinstance(exp, ExpectedCall) else ""


def _placeholder_fragments(placeholder: str) -> list[str]:
    """Lowercased pieces that betray a placeholder: the whole thing, its core, and each half."""
    p = placeholder.lower()
    half = (len(p) + 1) // 2
    candidates = [p, core(p)[2], p[:half], p[len(p) - half :]]
    unique = list(dict.fromkeys(candidates))
    return [c for c in unique if len(c) >= MIN_FRAGMENT_CHARS and any(ch.isalnum() for ch in c)]


def _fragments(pairs: list[tuple[Expected, Observation]], placeholders: dict[int, str], pii: list[str]) -> list[str]:
    masked = [p for slot, p in sorted(placeholders.items()) if slot >= len(pii) or pii[slot] != p]
    out = []
    for turn, (exp, obs) in enumerate(pairs):
        allowed = _expected_output(exp).lower()
        for seen in (t.lower() for t in _observed_texts(obs)):
            for placeholder in masked:
                fragment = next(
                    (f for f in _placeholder_fragments(placeholder) if f in seen and f not in allowed), None
                )
                if fragment is not None:
                    out.append(f"turn {turn}: client saw {_quote(fragment)} from placeholder {_quote(placeholder)}")
    return out


def _unstable_placeholders(log: UpstreamLog) -> list[str]:
    by_slot: dict[int, set[str]] = {}
    for _, slots in log.alignments:
        for slot, text in slots:
            by_slot.setdefault(slot, set()).add(text)
    return [
        f"slot {slot} was replaced inconsistently: {sorted(seen)}"
        for slot, seen in sorted(by_slot.items())
        if len(seen) > 1
    ]


def _parses(text: str) -> bool:
    try:
        json.loads(text)
    except json.JSONDecodeError:
        return False
    return True


def _invalid_json(pairs: list[tuple[Expected, Observation]]) -> list[str]:
    out = []
    for turn, (exp, obs) in enumerate(pairs):
        if not isinstance(obs, Completed):
            continue
        if isinstance(exp, ExpectedText):
            candidates = [obs.text] if _parses(exp.text) else []
        elif isinstance(exp, ExpectedCall):
            candidates = [c.arguments for c in obs.tool_calls]
        else:
            candidates = []
        out += [f"turn {turn}: not valid JSON: {_quote(c)}" for c in candidates if not _parses(c)]
    return out


def _open_failures(pairs: list[tuple[Expected, Observation]]) -> list[str]:
    out = []
    for turn, (exp, obs) in enumerate(pairs):
        if not isinstance(exp, ExpectedError):
            continue
        if isinstance(obs, Completed):
            out.append(f"turn {turn}: upstream failed with {exp.status}, client saw a completion {_quote(obs.text)}")
        elif obs.http_status is not None and obs.http_status != exp.status:
            out.append(f"turn {turn}: upstream failed with {exp.status}, client saw status {obs.http_status}")
    return out


def _quote(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def _excerpt(s: str) -> str:
    return s if len(s) <= EXCERPT_CHARS else s[:EXCERPT_CHARS] + "…"
