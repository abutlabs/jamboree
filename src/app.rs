use crossterm::event::KeyCode;
use std::cell::RefCell;

pub struct AppState {
    pub paused: bool,
    pub frozen_scroll: RefCell<Vec<u16>>, // one per node
    pub hidden: Vec<bool>,

    pub search_mode: bool,
    pub search_input: String,
    pub highlight: Option<String>,
}

impl AppState {
    pub fn new(node_count: usize) -> Self {
        Self {
            paused: false,
            frozen_scroll: RefCell::new(vec![0; node_count]),
            search_mode: false,
            search_input: String::new(),
            highlight: None,
            hidden: vec![false; node_count],
        }
    }

    pub fn handle_search_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.search_mode = false;
                self.search_input.clear();
            }
            KeyCode::Enter => {
                self.highlight = if self.search_input.is_empty() {
                    None
                } else {
                    Some(self.search_input.clone())
                };
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

    pub fn toggle_pause(&mut self) {
        let was_paused = self.paused;
        self.paused = !self.paused;
        if !self.paused && was_paused {
            // Clear frozen scroll when unpausing
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
}

