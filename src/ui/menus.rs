use crate::AppState;

// Menu constants
const SEARCH_MENU: &str =
    " Search: {}_  [Enter] Apply  [^W] Drop last term  [^U] Clear  [Esc] Cancel  (comma between terms)";

// Help and Quit come first, so a narrow terminal still shows them
const GRID_MENU_LOG_VIEWER: &str = " [?] Help  [q] Quit  [←→] Node  [↑↓] Scroll  [/] Search  [^G] Filter  [Enter] Zoom";
const GRID_MENU_NORMAL: &str = " [?] Help  [q] Quit  [←→] Node  [↑↓] Scroll  [/] Search  [^G] Filter  [Space] Freeze  [s] Stop  [r] Restart  [i] Isolate  [p] Split  [Enter] Zoom";

const FOCUSED_MENU_LOG_VIEWER: &str = " [?] Help  [q] Quit  [Esc] Grid  [/] Search  [^G] Filter";
const FOCUSED_MENU_NORMAL: &str = " [?] Help  [q] Quit  [Esc] Grid  [/] Search  [^G] Filter  [Space] Freeze  [s] Stop  [r] Restart  [i] Isolate  [p] Split";

const MATCH_HINT: &str = " [n/N] Match  [c] Clear ";
const MATCH_HINT_ESC: &str = " [n/N] Match  [c/Esc] Clear ";
const PAUSED_INDICATOR: &str = " [ALL FROZEN: Space resumes]";
const GREP_INDICATOR: &str = " [FILTER ON]";
const MOUSE_OFF_INDICATOR: &str = " [MOUSE OFF: drag selects, m restores]";

/// The base menu, with Ctrl+G offered as "Show all" while the filter is on
fn base(menu: &str, app: &AppState) -> String {
    match app.grep_mode {
        true => menu.replace("[^G] Filter", "[^G] Show all"),
        false => menu.to_string(),
    }
}

pub fn grid_menu(app: &AppState, has_tags: bool) -> String {
    match app.search_mode {
        true => format!("{}", SEARCH_MENU.replace("{}", &app.search_input)),
        false => {
            let base_menu = base(
                match app.log_viewer_mode {
                    true => GRID_MENU_LOG_VIEWER,
                    false => GRID_MENU_NORMAL,
                },
                app,
            );

            let status = match (app.log_viewer_mode, app.paused) {
                (false, true) => PAUSED_INDICATOR,
                _ => "",
            };

            let grep_status = match (app.mouse_off, app.grep_mode) {
                (true, true) => format!("{MOUSE_OFF_INDICATOR}{GREP_INDICATOR}"),
                (true, false) => MOUSE_OFF_INDICATOR.to_string(),
                (false, true) => GREP_INDICATOR.to_string(),
                (false, false) => String::new(),
            };

            let match_hint = match has_tags {
                true if app.esc_clears_search => MATCH_HINT_ESC,
                true => MATCH_HINT,
                false => "",
            };

            // The current state and how to undo it come first, so a narrow terminal keeps them
            format!("{}{}{}{}", status, grep_status, match_hint, base_menu)
        }
    }
}

pub fn focused_menu(app: &AppState, has_tags: bool) -> String {
    match app.search_mode {
        true => format!("{}", SEARCH_MENU.replace("{}", &app.search_input)),
        false => {
            let base_menu = base(
                match app.log_viewer_mode {
                    true => FOCUSED_MENU_LOG_VIEWER,
                    false => FOCUSED_MENU_NORMAL,
                },
                app,
            );

            let status = match (app.log_viewer_mode, app.paused) {
                (false, true) => PAUSED_INDICATOR,
                _ => "",
            };

            let grep_status = match (app.mouse_off, app.grep_mode) {
                (true, true) => format!("{MOUSE_OFF_INDICATOR}{GREP_INDICATOR}"),
                (true, false) => MOUSE_OFF_INDICATOR.to_string(),
                (false, true) => GREP_INDICATOR.to_string(),
                (false, false) => String::new(),
            };

            let match_hint = match has_tags {
                true if app.esc_clears_search => MATCH_HINT_ESC,
                true => MATCH_HINT,
                false => "",
            };

            format!("{}{}{}{}", status, grep_status, match_hint, base_menu)
        }
    }
}
