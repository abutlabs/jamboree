use std::cell::Cell;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Clear},
};

/// The keys screen; the Esc line follows the esc_clears_search config option, and a terminal
/// too short for it scrolls it from `scroll`
pub fn render(f: &mut Frame, esc_clears_search: bool, scroll: &Cell<u16>) {
    let area = f.size();
    
    // Create a centered popup
    let popup_width = 70.min(area.width.saturating_sub(4));
    let popup_height = 44.min(area.height.saturating_sub(2));
    
    let popup_area = Rect {
        x: (area.width - popup_width) / 2,
        y: (area.height - popup_height) / 2,
        width: popup_width,
        height: popup_height,
    };
    
    // Clear the popup area
    f.render_widget(Clear, popup_area);
    
    let help_text = r#"
                        JAMBOREE KEYS
                        =============

  NODES
  ─────────────────────────────────────────────────────
  ← → / h l / Tab Focus previous / next node
  click           Focus a node; double-click to zoom
  Enter / z       Zoom into the focused node
  Esc             {esc}
  1-6             Hide or show node 0 to 5

  SCROLL (freezes the focused pane; the others follow)
  ─────────────────────────────────────────────────────
  ↑ ↓ / j k       One line
  PgUp PgDn       One page (also Ctrl+b / Ctrl+f)
  Ctrl+u Ctrl+d   Half a page
  Home End        Top / bottom and follow again (also gg / G)
  wheel           Scroll the node under the pointer
  F               Follow the logs again
  Space           Freeze / follow all panes

  SEARCH
  ─────────────────────────────────────────────────────
  /               Search: highlight terms, comma-separated (up to 6)
                  in the box: Ctrl+w drops the last term, Ctrl+u all
  n N             Next / previous match
  Ctrl+g          Filter: only matching lines / show all again
                  (asks for terms if none)
  c               Clear the search and the filter

  NETWORK (focused node)
  ─────────────────────────────────────────────────────
  s               Stop / continue (SIGSTOP)
  i               Isolate / heal
  p               Split the network here / heal
  r               Restart
  x               Shut down all nodes (their logs stay)
  q               Quit
  ?               This help

  Shift+drag selects text while the mouse is on.
              ↑↓ scroll, any other key closes
"#;

    let esc = match esc_clears_search {
        true => "Filter off, then clear search, then back to grid",
        false => "Back to the grid / cancel",
    };
    let text = help_text.replace("{esc}", esc);
    let last = (text.lines().count() as u16).saturating_sub(popup_height.saturating_sub(2));
    scroll.set(scroll.get().min(last));
    let title = match last {
        0 => " Help ",
        _ => " Help (↑↓ scroll) ",
    };
    let paragraph = Paragraph::new(text)
        .scroll((scroll.get(), 0))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, popup_area);
}
