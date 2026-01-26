use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "jam-ui", term_width = 80)]
pub struct Cli {
    /// Search terms (comma-separated, max 6)
    #[arg(short, long)]
    pub search: Option<String>,

    /// Load logs from folder  
    #[arg(short = 'l', long = "logs")]
    pub log_folder: Option<String>,

    /// Skip config: NODE_ID:SKIP_IDS
    #[arg(short = 'x', long = "skip-announcements-to")]
    pub skip_announcements_to: Option<String>,
}
