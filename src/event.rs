use crate::AppState;
use crate::ViewMode;
use crossterm::event::{KeyCode, MouseEventKind};

pub enum Action {
    Continue,
    Quit,
    ChangeView(ViewMode),
}

pub fn handle_key_event(
    key: KeyCode,
    app: &mut AppState,
    _view_mode: &mut ViewMode,
    focused: &mut usize,
    node_count: usize,
) -> Action {
    if app.search_mode {
        app.handle_search_key(key);
        return Action::Continue; // Don't fall through to main key handling
    }

    // Main key handling
    match key {
        KeyCode::Char('/') => {
            app.search_mode = true;
            if let Some(ref current) = app.highlight {
                app.search_input = current.clone();
            }
        }
        KeyCode::Char(' ') => {
            app.toggle_pause();
        }
        KeyCode::Char('q') => {
            return Action::Quit;
        }
        KeyCode::Tab => *focused = (*focused + 1) % 6,
        KeyCode::BackTab => *focused = (*focused + 5) % 6,
        KeyCode::Enter => {
            return Action::ChangeView(ViewMode::Focused(*focused));
        }
        KeyCode::Up if app.paused => app.scroll_delta(*focused, -1),
        KeyCode::Down if app.paused => app.scroll_delta(*focused, 1),
        KeyCode::PageUp if app.paused => app.scroll_delta(*focused, -10),
        KeyCode::PageDown if app.paused => app.scroll_delta(*focused, 10),
        KeyCode::Char(c @ '0'..='5') => {
            let idx = (c as usize) - ('0' as usize);
            if idx < node_count {
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
    if !app.paused {
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
