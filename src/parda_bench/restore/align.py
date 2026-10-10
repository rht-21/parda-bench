"""Placeholder discovery and scripted-reply rendering.

The mock never assumes a placeholder format. It finds each slot's placeholder by anchoring the scenario's literal
input segments in the masked text the tool sent: whatever sits where a slot's value was is the tool's placeholder.
"""

from parda_bench.spec.scenario import (
    InputSegment,
    LiteralInput,
    LiteralReply,
    Mangle,
    ReplySegment,
)

SlotText = tuple[int, str]
"""One slot occurrence and the text the tool put in its place."""


class AlignError(ValueError):
    pass


def _script(segments: list[InputSegment]) -> tuple[str, list[tuple[int, str]]]:
    """A leading literal, then each slot with the literal that follows it (empty only after the last)."""
    lead = ""
    slots: list[tuple[int, str]] = []
    for seg in segments:
        if isinstance(seg, LiteralInput):
            if not seg.text:
                continue
            if slots:
                slots[-1] = (slots[-1][0], seg.text)
            else:
                lead = seg.text
        else:
            if slots and slots[-1][1] == "":
                raise AlignError(
                    f"slots {slots[-1][0]} and {seg.slot} are adjacent with no literal text between them, "
                    "so they cannot be told apart"
                )
            slots.append((seg.slot, ""))
    return lead, slots


def align(segments: list[InputSegment], masked: str, pii: list[str]) -> list[SlotText]:
    """Splits `masked` into the scenario's literals and per-slot replacements, in input order.

    A slot's text runs to an occurrence of the following literal such that the rest of the input still matches;
    the earliest such occurrence wins. A value that reached the upstream unmasked is preferred whole, even when it
    contains that literal (`2345 6789 0124` before `" "`). Raises `AlignError` when nothing matches.
    """
    lead, slots = _script(segments)
    found = _search(slots, masked[len(lead) :], pii) if masked.startswith(lead) else None
    if found is None:
        raise AlignError(f"the masked text does not contain the script's literal text in order: {masked!r}")
    for slot, text in found:
        if not text:
            raise AlignError(f"slot {slot} was replaced by nothing")
    return found


def _search(slots: list[tuple[int, str]], rest: str, pii: list[str]) -> list[SlotText] | None:
    if not slots:
        return [] if not rest else None
    (slot, literal), tail = slots[0], slots[1:]
    if not literal:
        return [(slot, rest)]
    for at in _split_points(rest, literal, pii[slot] if slot < len(pii) else None):
        found = _search(tail, rest[at + len(literal) :], pii)
        if found is not None:
            return [(slot, rest[:at]), *found]
    return None


def _split_points(rest: str, literal: str, unmasked: str | None) -> list[int]:
    """Where a slot's text may end: the unmasked value first, then every occurrence of `literal`, empty last."""
    whole = [len(unmasked)] if unmasked and rest.startswith(unmasked + literal) else []
    occurrences = [i for i in range(1, len(rest)) if rest.startswith(literal, i)]
    empty = [0] if rest.startswith(literal) else []
    return whole + occurrences + empty


def render_input(segments: list[InputSegment], pii: list[str]) -> str:
    """The real text of an input: literals with slots replaced by their values."""
    return "".join(s.text if isinstance(s, LiteralInput) else pii[s.slot] for s in segments)


def render_upstream_chunk(segments: list[ReplySegment], placeholders: dict[int, str]) -> str:
    """What the mock sends for one chunk; raises `AlignError` for a slot whose placeholder was never learned."""
    out = []
    for s in segments:
        if isinstance(s, LiteralReply):
            out.append(s.text)
            continue
        if s.slot not in placeholders:
            raise AlignError(f"no placeholder for slot {s.slot}")
        placeholder = placeholders[s.slot]
        shaped = apply_mangle(placeholder, s.mangle) if s.mangle else placeholder
        end = len(shaped) if s.to is None else min(s.to, len(shaped))
        out.append(shaped[min(s.from_ or 0, end) : end])
    return "".join(out)


def render_expected(chunks: list[list[ReplySegment]], pii: list[str]) -> str:
    """What the client should see: each slot's real value once, at its first fragment."""
    return "".join(
        s.text if isinstance(s, LiteralReply) else (pii[s.slot] if not s.from_ else "")
        for chunk in chunks
        for s in chunk
    )


def core(placeholder: str) -> tuple[str, str, str]:
    """Splits a placeholder into leading delimiters, trailing delimiters, and its alphanumeric core."""
    alnum = [i for i, c in enumerate(placeholder) if c.isalnum()]
    if not alnum:
        return placeholder, "", ""
    start, end = alnum[0], alnum[-1] + 1
    return placeholder[:start], placeholder[end:], placeholder[start:end]


def apply_mangle(placeholder: str, mangle: Mangle) -> str:
    """Alters a placeholder the way language models do when copying it."""
    if mangle is Mangle.LOWERCASE:
        return placeholder.lower()
    if mangle is Mangle.UPPERCASE:
        return placeholder.upper()
    opening, closing, inner = core(placeholder)
    if mangle is Mangle.STRIP_DELIMITERS:
        return inner
    return f" {inner} " if not opening and not closing else f"{opening} {inner} {closing}"
