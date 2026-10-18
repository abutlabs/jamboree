use anyhow::Result;
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    process,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use strip_ansi_escapes::strip;
use tokio::task;

pub struct Node {
    pub id: usize,
    /// Pane title prefix: the --cmd label, or "Node"
    pub label: String,
    master: Option<Box<dyn MasterPty + Send>>,
    buffer: Arc<Mutex<String>>,
    paused_buffer: Arc<Mutex<String>>,
    current_size: Arc<Mutex<PtySize>>,
    pub paused: Arc<Mutex<bool>>,
    child: Option<Mutex<Box<dyn Child + Send + Sync>>>,
}

impl Node {
    pub fn spawn(id: usize, label: String, log_dir: &PathBuf, cmd: CommandBuilder) -> Result<Self> {
        let pty_system = NativePtySystem::default();

        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let master = pair.master;

        let buffer = Arc::new(Mutex::new(String::new()));
        let paused_buffer = Arc::new(Mutex::new(String::new()));
        let current_size = Arc::new(Mutex::new(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }));
        let paused = Arc::new(Mutex::new(false));
        let child = Some(Mutex::new(pair.slave.spawn_command(cmd)?));

        let node = Node {
            id,
            label,
            master: Some(master),
            buffer,
            paused_buffer,
            current_size,
            paused,
            child,
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
        let paused_buffer = self.paused_buffer.clone();
        let paused = self.paused.clone();
        let mut reader = match self.master.as_ref().unwrap().try_clone_reader() {
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
                let is_paused = *paused.lock().unwrap();

                if is_paused {
                    let mut pb = paused_buffer.lock().unwrap();
                    pb.push_str(&clean_text);
                } else {
                    let mut b = buffer.lock().unwrap();
                    b.push_str(&clean_text);

                    // cap ONLY main buffer
                    if b.len() > 100_000 {
                        let drain = b.len() - 100_000;
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
        if let Some(ref master) = self.master {
            let mut current = self.current_size.lock().unwrap();
            if current.rows != new_size.rows || current.cols != new_size.cols {
                let _ = master.resize(new_size.clone());
                *current = new_size;
            }
        }
    }

    /// Load node from log file (for log viewer mode)
    pub fn from_log_file(id: usize, log_path: &PathBuf) -> Result<Self> {
        let buffer = Arc::new(Mutex::new(String::new()));
        let paused_buffer = Arc::new(Mutex::new(String::new()));
        let current_size = Arc::new(Mutex::new(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }));
        let paused = Arc::new(Mutex::new(false));

        let node = Node {
            id,
            label: "Node".to_string(),
            master: None,
            buffer,
            paused_buffer,
            current_size,
            paused,
            child: None,
        };

        // Load log file content
        if log_path.exists() {
            let content = std::fs::read_to_string(log_path)?;
            let mut b = node.buffer.lock().unwrap();
            *b = content;
        }

        Ok(node)
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

    pub fn flush_paused_buffer(&self) {
        let mut paused = self.paused_buffer.lock().unwrap();
        if paused.is_empty() {
            return;
        }

        let mut main = self.buffer.lock().unwrap();
        main.push_str(&paused);
        paused.clear();
    }

    /// Send `sig` (TERM, KILL, STOP, CONT, or 0 to probe) to the node's whole process group,
    /// so it reaches the client and not just the `sh -c` around it. The pty makes that shell
    /// a session leader, so its pid is the group id. Returns false if nothing was signalled.
    pub fn signal(&self, sig: &str) -> bool {
        let Some(child) = &self.child else {
            return false;
        };
        let mut child = child.lock().unwrap();
        // Reap the shell if it exited, so a zombie does not count as alive
        let _ = child.try_wait();
        let Some(pid) = child.process_id() else {
            return false;
        };
        #[cfg(unix)]
        {
            process::Command::new("kill")
                .arg(format!("-{sig}"))
                .arg("--")
                .arg(format!("-{pid}"))
                .stderr(process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        }
        #[cfg(windows)]
        {
            matches!(sig, "TERM" | "KILL")
                && process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .status()
                    .is_ok_and(|s| s.success())
        }
    }

    pub fn shutdown(&self) {
        self.signal("TERM");

        // Add shutdown message to buffer
        let shutdown_msg = format!("\n[Node {} shutdown by user]\n", self.id);
        let mut b = self.buffer.lock().unwrap();
        b.push_str(&shutdown_msg);
    }
}

/// TERM every node, give them `grace` to exit, then KILL what is left.
pub fn stop_all(nodes: &[Node], grace: Duration) {
    for node in nodes {
        node.shutdown();
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline && nodes.iter().any(|n| n.signal("0")) {
        thread::sleep(Duration::from_millis(100));
    }
    for node in nodes {
        node.signal("KILL");
    }
}
