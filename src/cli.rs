use crate::event::{Fault, parse_duration};
use clap::{ArgGroup, Parser};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "jamboree", term_width = 80, arg_required_else_help = true)]
#[command(group(ArgGroup::new("source").required(true).args(["cmd", "log_folder"])))]
pub struct Cli {
    /// Search terms (comma-separated, max 6)
    #[arg(short, long)]
    pub search: Option<String>,

    /// Load logs from folder  
    #[arg(short = 'l', long = "logs")]
    pub log_folder: Option<String>,

    /// Shell command that starts a node, run with `sh -c`; {i} is replaced by the node index and
    /// {port} by the port it listens on. Repeat it to run one node per command; prefix
    /// `<label>=` to name the panes
    #[arg(long)]
    pub cmd: Vec<String>,

    /// Number of nodes to start (default 6, or one per --cmd); with --logs, number of files to load (default all)
    #[arg(long)]
    pub nodes: Option<usize>,

    /// Port of node 0 in the genesis; node i is dialled on PORT+i
    #[arg(long, value_name = "PORT", default_value_t = 40000)]
    pub genesis_port: u16,

    /// Move each node off its genesis port to $PORT (also {port}), for clients whose port jamboree
    /// cannot share; needed by `i` only then
    #[arg(long, conflicts_with = "log_folder")]
    pub move_ports: bool,

    /// With --move-ports, node i listens on PORT+i instead of a free port
    #[arg(long, value_name = "PORT", requires = "move_ports")]
    pub listen_port: Option<u16>,

    /// Fault to apply, as its key would: `[<time>] <action>`, time like 90s or 2m after the
    /// start (default at the start). Actions: stop N, isolate N, split A/B (e.g.
    /// 0,1,2/3,4,5), restart N, heal (continue, un-isolate and unsplit all). Repeatable
    #[arg(long, value_name = "FAULT", value_parser = Fault::parse, conflicts_with = "log_folder")]
    pub fault: Vec<(Duration, Fault)>,

    /// Run without the UI for DURATION (e.g. 5m), for CI and scripts; then stop the nodes, print
    /// a summary and exit 0, or 1 when --head finds them on different heads
    #[arg(long, value_name = "DURATION", value_parser = parse_duration, conflicts_with = "log_folder")]
    pub headless: Option<Duration>,

    /// With --headless, check at the end that all nodes are on the same head: the first group
    /// that REGEX captures in each node's last matching log line. Prefix `<label>=` to apply it
    /// to that --cmd label's nodes only. Repeatable
    #[arg(long, value_name = "[LABEL=]REGEX", requires = "headless")]
    pub head: Vec<String>,

    /// How long --head waits for the nodes to agree after DURATION
    #[arg(long, value_name = "DURATION", value_parser = parse_duration, default_value = "60s", requires = "head")]
    pub timeout: Duration,
}
