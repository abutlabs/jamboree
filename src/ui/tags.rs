use crate::AppState;
use crate::ui::colors::COLORS;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_tags(f: &mut Frame, area: Rect, app: &AppState) {
    if let Some(ref terms) = app.highlight {
        if terms.is_empty() {
            return;
        }

        let mut spans = Vec::new();
        spans.push(Span::styled(
            " Tags: ",
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ));

        for (term, color_idx) in terms {
            let (fg, bg) = COLORS[(*color_idx).min(5)];
            let tag_style = Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD);

            // Tag format: [term]
            spans.push(Span::styled(format!(" [{}] ", term), tag_style));
            spans.push(Span::raw(" ")); // Space between tags
        }

        let line = Line::from(spans);
        let widget = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
        f.render_widget(widget, area);
    }
}
