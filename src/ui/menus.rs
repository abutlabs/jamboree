use crate::AppState;

// Menu constants
const SEARCH_MENU: &str =
    " Search: {}_  [Enter] Apply  [^W] Drop last term  [^U] Clear  [Esc] Cancel  (comma between terms)";

const GRID_MENU_LOG_VIEWER: &str = " [←→] Node  [↑↓] Scroll  [/] Search  [^G] Filter  [Enter] Zoom  [?] Help  [q] Quit";
const GRID_MENU_NORMAL: &str = " [←→] Node  [↑↓] Scroll  [/] Search  [^G] Filter  [Space] Freeze  [s] Stop  [r] Restart  [i] Isolate  [p] Split  [Enter] Zoom  [?] Help  [q] Quit";

const FOCUSED_MENU_LOG_VIEWER: &str = " [Esc] Grid  [/] Search  [^G] Filter  [?] Help  [q] Quit";
const FOCUSED_MENU_NORMAL: &str = " [Esc] Grid  [/] Search  [^G] Filter  [Space] Freeze  [s] Stop  [r] Restart  [i] Isolate  [p] Split  [?] Help  [q] Quit";

const MATCH_HINT: &str = " [n/N] Next/prev match  [c] Clear search ";
const MATCH_HINT_ESC: &str = " [n/N] Next/prev match  [c/Esc] Clear search ";
const PAUSED_INDICATOR: &str = " [FROZEN: Space resumes]";
const PAUSED_INDICATOR_FOCUSED: &str = " [FROZEN: Space resumes]";
const GREP_INDICATOR: &str = " [FILTER: matching lines only]";

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

            let grep_status = if app.grep_mode { GREP_INDICATOR } else { "" };

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
                (false, true) => PAUSED_INDICATOR_FOCUSED,
                _ => "",
            };

            let grep_status = if app.grep_mode { GREP_INDICATOR } else { "" };

            let search_indicator = match &app.highlight {
                Some(terms) if !terms.is_empty() => {
                    let term_strings: Vec<&str> =
                        terms.iter().map(|(term, _)| term.as_str()).collect();
                    format!(" [Search: {}]", term_strings.join(", "))
                }
                _ => String::new(),
            };

            let match_hint = match has_tags {
                true if app.esc_clears_search => MATCH_HINT_ESC,
                true => MATCH_HINT,
                false => "",
            };

            format!("{}{}{}{}{}", status, grep_status, search_indicator, match_hint, base_menu)
        }
    }
}
