use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "jam-ui")]
pub struct Cli {
    /// Comma-separated search terms (max 6)
    #[arg(short, long)]
    pub search: Option<String>,

    /// Load logs from folder instead of running nodes (relative path to log folder)
    #[arg(short = 'l', long = "logs")]
    pub log_folder: Option<String>,

    /// Per-node skip configuration: "NODE_ID:SKIP_IDS" or "NODE_ID:SKIP_IDS,NODE_ID2:SKIP_IDS2"
    /// Example: "0:4" means node 0 will skip announcements to node 4
    /// Example: "0:4,1:5" means node 0 skips node 4, node 1 skips node 5
    #[arg(short = 'x', long = "skip-announcements-to")]
    pub skip_announcements_to: Option<String>,
}
