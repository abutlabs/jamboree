use regex::Regex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

/// A parsed block entry from the logs
#[derive(Debug, Clone)]
struct BlockEntry {
    timestamp: String,
    node_id: usize,
    block_hash: String,
}

/// Scans all node log files and extracts block creation events
fn scan_logs_for_blocks(log_dir: &PathBuf, node_count: usize, block_regex: &Regex) -> Vec<BlockEntry> {
    let mut entries = Vec::new();

    for id in 0..node_count {
        let log_path = log_dir.join(format!("node_{}.log", id));
        if !log_path.exists() {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&log_path) {
            for line in content.lines() {
                if let Some(hash) = block_regex.captures(line).and_then(|c| c.get(1)) {
                    // Extract timestamp from beginning of line (format: "2026-01-26 11:01:30.018")
                    let timestamp = match line.get(..23) {
                        Some(ts) if ts.starts_with(|c: char| c.is_ascii_digit()) => ts.to_string(),
                        _ => "unknown".to_string(),
                    };

                    entries.push(BlockEntry {
                        timestamp,
                        node_id: id,
                        block_hash: hash.as_str().trim().to_string(),
                    });
                }
            }
        }
    }

    // Sort by timestamp
    entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    entries
}

/// Writes the summary log file
fn write_summary(log_dir: &PathBuf, node_count: usize, entries: &[BlockEntry]) -> std::io::Result<()> {
    use std::io::Write;

    let summary_path = log_dir.join("summary.log");
    let mut file = std::fs::File::create(&summary_path)?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
    writeln!(file, "BLOCK AUTHORING SUMMARY")?;
    writeln!(file, "Generated at: {}", now)?;
    writeln!(file, "Total blocks: {}", entries.len())?;
    writeln!(file, "{}", "=".repeat(100))?;
    writeln!(file, "{:<24} | {:<6} | {}", "Timestamp", "Node", "Block Hash")?;
    writeln!(file, "{}", "-".repeat(100))?;

    for entry in entries {
        writeln!(
            file,
            "{:<24} | {:<6} | {}",
            entry.timestamp, entry.node_id, entry.block_hash
        )?;
    }

    writeln!(file, "{}", "-".repeat(100))?;
    
    // Add per-node summary
    writeln!(file)?;
    writeln!(file, "BLOCKS PER NODE:")?;
    for id in 0..node_count {
        let count = entries.iter().filter(|e| e.node_id == id).count();
        if count > 0 {
            writeln!(file, "  Node {}: {} blocks", id, count)?;
        }
    }

    Ok(())
}

/// Generates the summary once (called on demand)
pub fn generate_summary(log_dir: &PathBuf, node_count: usize, block_regex: &Regex) {
    let entries = scan_logs_for_blocks(log_dir, node_count, block_regex);
    if let Err(e) = write_summary(log_dir, node_count, &entries) {
        eprintln!("Failed to write summary: {}", e);
    }
}

/// Message types for the summary task
pub enum SummaryCommand {
    /// Generate summary now
    Generate,
    /// Stop the background task
    Stop,
}

/// Starts the background summary generation task
/// Returns a sender that can be used to trigger summary generation or stop the task
pub fn start_summary_task(log_dir: PathBuf, node_count: usize, block_regex: Regex) -> mpsc::Sender<SummaryCommand> {
    let (tx, mut rx) = mpsc::channel::<SummaryCommand>(10);

    let log_dir = Arc::new(log_dir);

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                // Periodic tick every 30 seconds
                _ = ticker.tick() => {
                    generate_summary(&log_dir, node_count, &block_regex);
                }
                
                // Handle commands
                cmd = rx.recv() => {
                    match cmd {
                        Some(SummaryCommand::Generate) => {
                            generate_summary(&log_dir, node_count, &block_regex);
                        }
                        Some(SummaryCommand::Stop) | None => {
                            // Final generation before stopping
                            generate_summary(&log_dir, node_count, &block_regex);
                            break;
                        }
                    }
                }
            }
        }
    });

    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(lines: &str, block_regex: &Regex) -> Vec<BlockEntry> {
        let dir = std::env::temp_dir().join(format!("jam-ui-summary-{}-{}", std::process::id(), lines.len()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("node_1.log"), lines).unwrap();
        let entries = scan_logs_for_blocks(&dir, 2, block_regex);
        std::fs::remove_dir_all(&dir).unwrap();
        entries
    }

    #[test]
    fn jamixir_pattern_reads_timestamp_and_hash() {
        let entries = scan(
            "2026-01-26 11:01:30.018 [BLOCK] ⛓️ Block created successfully. Header Hash 0xabc \n\
             2026-01-26 11:01:31.000 [info] imported\n",
            &Regex::new(r"\[BLOCK\] ⛓️ Block created successfully\. Header Hash (.+)").unwrap(),
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, "2026-01-26 11:01:30.018");
        assert_eq!(entries[0].node_id, 1);
        assert_eq!(entries[0].block_hash, "0xabc");
    }

    #[test]
    fn custom_pattern_uses_first_capture_group() {
        let re = Regex::new(r"authored block hash=(0x[0-9a-f]+)").unwrap();
        let entries = scan("INFO authored block hash=0xdef slot=4\n", &re);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, "unknown");
        assert_eq!(entries[0].block_hash, "0xdef");
    }
}
