use clap::Parser;
use regex::Regex;
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

    /// Number of nodes to start (default 6); with --logs, number of files to load (default all)
    #[arg(long)]
    pub nodes: Option<usize>,

    /// Jamixir checkout: runs <path>/node.sh <id> (default: a sibling jamixir/ folder)
    #[arg(long)]
    pub jamixir: Option<PathBuf>,

    /// Block-authored line pattern for summary.log, capture group 1 = block hash
    /// (default: Jamixir's line; with --cmd and no pattern, no summary is written)
    #[arg(long, value_parser = parse_summary_regex)]
    pub summary_regex: Option<Regex>,

    /// Skip config: NODE_ID:SKIP_IDS (Jamixir only)
    #[arg(short = 'x', long = "skip-announcements-to", conflicts_with = "cmd")]
    pub skip_announcements_to: Option<String>,
}

fn parse_summary_regex(s: &str) -> Result<Regex, String> {
    let re = Regex::new(s).map_err(|e| e.to_string())?;
    if re.captures_len() < 2 {
        return Err("needs a capture group for the block hash".into());
    }
    Ok(re)
}
