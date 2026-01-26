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
fn scan_logs_for_blocks(log_dir: &PathBuf) -> Vec<BlockEntry> {
    let mut entries = Vec::new();

    for id in 0..6 {
        let log_path = log_dir.join(format!("node_{}.log", id));
        if !log_path.exists() {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&log_path) {
            for line in content.lines() {
                // Look for: "[BLOCK] ⛓️ Block created successfully. Header Hash 0x..."
                if let Some(hash_start) = line.find("[BLOCK] ⛓️ Block created successfully. Header Hash ") {
                    // Extract timestamp from beginning of line (format: "2026-01-26 11:01:30.018")
                    let timestamp = if line.len() >= 23 && line.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        line[..23].to_string()
                    } else {
                        "unknown".to_string()
                    };

                    // Extract block hash
                    let hash_offset = hash_start + "[BLOCK] ⛓️ Block created successfully. Header Hash ".len();
                    let block_hash = line[hash_offset..].trim().to_string();

                    entries.push(BlockEntry {
                        timestamp,
                        node_id: id,
                        block_hash,
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
fn write_summary(log_dir: &PathBuf, entries: &[BlockEntry]) -> std::io::Result<()> {
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
    for id in 0..6 {
        let count = entries.iter().filter(|e| e.node_id == id).count();
        if count > 0 {
            writeln!(file, "  Node {}: {} blocks", id, count)?;
        }
    }

    Ok(())
}

/// Generates the summary once (called on demand)
pub fn generate_summary(log_dir: &PathBuf) {
    let entries = scan_logs_for_blocks(log_dir);
    if let Err(e) = write_summary(log_dir, &entries) {
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
pub fn start_summary_task(log_dir: PathBuf) -> mpsc::Sender<SummaryCommand> {
    let (tx, mut rx) = mpsc::channel::<SummaryCommand>(10);

    let log_dir = Arc::new(log_dir);

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30));
        
        loop {
            tokio::select! {
                // Periodic tick every 30 seconds
                _ = ticker.tick() => {
                    generate_summary(&log_dir);
                }
                
                // Handle commands
                cmd = rx.recv() => {
                    match cmd {
                        Some(SummaryCommand::Generate) => {
                            generate_summary(&log_dir);
                        }
                        Some(SummaryCommand::Stop) | None => {
                            // Final generation before stopping
                            generate_summary(&log_dir);
                            break;
                        }
                    }
                }
            }
        }
    });

    tx
}
