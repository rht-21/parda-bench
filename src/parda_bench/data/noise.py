"""Realistic damage to the text around entities: OCR confusions, lost or doubled spaces, dropped punctuation.
Entity values are never touched, and each text keeps its first and last character so the separators next to an
entity survive."""

from parda_bench.data.rng import DataRng

OCR_SWAP = 0.05
DROP_SPACE = 0.08
DOUBLE_SPACE = 0.04
DROP_PUNCTUATION = 0.2
OCR_TWINS = {"O": "0", "o": "0", "l": "1", "I": "1", "S": "5", "B": "8", "e": "c"}
"""Characters a scanner or a hurried typist confuses."""


def add_noise(text: str, rng: DataRng) -> str:
    out: list[str] = []
    last = len(text) - 1
    for i, c in enumerate(text):
        if i in (0, last):
            out.append(c)
        elif c == " ":
            if rng.chance(DROP_SPACE):
                continue
            out.append("  " if rng.chance(DOUBLE_SPACE) else c)
        elif c in ".,;:" and rng.chance(DROP_PUNCTUATION):
            continue
        elif c in OCR_TWINS and rng.chance(OCR_SWAP):
            out.append(OCR_TWINS[c])
        else:
            out.append(c)
    return "".join(out)
