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

    /// Shell command that starts a node, run with `sh -c`; {i} is replaced by the node index.
    /// Repeat it to run one node per command; prefix `<label>=` to name the panes
    #[arg(long)]
    pub cmd: Vec<String>,

    /// Number of nodes to start (default 6, or one per --cmd); with --logs, number of files to load (default all)
    #[arg(long)]
    pub nodes: Option<usize>,
}
