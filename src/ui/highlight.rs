use ratatui::prelude::*;

pub fn highlight_text(text: &str, needle: Option<&str>) -> Text<'static> {
    let Some(needle) = needle.filter(|s| !s.is_empty()) else {
        return Text::raw(text.to_string());
    };

    let mut lines = Vec::new();

    for line in text.lines() {
        let mut spans = Vec::new();
        let mut rest = line;

        while let Some(pos) = rest.find(needle) {
            if pos > 0 {
                spans.push(Span::raw(rest[..pos].to_string()));
            }

            spans.push(Span::styled(
                rest[pos..pos + needle.len()].to_string(),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

            rest = &rest[pos + needle.len()..];
        }

        if !rest.is_empty() {
            spans.push(Span::raw(rest.to_string()));
        }

        lines.push(Line::from(spans));
    }

    Text::from(lines)
}
