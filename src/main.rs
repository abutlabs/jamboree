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
use portable_pty::CommandBuilder;
use ratatui::prelude::*;
use std::{io::stdout, path::PathBuf, process, time::Duration};

use crate::event::{Action, handle_key_event, handle_mouse_event};
use crate::ui::focused;
use crate::ui::help;
use ui::grid;

/// One `sh -c` command per node, with `{i}` replaced by the node index
fn node_commands(cmd: &str, count: usize) -> Result<Vec<CommandBuilder>> {
    anyhow::ensure!(count > 0, "--nodes must be at least 1");
    let cwd = std::env::current_dir()?;
    Ok((0..count)
        .map(|id| {
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", &utils::node_command(cmd, id)]);
            c.cwd(&cwd);
            c
        })
        .collect())
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

    // Resolve the node commands before terminal setup (so errors print normally)
    let commands = match (&cli.log_folder, &cli.cmd) {
        (None, Some(cmd)) => match node_commands(cmd, cli.nodes.unwrap_or(6)) {
            Ok(commands) => Some(commands),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        _ => None,
    };

    if let Err(e) = run_app(cli, commands).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}

async fn run_app(cli: Cli, commands: Option<Vec<CommandBuilder>>) -> Result<()> {
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

        // One pane per *.log file, up to --nodes
        let files = utils::find_log_files(&log_dir)?;
        for (id, log_path) in files.iter().take(cli.nodes.unwrap_or(usize::MAX)).enumerate() {
            nodes.push(Node::from_log_file(id, log_path)?);
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

        let nodes = commands
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(id, cmd)| Node::spawn(id, &log_dir, cmd))
            .collect::<Result<Vec<Node>>>()?;

        // Start the summary generation background task when there is a block pattern
        if let Some(re) = cli.summary_regex.clone() {
            summary_tx = Some(summary::start_summary_task(log_dir, nodes.len(), re));
        }

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
