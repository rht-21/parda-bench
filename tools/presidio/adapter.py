"""Parda Bench adapter for Presidio: `detect` via AnalyzerEngine, `mask` via AnonymizerEngine (irreversible)."""

from __future__ import annotations

import importlib.metadata

from parda_sdk import Adapter, Entity, run
from presidio_analyzer import AnalyzerEngine
from presidio_anonymizer import AnonymizerEngine

# Presidio ships English models only; Hindi and Hinglish text is analyzed as English, as a user would have to.
LANGUAGE = "en"


class Presidio(Adapter):
    tool_version = importlib.metadata.version("presidio-analyzer")

    def __init__(self) -> None:
        self.analyzer = AnalyzerEngine()
        self.anonymizer = AnonymizerEngine()

    def detect(self, text: str) -> list[Entity]:
        return [Entity(r.start, r.end, r.entity_type, r.score) for r in self.analyzer.analyze(text=text, language=LANGUAGE)]

    def mask(self, session: str, text: str) -> str:
        results = self.analyzer.analyze(text=text, language=LANGUAGE)
        return self.anonymizer.anonymize(text=text, analyzer_results=results).text


if __name__ == "__main__":
    run(Presidio())
