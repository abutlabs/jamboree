use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::time::Instant;
use std::collections::HashMap;

use crate::node::Node;

pub struct AppState {
    pub paused: bool,
    pub frozen_scroll: RefCell<Vec<u16>>, // one per node
    pub hidden: Vec<bool>,
    pub log_viewer_mode: bool, // true when loading from log files instead of running nodes

    pub search_mode: bool,
    pub search_input: String,
    pub highlight: Option<Vec<(String, usize)>>, // Multiple search terms with color index (max 6)
    pub grep_mode: bool, // true to filter lines (grep), false to only highlight
    pub show_help: bool, // true to display help overlay
    pub last_key: Option<char>, // Track last key for vi commands like 'gg'
    /// Where each pane was drawn, by node index, for the mouse
    pub panes: RefCell<Vec<(usize, Rect)>>,
    /// Node and time of the last click, to tell a double click
    pub last_click: Option<(usize, Instant)>,
}

impl AppState {
    fn apply_search(
        search_input: &str,
        existing_highlight: Option<&Vec<(String, usize)>>,
    ) -> Option<Vec<(String, usize)>> {
        if search_input.is_empty() {
            return None;
        }

        // Parse comma-separated terms, trim whitespace, limit to 6
        let new_terms: Vec<String> = search_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .take(6)
            .collect();

        if new_terms.is_empty() {
            return None;
        }

        // Preserve colors of existing tags, assign new colors to new tags
        let existing: HashMap<String, usize> = existing_highlight
            .map(|tags| {
                tags.iter()
                    .map(|(term, color)| (term.clone(), *color))
                    .collect()
            })
            .unwrap_or_default();

        // Find the highest existing color index
        let max_color = existing.values().max().copied().unwrap_or(0);

        // Find next available color, but reuse colors if we have fewer than 6 tags
        let mut used_colors: Vec<usize> = existing.values().copied().collect();
        used_colors.sort();
        let mut next_color = 0;

        let mut terms: Vec<(String, usize)> = Vec::new();

        for term in new_terms {
            if let Some(&color) = existing.get(&term) {
                // Term already exists, preserve its color
                terms.push((term, color));
            } else {
                // New term, find next available color (0-5)
                // First try max_color + 1, but if that's taken, find first unused
                while next_color < 6 && used_colors.contains(&next_color) {
                    next_color += 1;
                }
                let color = if next_color < 6 {
                    let assigned = next_color;
                    used_colors.push(assigned);
                    next_color += 1;
                    assigned
                } else {
                    // Fallback: use max_color + 1, capped at 5
                    (max_color + 1).min(5)
                };
                terms.push((term, color));
            }
        }

        Some(terms)
    }

    pub fn new(node_count: usize, search: Option<String>) -> Self {
        let search_input = search.unwrap_or_default();

        // Process CLI search string to create highlight tags
        let highlight = Self::apply_search(&search_input, None);


        Self {
            paused: false,
            frozen_scroll: RefCell::new(vec![0; node_count]),
            log_viewer_mode: false,
            search_mode: false,
            search_input,
            highlight,
            hidden: vec![false; node_count],
            grep_mode: false,
            show_help: false,
            last_key: None,
            panes: RefCell::new(Vec::new()),
            last_click: None,
        }
    }

    pub fn new_log_viewer(node_count: usize, search: Option<String>) -> Self {
        let search_input = search.unwrap_or_default();
        let highlight = Self::apply_search(&search_input, None);

        Self {
            paused: false, // Not used in log viewer mode
            frozen_scroll: RefCell::new(vec![0; node_count]),
            log_viewer_mode: true,
            search_mode: false,
            search_input,
            highlight,
            hidden: vec![false; node_count],
            grep_mode: false,
            show_help: false,
            last_key: None,
            panes: RefCell::new(Vec::new()),
            last_click: None,
        }
    }

    pub fn handle_search_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.search_mode = false;
                self.search_input.clear();
            }
            KeyCode::Enter => {
                self.highlight = Self::apply_search(&self.search_input, self.highlight.as_ref());
                self.search_mode = false;
            }
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
            }
            _ => {}
        }
    }

    pub fn toggle_pause(&mut self, nodes: &[Node]) {
        self.paused = !self.paused;
        if self.paused {
            // entering pause
            for n in nodes {
                *n.paused.lock().unwrap() = true;
            }
        } else {
            // leaving pause
            for n in nodes {
                *n.paused.lock().unwrap() = false;
                n.flush_paused_buffer();
            }

            // reset frozen scroll
            for s in self.frozen_scroll.borrow_mut().iter_mut() {
                *s = 0;
            }
        }
    }

    pub fn scroll_delta(&mut self, idx: usize, delta: i16) {
        let mut scroll = self.frozen_scroll.borrow_mut();
        let next = (scroll[idx] as i16 + delta).max(0).min(10000i16);
        scroll[idx] = next as u16;
    }

    /// Shutdown all nodes and auto-trigger pause
    pub fn shutdown_all_nodes(&mut self, nodes: &[Node]) {
        crate::node::stop_all(nodes, std::time::Duration::from_secs(3));

        // Auto-trigger pause if not already paused
        if !self.paused {
            self.toggle_pause(nodes);
        }
    }

    /// Toggle grep mode (filter lines vs highlight only)
    pub fn toggle_grep_mode(&mut self) {
        self.grep_mode = !self.grep_mode;
    }

    /// Toggle help overlay
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Scroll to the beginning of the file (vi 'gg')
    pub fn scroll_to_top(&mut self, idx: usize) {
        let mut scroll = self.frozen_scroll.borrow_mut();
        scroll[idx] = 0;
    }

    /// Scroll to the end of the file (vi 'G')
    pub fn scroll_to_bottom(&mut self, idx: usize) {
        let mut scroll = self.frozen_scroll.borrow_mut();
        scroll[idx] = 10000; // Will be clamped by the renderer
    }

    /// Scroll by half page (vi Ctrl+d / Ctrl+u)
    pub fn scroll_half_page(&mut self, idx: usize, down: bool) {
        let delta = if down { 15 } else { -15 };
        self.scroll_delta(idx, delta);
    }

    /// Scroll by full page (vi Ctrl+f / Ctrl+b)
    pub fn scroll_full_page(&mut self, idx: usize, down: bool) {
        let delta = if down { 30 } else { -30 };
        self.scroll_delta(idx, delta);
    }

    /// Find line numbers that contain any search term
    fn find_matching_lines(&self, text: &str) -> Vec<usize> {
        let Some(ref terms) = self.highlight else {
            return Vec::new();
        };

        if terms.is_empty() {
            return Vec::new();
        }

        text.lines()
            .enumerate()
            .filter(|(_, line)| {
                terms.iter().any(|(term, _)| {
                    !term.is_empty() && line.contains(term.as_str())
                })
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Jump to next search match (vi 'n')
    pub fn jump_to_next_match(&mut self, idx: usize, text: &str) {
        let matches = self.find_matching_lines(text);
        if matches.is_empty() {
            return;
        }

        let current_scroll = self.frozen_scroll.borrow()[idx] as usize;
        
        // Find first match after current scroll position
        if let Some(&next_line) = matches.iter().find(|&&line| line > current_scroll) {
            self.frozen_scroll.borrow_mut()[idx] = next_line as u16;
        } else {
            // Wrap around to first match
            self.frozen_scroll.borrow_mut()[idx] = matches[0] as u16;
        }
    }

    /// Jump to previous search match (vi 'N')
    pub fn jump_to_prev_match(&mut self, idx: usize, text: &str) {
        let matches = self.find_matching_lines(text);
        if matches.is_empty() {
            return;
        }

        let current_scroll = self.frozen_scroll.borrow()[idx] as usize;
        
        // Find last match before current scroll position
        if let Some(&prev_line) = matches.iter().rev().find(|&&line| line < current_scroll) {
            self.frozen_scroll.borrow_mut()[idx] = prev_line as u16;
        } else {
            // Wrap around to last match
            self.frozen_scroll.borrow_mut()[idx] = *matches.last().unwrap() as u16;
        }
    }

    /// Filter text to only include lines matching any of the search terms (grep behavior)
    pub fn filter_text(&self, text: &str) -> String {
        if !self.grep_mode {
            return text.to_string();
        }

        let Some(ref terms) = self.highlight else {
            return text.to_string();
        };

        if terms.is_empty() {
            return text.to_string();
        }

        // Match the text without colours, which can split a term
        text.lines()
            .filter(|line| {
                let plain = strip_ansi_escapes::strip_str(line);
                terms.iter().any(|(term, _)| {
                    !term.is_empty() && plain.contains(term.as_str())
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
