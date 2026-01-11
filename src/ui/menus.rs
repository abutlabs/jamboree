use crate::AppState;

// Menu constants
const SEARCH_MENU: &str = " Search: {}_  [Enter] Apply  [Esc] Cancel ";

const GRID_MENU_LOG_VIEWER: &str = " [0-5] Toggle Node Visibility  [Tab] Focus  [/] Search  [↑↓ PgUp PgDn] Scroll  [Enter] Zoom  [Q] Quit";
const GRID_MENU_NORMAL: &str = " [0-5] Toggle Node Visibility  [Tab] Focus  [/] Search  [Space] Pause All  [Enter] Zoom  [X] Shutdown All  [Q] Quit";

const FOCUSED_MENU_LOG_VIEWER: &str = " [Esc] Grid  [/] Search  [↑↓ PgUp PgDn] Scroll  [Q] Quit";
const FOCUSED_MENU_NORMAL: &str = " [Esc] Grid  [/] Search  [Space] Pause  [↑↓ PgUp PgDn] Scroll  [X] Shutdown All  [Q] Quit";

const TAG_HINT_GRID: &str = "  [←→ Tag Nav] [Del/BS Remove Tag]";
const TAG_HINT_FOCUSED: &str = "  [←→ Tag Nav] [Del/BS Remove]";
const PAUSED_INDICATOR: &str = "  [PAUSED]";
const PAUSED_INDICATOR_FOCUSED: &str = " [PAUSED]";

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
            
            let tag_hint = match has_tags {
                true => TAG_HINT_GRID,
                false => "",
            };
            
            format!("{}{}{}", base_menu, status, tag_hint)
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
            
            let search_indicator = match &app.highlight {
                Some(terms) if !terms.is_empty() => {
                    let term_strings: Vec<&str> = terms.iter().map(|(term, _)| term.as_str()).collect();
                    format!(" [Search: {}]", term_strings.join(", "))
                }
                _ => String::new(),
            };
            
            let tag_hint = match has_tags {
                true => TAG_HINT_FOCUSED,
                false => "",
            };
            
            format!("{} {}{}{}", base_menu, status, search_indicator, tag_hint)
        }
    }
}

