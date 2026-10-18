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
                        JAM-UI KEYBINDINGS
                        ==================

  NAVIGATION (Vi-style)
  ─────────────────────────────────────────────────────
  j / ↓           Scroll down one line
  k / ↑           Scroll up one line
  Ctrl+d          Scroll down half page
  Ctrl+u          Scroll up half page
  Ctrl+f / PgDn   Scroll down full page
  Ctrl+b / PgUp   Scroll up full page
  G               Go to end of file
  gg              Go to beginning of file

  VIEWS & FOCUS
  ─────────────────────────────────────────────────────
  Tab / Shift+Tab Cycle focus between nodes
  Enter           Zoom into focused node
  Esc             Return to grid view / Cancel
  0-9             Toggle node visibility

  SEARCH & FILTER
  ─────────────────────────────────────────────────────
  /               Enter search mode (comma-separated terms)
  n               Jump to next search match
  N               Jump to previous search match
  Ctrl+g          Toggle grep mode (filter vs highlight)
  ← →             Navigate between search tags
  Del / Backspace Remove selected tag

  CONTROLS
  ─────────────────────────────────────────────────────
  Space           Freeze/unfreeze all panes
  s               Stop/continue the focused node (SIGSTOP)
  i               Isolate/heal the focused node (--net-faults)
  x               Shutdown all nodes
  q               Quit
  ?               Toggle this help

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
