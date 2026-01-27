use crossterm::event::KeyCode;
use std::cell::RefCell;
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
    pub selected_tag: Option<usize>, // Currently selected tag index for keyboard navigation
    pub grep_mode: bool, // true to filter lines (grep), false to only highlight
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

        let selected_tag = if highlight.is_some() && !highlight.as_ref().unwrap().is_empty() {
            Some(0)
        } else {
            None
        };

        Self {
            paused: false,
            frozen_scroll: RefCell::new(vec![0; node_count]),
            log_viewer_mode: false,
            search_mode: false,
            search_input,
            highlight,
            hidden: vec![false; node_count],
            selected_tag,
            grep_mode: false,
        }
    }

    pub fn new_log_viewer(node_count: usize, search: Option<String>) -> Self {
        let search_input = search.unwrap_or_default();
        let highlight = Self::apply_search(&search_input, None);
        let selected_tag = if highlight.is_some() && !highlight.as_ref().unwrap().is_empty() {
            Some(0)
        } else {
            None
        };

        Self {
            paused: false, // Not used in log viewer mode
            frozen_scroll: RefCell::new(vec![0; node_count]),
            log_viewer_mode: true,
            search_mode: false,
            search_input,
            highlight,
            hidden: vec![false; node_count],
            selected_tag,
            grep_mode: false,
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
                // Initialize selected tag to first tag when tags are created
                if self.highlight.is_some() && !self.highlight.as_ref().unwrap().is_empty() {
                    self.selected_tag = Some(0);
                } else {
                    self.selected_tag = None;
                }
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

    pub fn remove_tag(&mut self, index: usize) {
        if let Some(ref mut terms) = self.highlight {
            if index < terms.len() {
                terms.remove(index);
                if terms.is_empty() {
                    self.highlight = None;
                    self.selected_tag = None;
                } else {
                    // Adjust selected tag index if needed
                    if let Some(ref mut selected) = self.selected_tag {
                        if *selected >= terms.len() {
                            *selected = terms.len().saturating_sub(1);
                        }
                        if *selected > index {
                            *selected -= 1;
                        }
                    }
                }
            }
        }
    }

    pub fn select_next_tag(&mut self) {
        if let Some(ref terms) = self.highlight {
            if !terms.is_empty() {
                let current = self.selected_tag.unwrap_or(0);
                self.selected_tag = Some((current + 1) % terms.len());
            }
        }
    }

    pub fn select_prev_tag(&mut self) {
        if let Some(ref terms) = self.highlight {
            if !terms.is_empty() {
                let current = self.selected_tag.unwrap_or(0);
                let len = terms.len();
                self.selected_tag = Some((current + len - 1) % len);
            }
        }
    }

    pub fn remove_selected_tag(&mut self) {
        if let Some(idx) = self.selected_tag {
            self.remove_tag(idx);
            // After removal, selected_tag is adjusted by remove_tag
        }
    }

    /// Shutdown all nodes and auto-trigger pause
    pub fn shutdown_all_nodes(&mut self, nodes: &[Node]) {
        // Shutdown all nodes
        for node in nodes {
            node.shutdown();
        }

        // Auto-trigger pause if not already paused
        if !self.paused {
            self.toggle_pause(nodes);
        }
    }

    /// Toggle grep mode (filter lines vs highlight only)
    pub fn toggle_grep_mode(&mut self) {
        self.grep_mode = !self.grep_mode;
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

        text.lines()
            .filter(|line| {
                terms.iter().any(|(term, _)| {
                    !term.is_empty() && line.contains(term.as_str())
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
