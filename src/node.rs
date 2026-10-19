use anyhow::Result;
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use strip_ansi_escapes::strip_str;
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
        telemetry: Option<TcpListener>,
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
        if let Some(listener) = telemetry {
            node.listen_telemetry(listener);
        }
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
                append(
                    &crash_msg,
                    Some(&mut log_file),
                    &buffer,
                    &paused_buffer,
                    &paused,
                );
                return;
            }
        };

        task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];
            // Bytes of an escape sequence or character cut off at the end of a read
            let mut held = Vec::new();

            loop {
                let (text, ended) = match reader.read(&mut buf) {
                    // EOF - node process ended
                    Ok(0) => (format!("\n[Node {} process ended]\n", id), true),
                    // Error reading - node may have crashed
                    Err(_) => (
                        format!("\n[Node {id} connection error - process may have crashed]\n"),
                        true,
                    ),
                    Ok(n) => {
                        held.extend_from_slice(&buf[..n]);
                        let whole = complete_prefix(&held);
                        let text = keep_colors(&String::from_utf8_lossy(&held[..whole]));
                        held.drain(..whole);
                        (text, false)
                    }
                };
                append(&text, Some(&mut log_file), &buffer, &paused_buffer, &paused);
                if ended {
                    break;
                }
            }
            // Reap the shell, so a zombie does not count as alive in `signal("0")`
            let _ = child.wait();
        });
    }

    /// Read the node's JIP-3 telemetry connections on `listener`, one at a time (a restarted node
    /// connects again), and write each best block change to its log and pane as
    /// `[jip3] best block <slot> <hash>`
    fn listen_telemetry(&self, listener: TcpListener) {
        let buffer = self.buffer.clone();
        let paused_buffer = self.paused_buffer.clone();
        let paused = self.paused.clone();
        let log_path = self.log_path.clone();
        thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut log_file = (log_path.as_ref())
                    .and_then(|p| OpenOptions::new().append(true).open(p).ok());
                // The first message is the node information
                let messages = std::iter::from_fn(|| read_message(&mut stream)).skip(1);
                for line in messages.filter_map(|m| best_block(&m)) {
                    append(&line, log_file.as_mut(), &buffer, &paused_buffer, &paused);
                }
            }
        });
    }

    /// Write a jamboree marker line into this node's log and pane, e.g. when a fault starts.
    pub fn mark(&self, text: &str) {
        let time = chrono::Local::now().format("%H:%M:%S%.3f");
        self.note(&format!("\n=== jamboree {time} {text} ===\n"));
    }

    /// Append a jamboree line to this node's log and pane
    fn note(&self, line: &str) {
        let mut log_file = self
            .log_path
            .as_ref()
            .and_then(|p| OpenOptions::new().append(true).open(p).ok());
        append(
            line,
            log_file.as_mut(),
            &self.buffer,
            &self.paused_buffer,
            &self.paused,
        );
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
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

    /// The last `max_bytes` of the pane's text, without colours
    pub fn get_text_tail(&self, max_bytes: usize) -> String {
        strip_str(self.get_colored_tail(max_bytes))
    }

    /// The last `max_bytes` of the pane's text, with the client's colours (SGR sequences)
    pub fn get_colored_tail(&self, max_bytes: usize) -> String {
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

        // Add shutdown message to log and buffer
        self.note(&format!("\n[Node {} shutdown by user]\n", self.id));
    }
}

/// Write a jamboree marker line into every node's log and pane
pub fn mark_all(nodes: &[Node], text: &str) {
    nodes.iter().for_each(|n| n.mark(text));
}

/// SIGSTOP node i, or SIGCONT it (`docker pause`/`unpause` for a container, which SIGSTOP
/// cannot reach), and mark it in every log
pub fn set_stopped(nodes: &[Node], i: usize, on: bool) {
    let node = &nodes[i];
    let done = match &node.container {
        Some(name) => docker(&[if on { "pause" } else { "unpause" }, name]),
        None => node.signal(if on { "STOP" } else { "CONT" }),
    };
    if !done {
        return node.mark("stop failed: the node is not running");
    }
    node.stopped.store(on, Ordering::Relaxed);
    let what = if on { "stopped" } else { "continued" };
    mark_all(nodes, &format!("{} {} {}", node.label, node.id, what));
}

/// Kill node i and run its command again, and mark it in every log; the other nodes keep
/// running
pub fn restart(nodes: &mut [Node], i: usize) {
    stop_all(&nodes[i..=i], Duration::from_secs(3));
    match nodes[i].start() {
        Ok(()) => mark_all(nodes, &format!("{} {} restarted", nodes[i].label, i)),
        Err(e) => nodes[i].mark(&format!("restart failed: {e:#}")),
    }
}

/// TERM every node, give them `grace` to exit, then KILL what is left. A stopped node is
/// continued first, and a container removed at the end: KILL stops only the `docker run`
/// client.
pub fn stop_all(nodes: &[Node], grace: Duration) {
    for node in nodes {
        if node.stopped.swap(false, Ordering::Relaxed) {
            match &node.container {
                Some(name) => docker(&["unpause", name]),
                None => node.signal("CONT"),
            };
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

/// The node's output for its pane: colours (SGR sequences) kept, other escapes and \r removed
fn keep_colors(text: &str) -> String {
    static SGR: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new("\x1b\\[[0-9;]*m").unwrap());
    let mut out = String::new();
    let mut last = 0;
    for sgr in SGR.find_iter(text) {
        out += &strip_str(&text[last..sgr.start()]);
        out += sgr.as_str();
        last = sgr.end();
    }
    out += &strip_str(&text[last..]);
    out.replace('\r', "")
}

/// Length of `bytes` without a trailing escape sequence or UTF-8 character that the next read
/// completes
fn complete_prefix(bytes: &[u8]) -> usize {
    let mut end = match std::str::from_utf8(bytes) {
        Err(e) if e.error_len().is_none() => e.valid_up_to(),
        _ => bytes.len(),
    };
    if let Some(esc) = bytes[..end].iter().rposition(|&b| b == 0x1b) {
        // CSI ends with a byte in @..~; a lone ESC or an unfinished CSI waits (up to 64 bytes)
        let rest = &bytes[esc + 1..end];
        let open = match rest.first() {
            None => true,
            Some(b'[') => !rest[1..].iter().any(|b| (0x40..=0x7e).contains(b)),
            _ => false,
        };
        if open && end - esc < 64 {
            end = esc;
        }
    }
    end
}

/// Append text to the log file without colours, and to the pane, or to the held-back buffer
/// while frozen.
fn append(
    text: &str,
    log_file: Option<&mut File>,
    buffer: &Mutex<String>,
    paused_buffer: &Mutex<String>,
    paused: &Mutex<bool>,
) {
    if let Some(log_file) = log_file {
        let _ = log_file.write_all(strip_str(text).as_bytes());
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

/// One JIP-3 message: its size as a little-endian u32, then the content
fn read_message(stream: &mut impl Read) -> Option<Vec<u8>> {
    let mut size = [0; 4];
    stream.read_exact(&mut size).ok()?;
    let size = u32::from_le_bytes(size) as usize;
    if size > 1 << 24 {
        return None;
    }
    let mut message = vec![0; size];
    stream.read_exact(&mut message).ok()?;
    Some(message)
}

/// The pane line for a JIP-3 "best block changed" event (timestamp u64, discriminator 11, slot
/// u32, header hash); None for other events
fn best_block(message: &[u8]) -> Option<String> {
    if message.len() < 45 || message[8] != 11 {
        return None;
    }
    let slot = u32::from_le_bytes(message[9..13].try_into().ok()?);
    let hash: String = message[13..45].iter().map(|b| format!("{b:02x}")).collect();
    Some(format!("[jip3] best block {slot} 0x{hash}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(content: &[u8]) -> Vec<u8> {
        [&(content.len() as u32).to_le_bytes()[..], content].concat()
    }

    fn best_block_event(slot: u32, hash: u8) -> Vec<u8> {
        [&7u64.to_le_bytes()[..], &[11], &slot.to_le_bytes(), &[hash; 32]].concat()
    }

    #[test]
    fn best_block_reads_only_best_block_events() {
        let line = best_block(&best_block_event(260, 0xab)).unwrap();
        assert_eq!(line, format!("[jip3] best block 260 0x{}\n", "ab".repeat(32)));
        let finalized = [&7u64.to_le_bytes()[..], &[12], &260u32.to_le_bytes(), &[0; 32]].concat();
        assert_eq!(best_block(&finalized), None);
        assert_eq!(best_block(&[0; 9]), None);
    }

    #[test]
    fn read_message_splits_frames_and_stops_at_a_cut_one() {
        let bytes = [frame(b"info"), frame(b"event"), frame(b"cut")[..5].to_vec()].concat();
        let mut stream = &bytes[..];
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"info"[..]));
        assert_eq!(read_message(&mut stream).as_deref(), Some(&b"event"[..]));
        assert_eq!(read_message(&mut stream), None);
    }

    #[test]
    fn telemetry_lines_reach_the_pane_after_a_reconnect() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let node = Node::from_log_file(0, &PathBuf::from("no-such-log")).unwrap();
        node.listen_telemetry(listener);
        for slot in [1, 2] {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            let bytes = [frame(b"node info"), frame(&best_block_event(slot, slot as u8))];
            stream.write_all(&bytes.concat()).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while node.get_text_tail(1000).lines().count() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let text = node.get_text_tail(1000);
        assert!(text.starts_with("[jip3] best block 1 0x0101"), "{text}");
        assert!(text.contains("\n[jip3] best block 2 0x0202"), "{text}");
    }

    #[test]
    fn keep_colors_drops_other_escapes() {
        let out = keep_colors("\x1b[2K\x1b[32m[info]\x1b[0m ok\r\n\x1b]0;title\x07next\n");
        assert_eq!(out, "\x1b[32m[info]\x1b[0m ok\nnext\n");
    }

    #[test]
    fn complete_prefix_holds_back_cut_sequences() {
        assert_eq!(complete_prefix(b"ab\x1b[3"), 2);
        assert_eq!(complete_prefix(b"ab\x1b"), 2);
        assert_eq!(complete_prefix(b"ab\x1b[32m"), 7);
        assert_eq!(complete_prefix("ab\u{e9}".as_bytes()), 4);
        assert_eq!(complete_prefix(&"ab\u{e9}".as_bytes()[..3]), 2);
    }
}
