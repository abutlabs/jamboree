use crate::event::{Fault, parse_duration};
use clap::{ArgGroup, Parser};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "jamboree", version, about, max_term_width = 100, arg_required_else_help = true)]
#[command(after_help = "\
Examples:
  jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}'
  jamboree --cmd 'theirs=./their-node {i}' --cmd 'mine=./my-node {i}'
  jamboree --cmd '...' --fault '60s split 0,1,2/3,4,5' --fault '120s heal'
  jamboree --cmd '... --telemetry {telemetry}' --headless 3m --head jip3
  jamboree --logs logs/20261018_145800

Keys: arrows or h/l focus, up/down scroll, Enter zoom, Space freeze, / search,
s i p r stop/isolate/split/restart, ? all keys, q quit.
Docs: https://github.com/jamixir/jamboree")]
#[command(group(ArgGroup::new("source").required(true).args(["cmd", "log_folder"])))]
pub struct Cli {
    /// Shell command that starts a node, run with `sh -c`; {i} is replaced by the node index,
    /// {port} by the port it listens on ($PORT) and {telemetry} by a HOST:PORT for its JIP-3
    /// telemetry ($TELEMETRY; `--telemetry {telemetry}`). Repeat it to run one node per command; prefix
    /// `<label>=` to name the panes, or `<label>[K]=` to run K such nodes
    #[arg(help_heading = "Start", long)]
    pub cmd: Vec<String>,

    /// Print each node's command with the placeholders filled in, and exit
    #[arg(help_heading = "Start", long, conflicts_with = "log_folder")]
    pub dry_run: bool,

    /// Number of nodes to start (default 6, or one per --cmd); with --logs, number of files to load (default all)
    #[arg(help_heading = "Start", long)]
    pub nodes: Option<usize>,

    /// Terms to highlight from the start (comma-separated, max 6)
    #[arg(help_heading = "Start", short, long)]
    pub search: Option<String>,

    /// Open a folder of *.log files instead of starting nodes: a past run, or any other logs
    #[arg(help_heading = "Start", short = 'l', long = "logs")]
    pub log_folder: Option<String>,

    /// Port of node 0 in the genesis; node i is dialled on PORT+i
    #[arg(help_heading = "Network", long, value_name = "PORT", default_value_t = 40000)]
    pub genesis_port: u16,

    /// Move each node off its genesis port to $PORT (also {port}), for clients whose port jamboree
    /// cannot share; needed by `i` only then
    #[arg(help_heading = "Network", long, conflicts_with = "log_folder")]
    pub move_ports: bool,

    /// With --move-ports, node i listens on PORT+i instead of a free port
    #[arg(help_heading = "Network", long, value_name = "PORT", requires = "move_ports")]
    pub listen_port: Option<u16>,

    /// Fault on a schedule: `[<time>] <action>`, time like 90s or 2m after the
    /// start (default at the start). Actions: stop N, isolate N, split A/B (e.g.
    /// 0,1,2/3,4,5), restart N, heal (continue, un-isolate and unsplit all). Repeatable
    #[arg(help_heading = "Network", long, value_name = "FAULT", value_parser = Fault::parse, conflicts_with = "log_folder")]
    pub fault: Vec<(Duration, Fault)>,

    /// Hold back every packet between nodes by DELAY (e.g. 100ms), in each direction
    #[arg(long, value_name = "DELAY", value_parser = parse_duration, default_value = "0ms", help_heading = "Network", conflicts_with = "log_folder")]
    pub delay: Duration,

    /// Spread --delay by up to JITTER either way (e.g. 20ms), so packets can arrive out of order
    #[arg(long, value_name = "JITTER", value_parser = parse_duration, default_value = "0ms", help_heading = "Network", conflicts_with = "log_folder")]
    pub jitter: Duration,

    /// Drop PERCENT of the packets between nodes at random (e.g. 1), in each direction
    #[arg(long, value_name = "PERCENT", default_value_t = 0.0, help_heading = "Network", conflicts_with = "log_folder")]
    pub loss: f64,

    /// Add a random fault schedule from SEED: a stop, isolate, split or restart every 20-60s,
    /// healed 20-60s later. The same seed and node count give the same schedule; it is saved as
    /// --fault arguments in the run's faults.txt. Without --headless it runs for an hour
    #[arg(long, value_name = "SEED", help_heading = "Network", conflicts_with = "log_folder")]
    pub chaos: Option<u64>,

    /// Run without the UI for DURATION (e.g. 5m), for CI and scripts; then stop the nodes, print
    /// a summary and exit 0, or 1 when --head finds them on different heads
    #[arg(help_heading = "Headless", long, value_name = "DURATION", value_parser = parse_duration, conflicts_with = "log_folder")]
    pub headless: Option<Duration>,

    /// With --headless, check at the end that all nodes are on the same head: the first group
    /// that REGEX captures in each node's last matching log line. Prefix `<label>=` to apply it
    /// to that --cmd label's nodes only. `jip3` as REGEX takes the best block from the nodes'
    /// JIP-3 telemetry instead, `jip3-final` the finalized block. Repeatable
    #[arg(help_heading = "Headless", long, value_name = "[LABEL=]REGEX", requires = "headless")]
    pub head: Vec<String>,

    /// How long --head waits for the nodes to agree after DURATION
    #[arg(help_heading = "Headless", long, value_name = "DURATION", value_parser = parse_duration, default_value = "60s", requires = "head")]
    pub timeout: Duration,
}
