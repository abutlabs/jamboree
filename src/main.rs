mod app;
mod cli;
mod event;
mod net;
mod node;
mod ui;
mod utils;

use crate::app::AppState;
use crate::cli::Cli;
use crate::net::Net;
use crate::node::Node;
use anyhow::Result;
use chrono::Local;
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture, Event, poll, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use portable_pty::CommandBuilder;
use ratatui::prelude::*;
use regex::Regex;
use std::{
    collections::BTreeMap,
    io::stdout,
    path::PathBuf,
    process,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::event::{Action, Fault, handle_key_event, handle_mouse_event};
use crate::ui::focused;
use crate::ui::help;
use ui::grid;

/// Number of nodes: a single --cmd is the template for every node, several --cmd give one node each
fn node_count(cmds: &[String], nodes: Option<usize>) -> Result<usize> {
    let count = match (cmds.len(), nodes) {
        (1, n) => n.unwrap_or(6),
        (len, None) => len,
        (len, Some(n)) if n == len => n,
        (len, Some(n)) => anyhow::bail!("--nodes {} does not match the {} --cmd flags", n, len),
    };
    anyhow::ensure!(count > 0, "--nodes must be at least 1");
    Ok(count)
}

/// The command template of node `id`
fn node_template(cmds: &[String], id: usize) -> &str {
    &cmds[if cmds.len() == 1 { 0 } else { id }]
}

/// Container name per node whose command uses `{container}`, unique to this run
fn container_names(cmds: &[String], count: usize) -> Vec<Option<String>> {
    (0..count)
        .map(|id| {
            node_template(cmds, id)
                .contains("{container}")
                .then(|| format!("jamboree-{}-{id}", std::process::id()))
        })
        .collect()
}

/// One labelled `sh -c` command per node, with `{i}` replaced by the node index, `{port}` by
/// the port it listens on (also in `$PORT` when the ports were moved) and `{container}` by its
/// container name
fn node_commands(
    cmds: &[String],
    ports: &[u16],
    moved: bool,
    containers: &[Option<String>],
) -> Result<Vec<(String, CommandBuilder, Option<String>)>> {
    let cwd = std::env::current_dir()?;
    (0..ports.len())
        .map(|id| {
            let (label, cmd) = utils::split_label(node_template(cmds, id));
            let cmd = utils::node_command(cmd, id)
                .replace("{port}", &ports[id].to_string())
                .replace("{container}", containers[id].as_deref().unwrap_or_default());
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", &cmd]);
            if moved {
                c.env("PORT", ports[id].to_string());
            }
            c.cwd(&cwd);
            Ok((label.unwrap_or("Node").to_string(), c, containers[id].clone()))
        })
        .collect()
}

pub enum ViewMode {
    Grid,
    /// The focused node alone
    Zoomed,
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show);
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Parse CLI first, before terminal setup (so --help works properly)
    let cli = Cli::parse();

    // Resolve the node commands before terminal setup (so errors print normally)
    let setup = || -> Result<_> {
        if cli.log_folder.is_some() {
            return Ok((Vec::new(), None, Vec::new(), Vec::new()));
        }
        let count = node_count(&cli.cmd, cli.nodes)?;
        let containers = container_names(&cli.cmd, count);
        let net = Net::start(
            cli.genesis_port,
            count,
            cli.move_ports,
            cli.listen_port,
            containers.clone(),
        )?;
        let ports = net.listen_ports();
        let commands = node_commands(&cli.cmd, ports, cli.move_ports, &containers)?;
        for (_, fault) in &cli.fault {
            fault.check(count).map_err(anyhow::Error::msg)?;
        }
        let mut faults = cli.fault.clone();
        faults.sort_by_key(|(at, _)| *at);
        let labels: Vec<String> = commands.iter().map(|(label, ..)| label.clone()).collect();
        let heads = (cli.head.iter())
            .map(|arg| Head::parse(arg, &labels))
            .collect::<Result<Vec<_>>>()?;
        Ok((commands, Some(net), faults, heads))
    };
    let (commands, net, faults, heads) = match setup() {
        Ok(setup) => setup,
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(1);
        }
    };

    if let Some(duration) = cli.headless {
        let net = net.expect("headless runs nodes");
        match run_headless(commands, net, faults, duration, heads, cli.timeout).await {
            Ok(code) => process::exit(code),
            Err(e) => {
                eprintln!("Error: {:#}", e);
                process::exit(1);
            }
        }
    }
    if let Err(e) = run_app(cli, commands, net, faults).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}

/// Create this run's log dir with its command line in cmd.txt, and start the nodes
fn spawn_nodes(
    commands: Vec<(String, CommandBuilder, Option<String>)>,
) -> Result<(PathBuf, Vec<Node>)> {
    let datetime = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let log_dir = PathBuf::from("logs").join(datetime);
    std::fs::create_dir_all(&log_dir)?;
    // The command line of this run, to rerun it or attach it to a bug report
    let quote = |a: String| {
        let plain = a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,".contains(c));
        match plain {
            true => a,
            false => format!("'{}'", a.replace('\'', "'\\''")),
        }
    };
    let quoted: Vec<String> = std::env::args().map(quote).collect();
    std::fs::write(log_dir.join("cmd.txt"), quoted.join(" ") + "\n")?;

    let nodes = commands
        .into_iter()
        .enumerate()
        .map(|(id, (label, cmd, container))| Node::spawn(id, label, &log_dir, cmd, container))
        .collect::<Result<Vec<Node>>>()?;
    Ok((log_dir, nodes))
}

/// Apply each --fault when due; one that cuts nodes off first waits, as long as the relay does,
/// for every node to listen behind it
fn apply_due(faults: &mut Vec<(Duration, Fault)>, started: Instant, nodes: &mut [Node], net: &Net) {
    while let Some((at, fault)) = faults.first() {
        let waiting = fault.cuts() && !net.ready() && started.elapsed() < crate::net::RELAY_WAIT;
        if started.elapsed() < *at || waiting {
            break;
        }
        fault.apply(nodes, net);
        faults.remove(0);
    }
}

/// A --head: the regex whose first group is a node's head, for one --cmd label or all nodes
struct Head {
    label: Option<String>,
    regex: Regex,
}

impl Head {
    /// Parse `[<label>=]REGEX`; the prefix is a label only if some --cmd has it
    fn parse(arg: &str, labels: &[String]) -> Result<Head> {
        let (label, pattern) = match arg.split_once('=') {
            Some((label, rest)) if labels.iter().any(|l| l == label) => {
                (Some(label.to_string()), rest)
            }
            _ => (None, arg),
        };
        let regex = Regex::new(pattern)?;
        anyhow::ensure!(
            regex.captures_len() > 1,
            "--head '{arg}' has no capture group for the head"
        );
        Ok(Head { label, regex })
    }
}

/// Each node's head per --head: the first group of its last matching log line. None for a node
/// with no match, and no entry for a stopped node or one no --head applies to
fn node_heads(nodes: &[Node], heads: &[Head]) -> Vec<(usize, Option<String>)> {
    let for_node = |node: &Node| {
        (heads
            .iter()
            .find(|h| h.label.as_deref() == Some(node.label.as_str())))
        .or_else(|| heads.iter().find(|h| h.label.is_none()))
    };
    (nodes.iter())
        .filter(|node| !node.is_stopped())
        .filter_map(|node| {
            let head = for_node(node)?;
            let text = node.get_text_tail(100_000);
            let last = head.regex.captures_iter(&text).last();
            Some((
                node.id,
                last.and_then(|c| Some(c.get(1)?.as_str().to_string())),
            ))
        })
        .collect()
}

/// Run the nodes without the UI for `duration`, then wait up to `timeout` for --head to find them
/// on one head, stop them and print a summary. Returns the exit code.
async fn run_headless(
    commands: Vec<(String, CommandBuilder, Option<String>)>,
    net: Net,
    mut faults: Vec<(Duration, Fault)>,
    duration: Duration,
    heads: Vec<Head>,
    timeout: Duration,
) -> Result<i32> {
    let (log_dir, mut nodes) = spawn_nodes(commands)?;
    println!("{} nodes, logs in {}", nodes.len(), log_dir.display());
    let interrupted = Arc::new(AtomicBool::new(false));
    let on_ctrl_c = interrupted.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        on_ctrl_c.store(true, Ordering::Relaxed);
    });

    let started = Instant::now();
    let checking = !heads.is_empty();
    let agree = |found: &[(usize, Option<String>)]| {
        let first = found.first().and_then(|(_, h)| h.as_ref());
        first.is_some() && found.iter().all(|(_, h)| h.as_ref() == first)
    };
    let mut found = Vec::new();
    while !interrupted.load(Ordering::Relaxed) {
        apply_due(&mut faults, started, &mut nodes, &net);
        let elapsed = started.elapsed();
        if elapsed >= duration && checking {
            found = node_heads(&nodes, &heads);
            if agree(&found) || elapsed >= duration + timeout {
                break;
            }
        } else if elapsed >= duration {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    node::stop_all(&nodes, Duration::from_secs(3));

    let name = |id: usize| format!("{} {id}", nodes[id].label);
    let code = match (interrupted.load(Ordering::Relaxed), checking) {
        (true, _) => {
            println!("interrupted");
            130
        }
        (false, false) => 0,
        (false, true) if agree(&found) => {
            let head = found[0].1.as_deref().unwrap_or_default();
            println!(
                "same head on {} nodes after {}s: {head}",
                found.len(),
                started.elapsed().as_secs()
            );
            0
        }
        (false, true) => {
            println!(
                "nodes on different heads {}s after the run:",
                timeout.as_secs()
            );
            let mut groups: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
            for (id, head) in found.clone() {
                groups.entry(head).or_default().push(name(id));
            }
            for (head, members) in groups {
                let head = head.unwrap_or("no match".to_string());
                println!("  {head}  {}", members.join(", "));
            }
            1
        }
    };
    let skipped: Vec<String> = (0..nodes.len())
        .filter(|id| checking && !found.iter().any(|(f, _)| f == id))
        .map(name)
        .collect();
    if !skipped.is_empty() {
        println!(
            "not checked (stopped, or no --head for its label): {}",
            skipped.join(", ")
        );
    }
    println!("logs in {}", log_dir.display());
    Ok(code)
}

async fn run_app(
    cli: Cli,
    commands: Vec<(String, CommandBuilder, Option<String>)>,
    net: Option<Net>,
    mut faults: Vec<(Duration, Fault)>,
) -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
    let _guard = TerminalGuard;

    let mut view_mode = ViewMode::Grid;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let (mut nodes, mut app) = if let Some(log_folder) = &cli.log_folder {
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
        let (_, nodes) = spawn_nodes(commands)?;
        let app = AppState::new(nodes.len(), cli.search);
        (nodes, app)
    };

    let mut focused = 0usize;
    let started = Instant::now();

    loop {
        if let Some(net) = &net {
            apply_due(&mut faults, started, &mut nodes, net);
        }

        terminal.draw(|f| {
            match view_mode {
                ViewMode::Grid => {
                    grid::render(f, &nodes, focused, &app, net.as_ref());
                }
                ViewMode::Zoomed => {
                    focused::render(f, &nodes[focused], &app, net.as_ref());
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
                    match handle_key_event(
                        k.code,
                        k.modifiers,
                        &mut app,
                        &mut focused,
                        &mut nodes,
                        net.as_ref(),
                    ) {
                        Action::Quit => {
                            node::stop_all(&nodes, Duration::from_secs(3));
                            // Cleanup and exit (process::exit terminates immediately, so guard won't drop)
                            let _ = disable_raw_mode();
                            let _ = execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show);
                            process::exit(0);
                        }
                        Action::ChangeView(new_view) => {
                            view_mode = new_view;
                        }
                        Action::Continue => {}
                    }
                }
                Event::Mouse(m) => {
                    let zoomed = matches!(view_mode, ViewMode::Zoomed);
                    let action = handle_mouse_event(m, &mut app, &mut focused, &nodes, zoomed);
                    if let Action::ChangeView(new_view) = action {
                        view_mode = new_view;
                    }
                }
                _ => {}
            }
        }
    }
}
