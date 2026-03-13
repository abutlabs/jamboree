mod app;
mod cli;
mod event;
mod node;
mod summary;
mod ui;
mod utils;

use crate::app::AppState;
use crate::cli::Cli;
use crate::node::Node;
use anyhow::Result;
use chrono::Local;
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{Event, poll, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use std::{io::stdout, path::PathBuf, process, time::Duration};

use crate::event::{Action, handle_key_event, handle_mouse_event};
use crate::ui::focused;
use crate::ui::help;
use ui::grid;
use std::collections::HashMap;

fn parse_skip_announcements(skip_str: &Option<String>) -> HashMap<usize, String> {
    let mut map = HashMap::new();
    
    if let Some(s) = skip_str {
        // Parse format: "0:4" or "0:4,1:5" (node_id:skip_node_ids)
        for part in s.split(',') {
            let part = part.trim();
            if let Some(colon_pos) = part.find(':') {
                let node_id_str = &part[..colon_pos];
                let skip_ids = &part[colon_pos + 1..];
                
                if let Ok(node_id) = node_id_str.parse::<usize>() {
                    map.insert(node_id, skip_ids.to_string());
                }
            }
        }
    }
    
    map
}

pub enum ViewMode {
    Grid,
    Focused(usize),
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Show);
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Parse CLI first, before terminal setup (so --help works properly)
    let cli = Cli::parse();

    // Clean up any leftover build lock from incomplete previous builds
    utils::cleanup_build_lock();

    if let Err(e) = run_app(cli).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}

async fn run_app(cli: Cli) -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, Hide)?;
    let _guard = TerminalGuard;

    let mut view_mode = ViewMode::Grid;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Track the log directory for summary generation
    let mut summary_tx: Option<tokio::sync::mpsc::Sender<summary::SummaryCommand>> = None;

    let (nodes, mut app) = if let Some(log_folder) = &cli.log_folder {
        // Log viewer mode: load from log files
        let log_dir = PathBuf::from(log_folder);
        let mut nodes = Vec::new();

        // Try to load node_0.log through node_5.log
        for id in 0..6 {
            let log_path = log_dir.join(format!("node_{}.log", id));
            if log_path.exists() {
                nodes.push(Node::from_log_file(id, &log_path)?);
            }
        }

        if nodes.is_empty() {
            anyhow::bail!("No log files found in {}", log_folder);
        }

        let app = AppState::new_log_viewer(nodes.len(), cli.search);
        (nodes, app)
    } else {
        // Normal mode: spawn nodes
        let datetime = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let log_dir = PathBuf::from("logs").join(datetime);
        std::fs::create_dir_all(&log_dir)?;

        // Parse skip_announcements_to format: "0:4" or "0:4,1:5" (node_id:skip_node_id)
        let skip_map = parse_skip_announcements(&cli.skip_announcements_to);
        
        let nodes = (0..6)
            .map(|id| {
                let skip_to = skip_map.get(&id).cloned();
                Node::spawn(id, &log_dir, skip_to)
            })
            .collect::<Result<Vec<Node>>>()?;

        // Start the summary generation background task
        summary_tx = Some(summary::start_summary_task(log_dir));

        let app = AppState::new(nodes.len(), cli.search);
        (nodes, app)
    };

    let mut focused = 0usize;

    loop {
        terminal.draw(|f| {
            match view_mode {
                ViewMode::Grid => {
                    grid::render(f, &nodes, focused, &app);
                }
                ViewMode::Focused(i) => {
                    focused::render(f, &nodes[i], &app);
                }
            }
            // Render help overlay on top if active
            if app.show_help {
                help::render(f);
            }
        })?;

        if poll(Duration::from_millis(50))? {
            match read()? {
                Event::Key(k) => {
                    // Check for 'x' key to trigger summary generation on shutdown
                    if k.code == crossterm::event::KeyCode::Char('x') && !app.log_viewer_mode {
                        if let Some(ref tx) = summary_tx {
                            let _ = tx.try_send(summary::SummaryCommand::Generate);
                        }
                    }
                    
                    match handle_key_event(k.code, k.modifiers, &mut app, &mut focused, &nodes) {
                        Action::Quit => {
                            // Generate final summary before exiting
                            if let Some(ref tx) = summary_tx {
                                let _ = tx.try_send(summary::SummaryCommand::Stop);
                                // Give a moment for the summary to be written
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                            
                            // Cleanup and exit (process::exit terminates immediately, so guard won't drop)
                            let _ = disable_raw_mode();
                            let _ = execute!(stdout(), LeaveAlternateScreen, Show);
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
}
