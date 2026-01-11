use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "jam-ui")]
pub struct Cli {
    /// Comma-separated search terms (max 6)
    #[arg(short, long)]
    pub search: Option<String>,
}
