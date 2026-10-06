"""Reversible masking: per-session placeholder maps and a stream unmasker that holds back partial placeholders."""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from regex_baseline.detect import detect

# Long enough for any `[LABEL_n]` this tool emits; a longer unmatched `[` is ordinary text.
_MAX_PLACEHOLDER = 32


@dataclass
class Session:
    by_value: dict[tuple[str, str], str] = field(default_factory=dict)
    by_placeholder: dict[str, str] = field(default_factory=dict)
    counts: dict[str, int] = field(default_factory=dict)
    # Held-back text per stream; a proxy restores several streams (content, each tool call) at once.
    pending: dict[str, str] = field(default_factory=dict)

    def placeholder_for(self, label: str, value: str) -> str:
        key = (label, value)
        if key not in self.by_value:
            self.counts[label] = self.counts.get(label, 0) + 1
            placeholder = f"[{label}_{self.counts[label]}]"
            self.by_value[key] = placeholder
            self.by_placeholder[placeholder] = value
        return self.by_value[key]


class Vault:
    def __init__(self) -> None:
        self._sessions: dict[str, Session] = {}

    def _session(self, session: str) -> Session:
        return self._sessions.setdefault(session, Session())

    def mask(self, session: str, text: str) -> str:
        s = self._session(session)
        out: list[str] = []
        last = 0
        for m in detect(text):
            out.append(text[last:m.start])
            out.append(s.placeholder_for(m.label, text[m.start:m.end]))
            last = m.end
        out.append(text[last:])
        return "".join(out)

    def unmask(self, session: str, text: str) -> str:
        known = self._session(session).by_placeholder
        if not known:
            return text
        pattern = re.compile("|".join(re.escape(p) for p in sorted(known, key=len, reverse=True)))
        return pattern.sub(lambda m: known[m.group()], text)

    def unmask_stream(self, session: str, chunk: str, is_final: bool, stream: str = "") -> str:
        s = self._session(session)
        buffered = s.pending.pop(stream, "") + chunk
        hold_from = len(buffered)
        if not is_final:
            open_at = buffered.rfind("[")
            if open_at != -1 and "]" not in buffered[open_at:] and len(buffered) - open_at < _MAX_PLACEHOLDER:
                hold_from = open_at
        if hold_from < len(buffered):
            s.pending[stream] = buffered[hold_from:]
        return self.unmask(session, buffered[:hold_from])
