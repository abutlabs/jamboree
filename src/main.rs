mod app;
mod event;
mod node;
mod ui;

use crate::app::AppState;
use crate::node::Node;
use anyhow::Result;
use chrono::Local;
use crossterm::{
    event::{Event, poll, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use std::{io::stdout, path::PathBuf, process, time::Duration};

use crate::event::{Action, handle_key_event, handle_mouse_event};
use crate::ui::focused;
use ui::grid;

pub enum ViewMode {
    Grid,
    Focused(usize),
}
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut view_mode = ViewMode::Grid;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Create timestamped log directory
    let datetime = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let log_dir = PathBuf::from("logs").join(datetime);
    std::fs::create_dir_all(&log_dir)?;

    let nodes = (0..6)
        .map(|id| Node::spawn(id, &log_dir))
        .collect::<Result<Vec<Node>>>()?;

    let mut app = AppState::new(nodes.len());

    let mut focused = 0usize;

    loop {
        terminal.draw(|f| match view_mode {
            ViewMode::Grid => {
                grid::render(f, &nodes, focused, &app);
            }
            ViewMode::Focused(i) => {
                focused::render(f, &nodes[i], &app);
            }
        })?;

        if poll(Duration::from_millis(50))? {
            match read()? {
                Event::Key(k) => {
                    match handle_key_event(
                        k.code,
                        &mut app,
                        &mut focused,
                        nodes.len(),
                    ) {
                        Action::Quit => {
                            // Cleanup and exit
                            let _ = disable_raw_mode();
                            let _ = execute!(stdout(), LeaveAlternateScreen);
                            process::exit(0);
                        }
                        Action::ChangeView(new_view) => {
                            view_mode = new_view;
                        }
                        Action::Continue => {}
                    }
                }
                Event::Mouse(m) => {
                    handle_mouse_event(m.kind, &mut app, focused);
                }
                _ => {}
            }
        }
    }

    // Unreachable - 'q' uses process::exit(0) which terminates immediately
    // Terminal cleanup happens automatically on process exit
    Ok(())
}
