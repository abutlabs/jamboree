use clap::{ArgGroup, Parser};
use regex::Regex;

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

    /// Shell command that starts one node, run with `sh -c`; {i} is replaced by the node index
    #[arg(long)]
    pub cmd: Option<String>,

    /// Number of nodes to start (default 6); with --logs, number of files to load (default all)
    #[arg(long)]
    pub nodes: Option<usize>,

    /// Block-authored line pattern; when given, summary.log lists the matches (capture group 1 = block hash)
    #[arg(long, value_parser = parse_summary_regex)]
    pub summary_regex: Option<Regex>,
}

fn parse_summary_regex(s: &str) -> Result<Regex, String> {
    let re = Regex::new(s).map_err(|e| e.to_string())?;
    if re.captures_len() < 2 {
        return Err("needs a capture group for the block hash".into());
    }
    Ok(re)
}
