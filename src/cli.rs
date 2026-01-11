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
}
