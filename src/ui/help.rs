use std::cell::Cell;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

/// The keys, by section; `{esc}` follows the esc_clears_search config option
const SECTIONS: [(&str, &[(&str, &str)]); 4] = [
    ("NODES", &[
        ("← → h l Tab", "Focus previous / next node"),
        ("click", "Focus a node; double-click to zoom"),
        ("m", "Mouse off: drag selects text (m again: on)"),
        ("Shift+drag", "Select with the mouse on (iTerm2: Option)"),
        ("Enter z", "Zoom into the focused node"),
        ("Esc", "{esc}"),
        ("1-6", "Hide or show node 0 to 5"),
    ]),
    ("SCROLL (freezes the focused pane; others follow)", &[
        ("↑ ↓ j k", "One line"),
        ("PgUp PgDn", "One page (also Ctrl+b / Ctrl+f)"),
        ("Ctrl+u Ctrl+d", "Half a page"),
        ("Home End", "Top / bottom and follow (also gg / G)"),
        ("wheel", "Scroll the node under the pointer"),
        ("F", "Follow the logs again"),
        ("Space", "Resume frozen panes, else freeze all"),
    ]),
    ("SEARCH", &[
        ("/", "Highlight terms, comma-separated (up to 6)"),
        ("", "in the box: Ctrl+w drops a term, Ctrl+u all"),
        ("n N", "Next / previous match"),
        ("Ctrl+g", "Filter: only matching lines / show all"),
        ("c", "Clear the search and the filter"),
    ]),
    ("NETWORK (focused node)", &[
        ("s", "Stop / continue (SIGSTOP)"),
        ("i", "Isolate / heal"),
        ("p", "Split the network here / heal"),
        ("r", "Restart"),
        ("x", "Shut down all nodes (their logs stay)"),
        ("q", "Quit"),
        ("?", "This help"),
    ]),
];

/// Width of one column of keys: 2 indent, 15 key, 44 description
const COLUMN: u16 = 61;

/// The keys screen: one column, two side by side when one does not fit the height, and
/// scrolled from `scroll` when neither fits
pub fn render(f: &mut Frame, esc_clears_search: bool, scroll: &Cell<u16>) {
    let area = f.size();
    let esc = match esc_clears_search {
        true => "Filter off, then clear search, then grid",
        false => "Back to the grid / cancel",
    };
    let lines = |sections: &[(&str, &[(&str, &str)])]| -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for (title, keys) in sections {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.push(Line::styled(title.to_string(), Style::default().fg(Color::Cyan).bold()));
            for (key, what) in keys.iter() {
                lines.push(Line::from(format!("  {key:<15}{}", what.replace("{esc}", esc))));
            }
        }
        lines
    };
    let one = lines(&SECTIONS);
    let two = [lines(&SECTIONS[..2]), lines(&SECTIONS[2..])];
    let footer = 2; // borders
    let fits = |n: usize| n as u16 + footer <= area.height;
    let columns: Vec<Vec<Line>> = match (fits(one.len()), area.width >= 2 * COLUMN + 4) {
        (false, true) => two.to_vec(),
        _ => vec![one],
    };
    let tallest = columns.iter().map(|c| c.len()).max().unwrap_or(0) as u16;

    let width = (columns.len() as u16 * COLUMN + 2).min(area.width);
    let height = (tallest + footer).min(area.height);
    let popup = Rect {
        x: (area.width - width) / 2,
        y: (area.height - height) / 2,
        width,
        height,
    };
    let last = tallest.saturating_sub(height - footer);
    scroll.set(scroll.get().min(last));
    let title = match last {
        0 => " Keys: any key closes ".to_string(),
        _ => " Keys: ↑↓ scroll, any other key closes ".to_string(),
    };

    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let parts = Layout::horizontal(vec![Constraint::Length(COLUMN); columns.len()]).split(inner);
    for (column, part) in columns.into_iter().zip(parts.iter()) {
        f.render_widget(Paragraph::new(column).scroll((scroll.get(), 0)), *part);
    }
}
