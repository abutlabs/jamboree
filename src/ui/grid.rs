use portable_pty::PtySize;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::AppState;
use crate::node::Node;
use crate::ui::highlight::highlight_text;
use crate::ui::menus;
use crate::ui::tags;
pub fn render(f: &mut Frame, nodes: &[Node], focused: usize, app: &AppState) {
    let visible: Vec<(usize, &Node)> = nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !app.hidden[*i])
        .collect();

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

    if visible.is_empty() {
        f.render_widget(
            Paragraph::new("No nodes visible").block(Block::default().borders(Borders::ALL)),
            layout[1],
        );
        return;
    }

    // ---- MENU BAR ----
    let menu_text = menus::grid_menu(app, has_tags);

    let menu = Paragraph::new(menu_text).style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_widget(menu, layout[0]);

    // ---- TAGS ----
    if has_tags {
        tags::render_tags(f, layout[1], app);
    }

    // ---- GRID ----
    let grid_area = if has_tags { layout[2] } else { layout[1] };

    let count = visible.len();
    let cols = (count as f64).sqrt().ceil() as usize;
    let rows = count.div_ceil(cols);

    let row_constraints = vec![Constraint::Percentage(100 / rows as u16); rows];
    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(grid_area);

    for r in 0..rows {
        let col_constraints = vec![Constraint::Percentage(100 / cols as u16); cols];
        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints)
            .split(row_chunks[r]);

        for c in 0..cols {
            let idx = r * cols + c;
            if idx >= visible.len() {
                break;
            }

            let (node_idx, node) = visible[idx];
            let area = col_chunks[c];

            // ---- RESIZE PTY (ONLY IF CHANGED) ----
            let new_size = PtySize {
                rows: area.height.max(1) as u16,
                cols: area.width.max(1) as u16,
                pixel_width: 0,
                pixel_height: 0,
            };

            // Resize PTY if needed
            node.resize_pty_if_needed(new_size);

            // Extract tail text
            let visible_lines = area.height.saturating_sub(2) as usize;
            let raw_text = node.get_text_tail(5_000_000);

            // Apply grep filter if enabled
            let text = app.filter_text(&raw_text);

            // ---- SCROLL LOGIC (EXACT, NO WRAP) ----
            let line_count = text.bytes().filter(|&b| b == b'\n').count();
            let max_scroll = line_count.saturating_sub(visible_lines).min(10_000) as u16;

            let scroll_y = if app.paused || app.log_viewer_mode {
                // Clamp stored scroll to actual max and update stored value
                let stored = app.frozen_scroll.borrow()[node_idx];
                let clamped = stored.min(max_scroll);
                if clamped != stored {
                    app.frozen_scroll.borrow_mut()[node_idx] = clamped;
                }
                clamped
            } else {
                app.frozen_scroll.borrow_mut()[node_idx] = max_scroll;
                max_scroll
            };

            // ---- HIGHLIGHT ----
            let content = highlight_text(&text, app.highlight.as_deref());

            // ---- RENDER ----
            let widget = Paragraph::new(content).scroll((scroll_y, 0)).block(
                Block::default()
                    .title(format!("Node {}", node_idx))
                    .borders(Borders::ALL)
                    .border_style(if focused == node_idx {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }),
            );

            f.render_widget(widget, area);
        }
    }
}
