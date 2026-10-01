from __future__ import annotations

from parda_sdk import Adapter, Entity, run

from regex_baseline.detect import detect
from regex_baseline.vault import Vault


class RegexBaseline(Adapter):
    tool_version = "0.1.0"

    def __init__(self) -> None:
        self.vault = Vault()

    def detect(self, text: str) -> list[Entity]:
        return [Entity(m.start, m.end, m.label) for m in detect(text)]

    def mask(self, session: str, text: str) -> str:
        return self.vault.mask(session, text)

    def unmask(self, session: str, text: str) -> str:
        return self.vault.unmask(session, text)

    def unmask_stream(self, session: str, chunk: str, is_final: bool) -> str:
        return self.vault.unmask_stream(session, chunk, is_final)


if __name__ == "__main__":
    run(RegexBaseline())
