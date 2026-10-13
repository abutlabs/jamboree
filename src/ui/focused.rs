use portable_pty::PtySize;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::AppState;
use crate::node::Node;
use crate::ui::highlight::highlight_text;
use crate::ui::menus;
use crate::ui::tags;

pub fn render(f: &mut Frame, node: &Node, app: &AppState) {
    // Layout: menu bar, optional tags bar, content
    let has_tags = app.highlight.is_some() && !app.highlight.as_ref().unwrap().is_empty();
    let constraints = if has_tags {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ]
    } else {
        vec![Constraint::Length(1), Constraint::Min(0)]
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.size());

    let menu_text = menus::focused_menu(app, has_tags);

    let menu =
        Paragraph::new(menu_text).style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(menu, layout[0]);

    // ---- TAGS ----
    if has_tags {
        tags::render_tags(f, layout[1], app);
    }

    let area = if has_tags { layout[2] } else { layout[1] };

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

    let raw_text = node.get_text_tail(5_000_000);

    // Apply grep filter if enabled
    let text = app.filter_text(&raw_text);

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

    let max_scroll = total_wrapped_lines.saturating_sub(visible_lines).min(10000) as u16;

    let scroll_y = if app.paused || app.log_viewer_mode {
        // Clamp stored scroll to actual max and update stored value
        let stored = app.frozen_scroll.borrow()[node.id];
        let clamped = stored.min(max_scroll);
        if clamped != stored {
            app.frozen_scroll.borrow_mut()[node.id] = clamped;
        }
        clamped
    } else {
        app.frozen_scroll.borrow_mut()[node.id] = max_scroll;
        max_scroll
    };
    let highlighted_text = highlight_text(&text, app.highlight.as_ref().map(|v| v.as_slice()));
    let widget = Paragraph::new(highlighted_text)
        .scroll((scroll_y, 0))
        .block(
            Block::default()
                .title(format!(
                    "{} {} (ZOOM){}",
                    node.label,
                    node.id,
                    if paused { " [PAUSED]" } else { "" }
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(widget, area);
}
