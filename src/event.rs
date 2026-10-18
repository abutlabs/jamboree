use crate::AppState;
use crate::ViewMode;
use crate::node::Node;
use crossterm::event::{KeyCode, KeyModifiers, MouseEventKind};

pub enum Action {
    Continue,
    Quit,
    ChangeView(ViewMode),
}

pub fn handle_key_event(
    key: KeyCode,
    modifiers: KeyModifiers,
    app: &mut AppState,
    focused: &mut usize,
    nodes: &[Node],
) -> Action {
    // If help is showing, any key closes it
    if app.show_help {
        app.show_help = false;
        app.last_key = None;
        return Action::Continue;
    }

    if app.search_mode {
        app.handle_search_key(key);
        app.last_key = None;
        return Action::Continue;
    }

    // Handle tag navigation if tags exist
    if app.highlight.is_some() && !app.highlight.as_ref().unwrap().is_empty() {
        match key {
            KeyCode::Left => {
                app.select_prev_tag();
                app.last_key = None;
                return Action::Continue;
            }
            KeyCode::Right => {
                app.select_next_tag();
                app.last_key = None;
                return Action::Continue;
            }
            KeyCode::Delete | KeyCode::Backspace => {
                app.remove_selected_tag();
                app.last_key = None;
                return Action::Continue;
            }
            _ => {}
        }
    }

    // Check if scrolling is allowed (paused or log viewer mode)
    let can_scroll = app.paused || app.log_viewer_mode;

    // Main key handling
    let action = match key {
        // Help - always available
        KeyCode::Char('?') => {
            app.toggle_help();
            Action::Continue
        }

        // Search - always available
        KeyCode::Char('/') => {
            app.search_mode = true;
            if let Some(ref terms) = app.highlight {
                app.search_input = terms
                    .iter()
                    .map(|(term, _)| term.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
            }
            Action::Continue
        }

        // Grep toggle with Ctrl+G - always available
        KeyCode::Char('g') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_grep_mode();
            Action::Continue
        }

        // Vi: gg - go to top (second g after first g)
        KeyCode::Char('g') if app.last_key == Some('g') => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes); // Auto-pause when trying to scroll
            }
            app.scroll_to_top(*focused);
            Action::Continue
        }

        // First 'g' press - wait for potential 'gg'
        KeyCode::Char('g') => {
            app.last_key = Some('g');
            return Action::Continue; // Return early to preserve last_key
        }

        // Vi: G - go to bottom
        KeyCode::Char('G') => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_to_bottom(*focused);
            Action::Continue
        }

        // Vi: j - scroll down one line
        KeyCode::Char('j') => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_delta(*focused, 1);
            Action::Continue
        }

        // Vi: k - scroll up one line
        KeyCode::Char('k') => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_delta(*focused, -1);
            Action::Continue
        }

        // Vi: Ctrl+d - scroll down half page
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_half_page(*focused, true);
            Action::Continue
        }

        // Vi: Ctrl+u - scroll up half page
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_half_page(*focused, false);
            Action::Continue
        }

        // Vi: Ctrl+f - scroll down full page
        KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_full_page(*focused, true);
            Action::Continue
        }

        // Vi: Ctrl+b - scroll up full page
        KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
            if !can_scroll && !app.log_viewer_mode {
                app.toggle_pause(nodes);
            }
            app.scroll_full_page(*focused, false);
            Action::Continue
        }

        // Vi: n - jump to next search match
        KeyCode::Char('n') => {
            if app.highlight.is_some() {
                if !can_scroll && !app.log_viewer_mode {
                    app.toggle_pause(nodes);
                }
                let text = nodes[*focused].get_text_tail(5_000_000);
                let filtered_text = app.filter_text(&text);
                app.jump_to_next_match(*focused, &filtered_text);
            }
            Action::Continue
        }

        // Vi: N - jump to previous search match
        KeyCode::Char('N') => {
            if app.highlight.is_some() {
                if !can_scroll && !app.log_viewer_mode {
                    app.toggle_pause(nodes);
                }
                let text = nodes[*focused].get_text_tail(5_000_000);
                let filtered_text = app.filter_text(&text);
                app.jump_to_prev_match(*focused, &filtered_text);
            }
            Action::Continue
        }

        // Arrow keys and Page keys (existing)
        KeyCode::Up if can_scroll => {
            app.scroll_delta(*focused, -1);
            Action::Continue
        }
        KeyCode::Down if can_scroll => {
            app.scroll_delta(*focused, 1);
            Action::Continue
        }
        KeyCode::PageUp if can_scroll => {
            app.scroll_full_page(*focused, false);
            Action::Continue
        }
        KeyCode::PageDown if can_scroll => {
            app.scroll_full_page(*focused, true);
            Action::Continue
        }

        // Freeze/unfreeze all panes
        KeyCode::Char(' ') if !app.log_viewer_mode => {
            app.toggle_pause(nodes);
            Action::Continue
        }

        // Stop/continue the focused node (SIGSTOP/SIGCONT), marked in every log
        KeyCode::Char('s') if !app.log_viewer_mode => {
            let node = &nodes[*focused];
            let what = if node.toggle_stopped() {
                "stopped"
            } else {
                "continued"
            };
            for n in nodes {
                n.mark(&format!("{} {} {}", node.label, node.id, what));
            }
            Action::Continue
        }

        // Shutdown
        KeyCode::Char('x') if !app.log_viewer_mode => {
            app.shutdown_all_nodes(nodes);
            Action::Continue
        }

        // Quit
        KeyCode::Char('q') => Action::Quit,

        // Focus navigation
        KeyCode::Tab => {
            *focused = (*focused + 1) % nodes.len();
            Action::Continue
        }
        KeyCode::BackTab => {
            *focused = (*focused + nodes.len() - 1) % nodes.len();
            Action::Continue
        }

        // Zoom
        KeyCode::Enter => Action::ChangeView(ViewMode::Focused(*focused)),

        // Node visibility toggle
        KeyCode::Char(c @ '0'..='9') => {
            let idx = (c as usize) - ('0' as usize);
            if idx < nodes.len() {
                app.hidden[idx] = !app.hidden[idx];
            }
            Action::Continue
        }

        // Back to grid
        KeyCode::Esc => Action::ChangeView(ViewMode::Grid),

        _ => Action::Continue,
    };

    // Clear last_key after processing (except when we're waiting for 'gg')
    if key != KeyCode::Char('g') || app.last_key == Some('g') {
        app.last_key = None;
    }

    // Handle 'g' timeout - if last_key is 'g' and we processed something else, toggle grep
    if app.last_key == Some('g') {
        // This case is handled above, but if we reach here with 'g' still set, 
        // it means we're waiting for the next key
    }

    action
}

pub fn handle_mouse_event(mouse: MouseEventKind, app: &mut AppState, focused: usize) -> Action {
    if !app.paused && !app.log_viewer_mode {
        return Action::Continue;
    }

    match mouse {
        MouseEventKind::ScrollUp => {
            app.scroll_delta(focused, -3);
        }
        MouseEventKind::ScrollDown => {
            app.scroll_delta(focused, 3);
        }
        _ => {}
    }

    Action::Continue
}
