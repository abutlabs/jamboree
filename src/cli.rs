use clap::{ArgGroup, Parser};

#[derive(Parser, Debug)]
#[command(name = "jam-ui", term_width = 80, arg_required_else_help = true)]
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

    /// Move each node off its genesis port to $PORT (also {port}), for clients whose port jam-ui
    /// cannot share; needed by `i` only then
    #[arg(long, conflicts_with = "log_folder")]
    pub move_ports: bool,

    /// With --move-ports, node i listens on PORT+i instead of a free port
    #[arg(long, value_name = "PORT", requires = "move_ports")]
    pub listen_port: Option<u16>,
}
