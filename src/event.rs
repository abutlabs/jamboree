use crate::AppState;
use crate::ViewMode;
use crate::node::Node;
use crossterm::event::{KeyCode, MouseEventKind};

pub enum Action {
    Continue,
    Quit,
    ChangeView(ViewMode),
}

pub fn handle_key_event(
    key: KeyCode,
    app: &mut AppState,
    focused: &mut usize,
    nodes: &[Node],
) -> Action {
    if app.search_mode {
        app.handle_search_key(key);
        return Action::Continue; // Don't fall through to main key handling
    }

    // Handle tag navigation if tags exist
    if app.highlight.is_some() && !app.highlight.as_ref().unwrap().is_empty() {
        match key {
            KeyCode::Left => {
                app.select_prev_tag();
                return Action::Continue;
            }
            KeyCode::Right => {
                app.select_next_tag();
                return Action::Continue;
            }
            KeyCode::Delete | KeyCode::Backspace => {
                app.remove_selected_tag();
                return Action::Continue;
            }
            _ => {}
        }
    }

    // Main key handling
    match key {
        KeyCode::Char('/') => {
            app.search_mode = true;
            if let Some(ref terms) = app.highlight {
                app.search_input = terms
                    .iter()
                    .map(|(term, _)| term.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
            }
        }
        KeyCode::Char(' ') if !app.log_viewer_mode => {
            app.toggle_pause(nodes);
        }
        KeyCode::Char('x') if !app.log_viewer_mode => {
            app.shutdown_all_nodes(nodes);
        }
        KeyCode::Char('q') => {
            return Action::Quit;
        }
        KeyCode::Tab => *focused = (*focused + 1) % nodes.len(),
        KeyCode::BackTab => *focused = (*focused + nodes.len() - 1) % nodes.len(),
        KeyCode::Enter => {
            return Action::ChangeView(ViewMode::Focused(*focused));
        }
        KeyCode::Up if app.paused || app.log_viewer_mode => {
            app.scroll_delta(*focused, -1);
        }
        KeyCode::Down if app.paused || app.log_viewer_mode => {
            app.scroll_delta(*focused, 1);
        }
        KeyCode::PageUp if app.paused || app.log_viewer_mode => {
            app.scroll_delta(*focused, -10);
        }
        KeyCode::PageDown if app.paused || app.log_viewer_mode => {
            app.scroll_delta(*focused, 10);
        }
        KeyCode::Char(c @ '0'..='5') => {
            let idx = (c as usize) - ('0' as usize);
            if idx < nodes.len() {
                app.hidden[idx] = !app.hidden[idx];
            }
        }
        KeyCode::Esc => {
            return Action::ChangeView(ViewMode::Grid);
        }
        _ => {}
    }

    Action::Continue
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
