//! A small document model rendered to both Markdown and standalone HTML.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, text: String },
    Paragraph(String),
    Table(Table),
    List(Vec<String>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub title: String,
    pub blocks: Vec<Block>,
}

impl Doc {
    pub fn heading(&mut self, level: u8, text: impl Into<String>) {
        self.blocks.push(Block::Heading {
            level,
            text: text.into(),
        });
    }

    pub fn paragraph(&mut self, text: impl Into<String>) {
        self.blocks.push(Block::Paragraph(text.into()));
    }

    pub fn table(&mut self, headers: &[&str], rows: Vec<Vec<String>>) {
        let headers = headers.iter().map(|h| (*h).to_owned()).collect();
        self.blocks.push(Block::Table(Table { headers, rows }));
    }

    pub fn list(&mut self, items: Vec<String>) {
        self.blocks.push(Block::List(items));
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![format!("# {}", self.title)];
        for block in &self.blocks {
            lines.push(String::new());
            match block {
                Block::Heading { level, text } => {
                    lines.push(format!("{} {text}", "#".repeat(usize::from(*level))));
                }
                Block::Paragraph(text) => lines.push(text.clone()),
                Block::List(items) => {
                    lines.extend(items.iter().map(|i| format!("- {}", md_cell(i))));
                }
                Block::Table(t) => {
                    lines.push(md_row(&t.headers));
                    lines.push(format!("|{}", "---|".repeat(t.headers.len())));
                    lines.extend(t.rows.iter().map(|r| md_row(r)));
                }
            }
        }
        lines.push(String::new());
        lines.join("\n")
    }

    #[must_use]
    pub fn to_html(&self) -> String {
        let mut parts = vec![format!("<h1>{}</h1>", esc(&self.title))];
        for block in &self.blocks {
            match block {
                Block::Heading { level, text } => {
                    let level = (*level).clamp(1, 6);
                    parts.push(format!("<h{level}>{}</h{level}>", esc(text)));
                }
                Block::Paragraph(text) => parts.push(format!("<p>{}</p>", esc(text))),
                Block::List(items) => {
                    let items = tagged("li", items);
                    parts.push(format!("<ul>{items}</ul>"));
                }
                Block::Table(t) => parts.push(html_table(t)),
            }
        }
        format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n\
             <style>{STYLE}</style>\n</head>\n<body>\n<main>\n{}\n</main>\n</body>\n</html>\n",
            esc(&self.title),
            parts.join("\n")
        )
    }
}

fn html_table(t: &Table) -> String {
    let body: Vec<String> = t
        .rows
        .iter()
        .map(|row| format!("<tr>{}</tr>\n", tagged("td", row)))
        .collect();
    format!(
        "<div class=\"scroll\"><table>\n<thead><tr>{}</tr></thead>\n<tbody>\n{}</tbody></table></div>",
        tagged("th", &t.headers),
        body.concat()
    )
}

/// Each item escaped and wrapped in `<tag>…</tag>`, concatenated.
fn tagged(tag: &str, items: &[String]) -> String {
    let wrapped: Vec<String> = items
        .iter()
        .map(|i| format!("<{tag}>{}</{tag}>", esc(i)))
        .collect();
    wrapped.concat()
}

const STYLE: &str = ":root{--bg:#fff;--fg:#1d1d1f;--muted:#6e6e73;--line:#e3e3e8;--head:#f5f5f7}\
@media (prefers-color-scheme:dark){:root{--bg:#141416;--fg:#ececf0;--muted:#a1a1a8;--line:#2c2c31;--head:#1d1d21}}\
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,sans-serif}\
main{max-width:1100px;margin:0 auto;padding:24px 16px}\
h1{font-size:28px}h2{margin-top:40px;border-bottom:1px solid var(--line);padding-bottom:6px}\
p{color:var(--muted)}.scroll{overflow-x:auto}\
table{border-collapse:collapse;font-variant-numeric:tabular-nums;font-size:14px;margin:12px 0}\
th,td{border:1px solid var(--line);padding:4px 10px;text-align:left;white-space:nowrap}\
th{background:var(--head)}";

fn md_row(cells: &[String]) -> String {
    let cells: Vec<String> = cells.iter().map(|c| md_cell(c)).collect();
    format!("| {} |", cells.join(" | "))
}

/// Keeps a value on one Markdown table line and stops it from closing the cell early.
fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Doc {
        let mut d = Doc {
            title: "T".to_owned(),
            blocks: vec![],
        };
        d.heading(2, "Section");
        d.table(&["a", "b"], vec![vec!["x|y".to_owned(), "<z>".to_owned()]]);
        d
    }

    #[test]
    fn markdown_escapes_pipes_in_cells() {
        let md = sample().to_markdown();
        assert!(md.contains("## Section\n"));
        assert!(
            md.contains("| a | b |\n|---|---|\n| x\\|y | <z> |\n"),
            "{md}"
        );
    }

    #[test]
    fn html_escapes_markup() {
        let html = sample().to_html();
        assert!(html.contains("<td>&lt;z&gt;</td>"));
        assert!(!html.contains("<z>"));
    }
}
