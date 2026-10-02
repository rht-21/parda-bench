"""Parda Bench adapter for LangChain's `PresidioReversibleAnonymizer`, used as a chain would: mask before the
model call, deanonymize after. It replaces values with realistic fake ones rather than placeholder tokens."""

from __future__ import annotations

import importlib.metadata

from langchain_experimental.data_anonymizer import PresidioReversibleAnonymizer
from parda_sdk import Adapter, run

# Fixed so repeated runs pick the same fake values; it changes which fakes appear, not what gets masked.
FAKER_SEED = 42


class LangChainPresidio(Adapter):
    tool_version = importlib.metadata.version("langchain-experimental")

    def __init__(self) -> None:
        # One instance: each one loads a spaCy model. The harness runs one conversation at a time, so a new
        # session id starts a fresh mapping on the same instance.
        self.anonymizer = PresidioReversibleAnonymizer(faker_seed=FAKER_SEED)
        self.session = ""

    def mask(self, session: str, text: str) -> str:
        if session != self.session:
            self.anonymizer.reset_deanonymizer_mapping()
            self.session = session
        return self.anonymizer.anonymize(text)

    def unmask(self, session: str, text: str) -> str:
        if session != self.session:
            raise ValueError(f"unmask for session {session!r}, but the active session is {self.session!r}")
        return self.anonymizer.deanonymize(text)


if __name__ == "__main__":
    run(LangChainPresidio())
