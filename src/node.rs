use anyhow::Result;
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
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
    /// Pid of the `sh -c` that runs the node; also its process group id
    pid: Option<u32>,
    log_path: Option<PathBuf>,
    /// Command that starts the node, kept for restarts
    cmd: Option<CommandBuilder>,
    /// Name of the node's Docker container, when its command uses `{container}`
    container: Option<String>,
    /// The node's process group is SIGSTOPped
    stopped: AtomicBool,
}

impl Node {
    pub fn spawn(
        id: usize,
        label: String,
        log_dir: &PathBuf,
        cmd: CommandBuilder,
        container: Option<String>,
    ) -> Result<Self> {
        let mut node = Node {
            id,
            label,
            master: None,
            buffer: Arc::new(Mutex::new(String::new())),
            paused_buffer: Arc::new(Mutex::new(String::new())),
            current_size: Arc::new(Mutex::new(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })),
            paused: Arc::new(Mutex::new(false)),
            pid: None,
            log_path: Some(log_dir.join(format!("node_{}.log", id))),
            cmd: Some(cmd),
            container,
            stopped: AtomicBool::new(false),
        };
        node.start()?;
        Ok(node)
    }

    /// Run the node's command in a new pty, appending to its log and pane
    fn start(&mut self) -> Result<()> {
        let (Some(cmd), Some(log_path)) = (&self.cmd, &self.log_path) else {
            return Ok(());
        };
        let size = *self.current_size.lock().unwrap();
        let pair = NativePtySystem::default().openpty(size)?;
        let child = pair.slave.spawn_command(cmd.clone())?;
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        self.pid = child.process_id();
        self.master = Some(pair.master);
        self.start_reader_task(log_file, child);
        Ok(())
    }

    /// Kill the node and run its command again; the other nodes keep running
    pub fn restart(&mut self) -> Result<()> {
        stop_all(std::slice::from_ref(self), Duration::from_secs(3));
        self.stopped.store(false, Ordering::Relaxed);
        self.start()
    }

    /// Copies the node's output to its log and pane; reaps the shell once the output ends.
    fn start_reader_task(&self, mut log_file: File, mut child: Box<dyn Child + Send + Sync>) {
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

                append(
                    &clean_text,
                    Some(&mut log_file),
                    &buffer,
                    &paused_buffer,
                    &paused,
                );
            }
            // Reap the shell, so a zombie does not count as alive in `signal("0")`
            let _ = child.wait();
        });
    }

    /// Write a jam-ui marker line into this node's log and pane, e.g. when a fault starts.
    pub fn mark(&self, text: &str) {
        let line = format!(
            "\n=== jam-ui {} {} ===\n",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            text
        );
        let mut log_file = self
            .log_path
            .as_ref()
            .and_then(|p| OpenOptions::new().append(true).open(p).ok());
        append(
            &line,
            log_file.as_mut(),
            &self.buffer,
            &self.paused_buffer,
            &self.paused,
        );
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    /// SIGSTOP the node, or SIGCONT it if it is stopped (`docker pause`/`unpause` for a
    /// container, which SIGSTOP cannot reach). Returns whether it is now stopped, or None if
    /// the node is not running.
    pub fn toggle_stopped(&self) -> Option<bool> {
        let stop = !self.stopped.load(Ordering::Relaxed);
        let done = match &self.container {
            Some(name) => docker(&[if stop { "pause" } else { "unpause" }, name]),
            None => self.signal(if stop { "STOP" } else { "CONT" }),
        };
        done.then(|| {
            self.stopped.store(stop, Ordering::Relaxed);
            stop
        })
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
            pid: None,
            log_path: None,
            cmd: None,
            container: None,
            stopped: AtomicBool::new(false),
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
        let Some(pid) = self.pid else {
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

/// TERM every node, give them `grace` to exit, then KILL what is left. A container is
/// unpaused first, and removed at the end: KILL stops only the `docker run` client.
pub fn stop_all(nodes: &[Node], grace: Duration) {
    for node in nodes {
        if let (Some(name), true) = (&node.container, node.is_stopped()) {
            docker(&["unpause", name]);
        }
        node.shutdown();
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline && nodes.iter().any(|n| n.signal("0")) {
        thread::sleep(Duration::from_millis(100));
    }
    for node in nodes {
        node.signal("KILL");
        if let Some(name) = &node.container {
            docker(&["rm", "-f", name]);
        }
    }
}

/// Run `docker <args>`; returns whether it succeeded
fn docker(args: &[&str]) -> bool {
    process::Command::new("docker")
        .args(args)
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Append text to the log file and to the pane, or to the held-back buffer while frozen.
fn append(
    text: &str,
    log_file: Option<&mut File>,
    buffer: &Mutex<String>,
    paused_buffer: &Mutex<String>,
    paused: &Mutex<bool>,
) {
    if let Some(log_file) = log_file {
        let _ = log_file.write_all(text.as_bytes());
    }

    if *paused.lock().unwrap() {
        paused_buffer.lock().unwrap().push_str(text);
        return;
    }

    let mut b = buffer.lock().unwrap();
    b.push_str(text);

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
