use portable_pty::PtySize;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::AppState;
use crate::node::Node;
use crate::ui::highlight::highlight_text;
pub fn render(f: &mut Frame, nodes: &[Node], focused: usize, app: &AppState) {
    let visible: Vec<(usize, &Node)> = nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !app.hidden[*i])
        .collect();

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(f.size());

    if visible.is_empty() {
        f.render_widget(
            Paragraph::new("No nodes visible").block(Block::default().borders(Borders::ALL)),
            layout[1],
        );
        return;
    }

    // ---- MENU BAR ----
    let menu_text = if app.search_mode {
        format!(
            " Search: {}_  [Enter] Apply  [Esc] Cancel ",
            app.search_input
        )
    } else {
        format!(
            " [0-5] Toggle Node Visibility  [Tab] Focus  [/] Search  [Space] Pause All  [Enter] Zoom  [Q] Quit{}",
            if app.paused { "  [PAUSED]" } else { "" }
        )
    };

    let menu = Paragraph::new(menu_text).style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_widget(menu, layout[0]);

    // ---- GRID ----

    let count = visible.len();
    let cols = (count as f64).sqrt().ceil() as usize;
    let rows = (count + cols - 1) / cols;

    let row_constraints = vec![Constraint::Percentage(100 / rows as u16); rows];
    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(layout[1]);

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
            let text = node.get_text_tail(50_000);

            // ---- SCROLL LOGIC (EXACT, NO WRAP) ----
            let line_count = text.bytes().filter(|&b| b == b'\n').count();

            let scroll_y = if app.paused {
                app.frozen_scroll.borrow()[node_idx]
            } else {
                let s = line_count.saturating_sub(visible_lines).min(10_000) as u16;
                app.frozen_scroll.borrow_mut()[node_idx] = s;
                s
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
