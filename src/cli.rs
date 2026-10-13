use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "jam-ui", term_width = 80)]
pub struct Cli {
    /// Search terms (comma-separated, max 6)
    #[arg(short, long)]
    pub search: Option<String>,

    /// Load logs from folder  
    #[arg(short = 'l', long = "logs")]
    pub log_folder: Option<String>,

    /// Shell command that starts one node, run with `sh -c`; {i} is replaced by the node index
    #[arg(long, conflicts_with = "jamixir")]
    pub cmd: Option<String>,

    /// Number of nodes to start
    #[arg(long, default_value_t = 6)]
    pub nodes: usize,

    /// Jamixir checkout: runs <path>/node.sh <id> (default: a sibling jamixir/ folder)
    #[arg(long)]
    pub jamixir: Option<PathBuf>,

    /// Skip config: NODE_ID:SKIP_IDS (Jamixir only)
    #[arg(short = 'x', long = "skip-announcements-to", conflicts_with = "cmd")]
    pub skip_announcements_to: Option<String>,
}
