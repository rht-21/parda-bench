//! Incremental server-sent-events parser for streamed responses.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub name: Option<String>,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    /// Feeds raw text and returns every event completed by it.
    pub fn push(&mut self, text: &str) -> Vec<Event> {
        self.buffer.push_str(&text.replace('\r', ""));
        let mut events = Vec::new();
        while let Some(end) = self.buffer.find("\n\n") {
            let block: String = self.buffer.drain(..end + 2).collect();
            let mut name = None;
            let mut data = Vec::new();
            for line in block.lines() {
                if let Some(v) = line.strip_prefix("event:") {
                    name = Some(v.trim().to_owned());
                } else if let Some(v) = line.strip_prefix("data:") {
                    data.push(v.strip_prefix(' ').unwrap_or(v));
                }
            }
            if name.is_some() || !data.is_empty() {
                events.push(Event {
                    name,
                    data: data.join("\n"),
                });
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_reads() {
        let mut p = SseParser::default();
        assert!(p.push("event: message_start\r\ndata: {\"a\"").is_empty());
        let events = p.push(":1}\r\n\r\ndata: [DONE]\n\n: comment\n\n");
        assert_eq!(
            events,
            vec![
                Event {
                    name: Some("message_start".to_owned()),
                    data: "{\"a\":1}".to_owned()
                },
                Event {
                    name: None,
                    data: "[DONE]".to_owned()
                },
            ]
        );
    }
}
