mod app;
mod cli;
mod event;
mod node;
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

/// One labelled `sh -c` command per node, with `{i}` replaced by the node index:
/// a single --cmd is the template for every node, several --cmd give one node each
fn node_commands(cmds: &[String], nodes: Option<usize>) -> Result<Vec<(String, CommandBuilder)>> {
    let count = match (cmds.len(), nodes) {
        (1, n) => n.unwrap_or(6),
        (len, None) => len,
        (len, Some(n)) if n == len => n,
        (len, Some(n)) => anyhow::bail!("--nodes {} does not match the {} --cmd flags", n, len),
    };
    anyhow::ensure!(count > 0, "--nodes must be at least 1");
    let cwd = std::env::current_dir()?;
    Ok((0..count)
        .map(|id| {
            let (label, cmd) = utils::split_label(&cmds[if cmds.len() == 1 { 0 } else { id }]);
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", &utils::node_command(cmd, id)]);
            c.cwd(&cwd);
            (label.unwrap_or("Node").to_string(), c)
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
    let commands = match &cli.log_folder {
        None => match node_commands(&cli.cmd, cli.nodes) {
            Ok(commands) => commands,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Some(_) => Vec::new(),
    };

    if let Err(e) = run_app(cli, commands).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}

async fn run_app(cli: Cli, commands: Vec<(String, CommandBuilder)>) -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, Hide)?;
    let _guard = TerminalGuard;

    let mut view_mode = ViewMode::Grid;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

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
            .into_iter()
            .enumerate()
            .map(|(id, (label, cmd))| Node::spawn(id, label, &log_dir, cmd))
            .collect::<Result<Vec<Node>>>()?;

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
                    match handle_key_event(k.code, k.modifiers, &mut app, &mut focused, &nodes) {
                        Action::Quit => {
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
