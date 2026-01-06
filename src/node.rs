use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use strip_ansi_escapes::strip;
use tokio::task;

pub struct Node {
    pub id: usize,
    master: Box<dyn MasterPty + Send>,
    buffer: Arc<Mutex<String>>,
    current_size: Arc<Mutex<PtySize>>,
    pub paused: Arc<Mutex<bool>>,
}

impl Node {
    pub fn spawn(id: usize, log_dir: &PathBuf) -> Result<Self> {
        let pty_system = NativePtySystem::default();

        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let master = pair.master;
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        // Assume structure: parent/jamixir/ and parent/jam-ui/
        let parent_dir = std::path::Path::new(manifest_dir).parent().unwrap();
        let jamixir_root = parent_dir.join("jamixir");

        let mut cmd = CommandBuilder::new(jamixir_root.join("node.sh"));
        cmd.arg(id.to_string());
        cmd.cwd(&jamixir_root);

        pair.slave.spawn_command(cmd)?;

        let buffer = Arc::new(Mutex::new(String::new()));
        let current_size = Arc::new(Mutex::new(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }));
        let paused = Arc::new(Mutex::new(false));

        let node = Node {
            id,
            master,
            buffer,
            current_size,
            paused,
        };

        // Start the reader task
        let log_path = log_dir.join(format!("node_{}.log", id));
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        node.start_reader_task(log_file);

        Ok(node)
    }

    fn start_reader_task(&self, mut log_file: File) {
        let id = self.id;
        let buffer = self.buffer.clone();
        let paused = self.paused.clone();
        let mut reader = match self.master.try_clone_reader() {
            Ok(r) => r,
            Err(_) => {
                let crash_msg = format!("\n[Node {} failed to create reader]\n", id);
                let mut b = buffer.lock().unwrap();
                b.push_str(&crash_msg);
                return;
            }
        };

        task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];

            loop {
                let n = match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF - node process ended
                        let crash_msg = format!("\n[Node {} process ended]\n", id);
                        let mut b = buffer.lock().unwrap();
                        b.push_str(&crash_msg);
                        break;
                    }
                    Err(_) => {
                        // Error reading - node may have crashed
                        let crash_msg = format!(
                            "\n[Node {} connection error - process may have crashed]\n",
                            id
                        );
                        let mut b = buffer.lock().unwrap();
                        b.push_str(&crash_msg);
                        break;
                    }
                    Ok(n) => n,
                };

                let text = String::from_utf8_lossy(&buf[..n]);

                // Strip ANSI codes for both log file and UI (clean text in logs, no escape codes)
                let clean_text = String::from_utf8_lossy(&strip(text.as_bytes())).replace('\r', "");

                // Write clean text (no ANSI codes) to log file
                let _ = log_file.write_all(clean_text.as_bytes());

                // Append to UI buffer
                let mut b = buffer.lock().unwrap();
                b.push_str(&clean_text);

                // Cap buffer size (avoid RAM blowup) - but only when NOT paused
                let is_paused = *paused.lock().unwrap();
                if !is_paused {
                    if b.len() > 100_000 {
                        let drain = b.len() - 100_000;
                        // Find the nearest character boundary to avoid splitting UTF-8 characters
                        let safe_drain = (0..=drain)
                            .rev()
                            .find(|&i| b.is_char_boundary(i))
                            .unwrap_or(0);
                        b.drain(..safe_drain);
                    }
                }
            }
        });
    }

    pub fn resize_pty_if_needed(&self, new_size: PtySize) {
        let mut current = self.current_size.lock().unwrap();
        if current.rows != new_size.rows || current.cols != new_size.cols {
            let _ = self.master.resize(new_size.clone());
            *current = new_size;
        }
    }

    pub fn get_text_tail(&self, max_bytes: usize) -> String {
        let b = self.buffer.lock().unwrap();
        let len = b.len();

        if len > max_bytes {
            let start = len - max_bytes;
            let safe_start = (start..len).find(|&i| b.is_char_boundary(i)).unwrap_or(len);
            b[safe_start..].to_string()
        } else {
            b.clone()
        }
    }
}
