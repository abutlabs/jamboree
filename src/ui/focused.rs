use portable_pty::PtySize;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::AppState;
use crate::node::Node;
use crate::ui::highlight::highlight_text;

pub fn render(f: &mut Frame, node: &Node, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(f.size());

    let menu_text = if app.search_mode {
        format!(
            " Search: {}_  [Enter] Apply  [Esc] Cancel ",
            app.search_input
        )
    } else {
        let search_indicator = if let Some(ref term) = app.highlight {
            format!(" [Search: {}]", term)
        } else {
            String::new()
        };
        format!(
            " [Esc] Grid  [/] Search  [Space] Pause  [↑↓ PgUp PgDn] Scroll  [Q] Quit {}{}",
            if app.paused { " [PAUSED]" } else { "" },
            search_indicator
        )
    };

    let menu =
        Paragraph::new(menu_text).style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(menu, layout[0]);
    let area = layout[1];

    // Only resize PTY if dimensions changed (avoid expensive syscalls)
    let new_size = PtySize {
        rows: area.height.max(1) as u16,
        cols: area.width.max(1) as u16,
        pixel_width: 0,
        pixel_height: 0,
    };

    // Resize PTY if needed
    node.resize_pty_if_needed(new_size);

    // Fast tail extraction with wrap calculation for focused view
    let visible_lines = area.height.saturating_sub(2) as usize; // borders
    let available_width = area.width.saturating_sub(2).max(1) as usize;

    let text = node.get_text_tail(50_000);

    // Calculate wrapped lines for focused view (has wrapping enabled)
    // Only one node in focused view, so we can afford this calculation
    let total_wrapped_lines: usize = text
        .lines()
        .map(|line| {
            let line_len = line.chars().count();
            if line_len == 0 {
                1
            } else {
                (line_len + available_width - 1) / available_width
            }
        })
        .sum();

    let paused = *node.paused.lock().unwrap();

    let scroll_y = if app.paused {
        app.frozen_scroll.borrow()[node.id]
    } else {
        let s = total_wrapped_lines.saturating_sub(visible_lines).min(10000) as u16;
        app.frozen_scroll.borrow_mut()[node.id] = s;
        s
    };
    let highlighted_text = highlight_text(&text, app.highlight.as_deref());
    let widget = Paragraph::new(highlighted_text)
        .scroll((scroll_y, 0))
        .block(
            Block::default()
                .title(format!(
                    "Node {} (ZOOM){}",
                    node.id,
                    if paused { " [PAUSED]" } else { "" }
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(widget, area);
}
