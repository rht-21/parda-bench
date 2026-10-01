"""Test worker: behaves according to the text it is asked to detect in."""

import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[5] / "sdk" / "python" / "src"))

from parda_sdk import Adapter, Entity, run  # noqa: E402


class Fake(Adapter):
    tool_version = "0.0.0"

    def capabilities(self) -> list[str]:
        caps = super().capabilities()
        return caps + ["mask"] if os.environ.get("FAKE_EXTRA_CAPABILITY") else caps

    def detect(self, text: str) -> list[Entity]:
        if text == "crash":
            os._exit(3)
        if text == "garbage":
            sys.__stdout__.write("this is not json\n")
            sys.__stdout__.flush()
            return []
        if text == "hang":
            time.sleep(30)
        if text == "error":
            raise RuntimeError("tool rejected input")
        return [Entity(0, len(text), "WORD"), Entity(0, 1, "INITIAL")]


run(Fake())
