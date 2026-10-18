use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Clear},
};

pub fn render(f: &mut Frame) {
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
  click           Focus a node; click it again to zoom
  Enter / z       Zoom into the focused node
  Esc             Back to the grid / cancel
  1-9             Hide or show the 1st to 9th pane

  SCROLL (freezes all panes first)
  ─────────────────────────────────────────────────────
  ↑ ↓ / j k       One line
  PgUp PgDn       One page (also Ctrl+b / Ctrl+f)
  Ctrl+u Ctrl+d   Half a page
  Home End        Top / bottom (also gg / G)
  wheel           Scroll the node under the pointer
  Space           Freeze / follow the logs again (also F)

  SEARCH
  ─────────────────────────────────────────────────────
  /               Search terms, comma-separated (up to 6)
  n N             Next / previous match
  Ctrl+g          Only matching lines

  NETWORK (focused node)
  ─────────────────────────────────────────────────────
  s               Stop / continue (SIGSTOP)
  i               Isolate / heal
  p               Split the network here / heal
  r               Restart
  x               Shut down all nodes
  q               Quit
  ?               This help

  Shift+drag selects text while the mouse is on.
                    Press any key to close
"#;

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, popup_area);
}
