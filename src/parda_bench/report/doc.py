"""A small document model rendered to both Markdown and standalone HTML."""

import html
from dataclasses import dataclass, field


@dataclass(frozen=True)
class Heading:
    level: int
    text: str


@dataclass(frozen=True)
class Paragraph:
    text: str


@dataclass(frozen=True)
class Table:
    headers: list[str]
    rows: list[list[str]]


@dataclass(frozen=True)
class Bullets:
    items: list[str]


Block = Heading | Paragraph | Table | Bullets

STYLE = (
    ":root{--bg:#fff;--fg:#1d1d1f;--muted:#6e6e73;--line:#e3e3e8;--head:#f5f5f7}"
    "@media (prefers-color-scheme:dark){:root{--bg:#141416;--fg:#ececf0;--muted:#a1a1a8;--line:#2c2c31;--head:#1d1d21}}"
    "body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,sans-serif}"
    "main{max-width:1100px;margin:0 auto;padding:24px 16px}"
    "h1{font-size:28px}h2{margin-top:40px;border-bottom:1px solid var(--line);padding-bottom:6px}"
    "p{color:var(--muted)}.scroll{overflow-x:auto}"
    "table{border-collapse:collapse;font-variant-numeric:tabular-nums;font-size:14px;margin:12px 0}"
    "th,td{border:1px solid var(--line);padding:4px 10px;text-align:left;white-space:nowrap}"
    "th{background:var(--head)}"
)


def _md_cell(s: str) -> str:
    """Keeps a value on one Markdown table line and stops it from closing the cell early."""
    return s.replace("|", "\\|").replace("\n", " ")


def _tagged(tag: str, items: list[str]) -> str:
    return "".join(f"<{tag}>{html.escape(i)}</{tag}>" for i in items)


@dataclass
class Doc:
    title: str
    blocks: list[Block] = field(default_factory=list)

    def heading(self, level: int, text: str) -> None:
        self.blocks.append(Heading(level, text))

    def paragraph(self, text: str) -> None:
        self.blocks.append(Paragraph(text))

    def table(self, headers: list[str], rows: list[list[str]]) -> None:
        self.blocks.append(Table(headers, rows))

    def bullets(self, items: list[str]) -> None:
        self.blocks.append(Bullets(items))

    def to_markdown(self) -> str:
        lines = [f"# {self.title}"]
        for block in self.blocks:
            lines.append("")
            if isinstance(block, Heading):
                lines.append(f"{'#' * block.level} {block.text}")
            elif isinstance(block, Paragraph):
                lines.append(block.text)
            elif isinstance(block, Bullets):
                lines += [f"- {_md_cell(i)}" for i in block.items]
            else:
                lines.append("| " + " | ".join(map(_md_cell, block.headers)) + " |")
                lines.append("|" + "---|" * len(block.headers))
                lines += ["| " + " | ".join(map(_md_cell, row)) + " |" for row in block.rows]
        return "\n".join(lines) + "\n"

    def to_html(self) -> str:
        parts = [f"<h1>{html.escape(self.title)}</h1>"]
        for block in self.blocks:
            if isinstance(block, Heading):
                level = min(max(block.level, 1), 6)
                parts.append(f"<h{level}>{html.escape(block.text)}</h{level}>")
            elif isinstance(block, Paragraph):
                parts.append(f"<p>{html.escape(block.text)}</p>")
            elif isinstance(block, Bullets):
                parts.append(f"<ul>{_tagged('li', block.items)}</ul>")
            else:
                body = "".join(f"<tr>{_tagged('td', row)}</tr>\n" for row in block.rows)
                parts.append(
                    f'<div class="scroll"><table>\n<thead><tr>{_tagged("th", block.headers)}</tr></thead>\n'
                    f"<tbody>\n{body}</tbody></table></div>"
                )
        return (
            '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
            f"<title>{html.escape(self.title)}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n"
            + "\n".join(parts)
            + "\n</main>\n</body>\n</html>\n"
        )
