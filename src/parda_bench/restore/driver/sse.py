"""Incremental server-sent-events parser for streamed responses."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Event:
    name: str | None
    data: str


class SseParser:
    def __init__(self) -> None:
        self._buffer = ""

    def push(self, text: str) -> list[Event]:
        """Feeds raw text and returns every event completed by it."""
        self._buffer += text.replace("\r", "")
        events = []
        while "\n\n" in self._buffer:
            block, self._buffer = self._buffer.split("\n\n", 1)
            name = None
            data = []
            for line in block.split("\n"):
                if line.startswith("event:"):
                    name = line[6:].strip()
                elif line.startswith("data:"):
                    data.append(line[5:].removeprefix(" "))
            if name is not None or data:
                events.append(Event(name, "\n".join(data)))
        return events
