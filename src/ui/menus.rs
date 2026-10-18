use crate::AppState;

// Menu constants
const SEARCH_MENU: &str = " Search: {}_  [Enter] Apply  [Esc] Cancel ";

const GRID_MENU_LOG_VIEWER: &str = " [←→] Node  [↑↓] Scroll  [/] Search  [Enter] Zoom  [?] Help  [q] Quit";
const GRID_MENU_NORMAL: &str = " [←→] Node  [↑↓] Scroll  [/] Search  [Space] Freeze  [s] Stop  [r] Restart  [i] Isolate  [p] Split  [Enter] Zoom  [?] Help  [q] Quit";

const FOCUSED_MENU_LOG_VIEWER: &str = " [Esc] Grid  [/] Search  [?] Help  [q] Quit";
const FOCUSED_MENU_NORMAL: &str = " [Esc] Grid  [/] Search  [Space] Freeze  [s] Stop  [r] Restart  [i] Isolate  [p] Split  [?] Help  [q] Quit";

const MATCH_HINT: &str = "  [n/N] Match  [^G] Only matches";
const PAUSED_INDICATOR: &str = "  [FROZEN]";
const PAUSED_INDICATOR_FOCUSED: &str = " [FROZEN]";
const GREP_INDICATOR: &str = " [GREP]";

pub fn grid_menu(app: &AppState, has_tags: bool) -> String {
    match app.search_mode {
        true => format!("{}", SEARCH_MENU.replace("{}", &app.search_input)),
        false => {
            let base_menu = match app.log_viewer_mode {
                true => GRID_MENU_LOG_VIEWER,
                false => GRID_MENU_NORMAL,
            };

            let status = match (app.log_viewer_mode, app.paused) {
                (false, true) => PAUSED_INDICATOR,
                _ => "",
            };

            let grep_status = if app.grep_mode { GREP_INDICATOR } else { "" };

            let match_hint = match has_tags {
                true => MATCH_HINT,
                false => "",
            };

            format!("{}{}{}{}", base_menu, status, grep_status, match_hint)
        }
    }
}

pub fn focused_menu(app: &AppState, has_tags: bool) -> String {
    match app.search_mode {
        true => format!("{}", SEARCH_MENU.replace("{}", &app.search_input)),
        false => {
            let base_menu = match app.log_viewer_mode {
                true => FOCUSED_MENU_LOG_VIEWER,
                false => FOCUSED_MENU_NORMAL,
            };

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
                true => MATCH_HINT,
                false => "",
            };

            format!("{} {}{}{}{}", base_menu, status, grep_status, search_indicator, match_hint)
        }
    }
}
