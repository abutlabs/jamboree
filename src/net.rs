//! Network faults between nodes (Linux). jam-ui runs in its own user and network namespace
//! (`--net-faults` re-runs it under `systemd-run --user --scope unshare -Urn`), each node in
//! its own cgroup. nftables marks every packet with its sender's node and silently drops it at
//! a receiver in another group, so nothing in the client has to change.

use anyhow::{bail, Context, Result};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

/// Set in the re-run process, so it does not re-run itself again
const INSIDE: &str = "JAM_UI_NET_FAULTS";

/// Re-run jam-ui in a private network namespace with a delegated cgroup. Returns only if
/// already inside one.
pub fn reexec_isolated() -> Result<()> {
    use std::os::unix::process::CommandExt;
    if std::env::var_os(INSIDE).is_some() {
        return Ok(());
    }
    let err = Command::new("systemd-run")
        .args(["--user", "--scope", "-q", "--", "unshare", "-Urn"])
        .arg(std::env::current_exe()?)
        .args(std::env::args_os().skip(1))
        .env(INSIDE, "1")
        .exec();
    Err(err).context("--net-faults needs systemd-run and unshare")
}

pub struct Net {
    /// Our cgroup, under which node i gets n<i>
    cgroup: PathBuf,
    /// The same path relative to the cgroup root, as nft matches it
    rel: String,
    /// Partition group of each node; packets between different groups are dropped
    groups: Vec<usize>,
}

impl Net {
    pub fn new() -> Result<Self> {
        let status = Command::new("ip")
            .args(["link", "set", "lo", "up"])
            .status()?;
        anyhow::ensure!(status.success(), "ip link set lo up failed");
        let own = std::fs::read_to_string("/proc/self/cgroup")?;
        let rel = own.trim().trim_start_matches("0::/").to_string();
        nft("add table inet jam\n\
             add chain inet jam out { type filter hook output priority 0; }\n\
             add chain inet jam in { type filter hook input priority 0; }\n")?;
        Ok(Net {
            cgroup: PathBuf::from("/sys/fs/cgroup").join(&rel),
            rel,
            groups: Vec::new(),
        })
    }

    /// Give node i its cgroup and sender mark; returns the shell prefix that moves the
    /// node's `sh -c` (and so everything it starts) into that cgroup.
    pub fn add_node(&mut self, i: usize) -> Result<String> {
        let dir = self.cgroup.join(format!("n{i}"));
        std::fs::create_dir_all(&dir)?;
        nft(&format!(
            "add rule inet jam out {} meta mark set {}\n",
            self.socket(i),
            i + 1
        ))?;
        self.groups.push(0);
        Ok(format!("echo $$ > {}/cgroup.procs; ", dir.display()))
    }

    pub fn is_isolated(&self, i: usize) -> bool {
        self.groups.get(i).is_some_and(|&g| g != 0)
    }

    /// Cut node i off from every other node, or heal it. Returns whether it is now isolated.
    pub fn toggle_isolated(&mut self, i: usize) -> Result<bool> {
        self.groups[i] = if self.groups[i] == 0 { i + 1 } else { 0 };
        let mut rules = String::from("flush chain inet jam in\n");
        for (a, ga) in self.groups.iter().enumerate() {
            for (b, gb) in self.groups.iter().enumerate() {
                if ga != gb {
                    rules += &format!(
                        "add rule inet jam in meta mark {} {} drop\n",
                        a + 1,
                        self.socket(b)
                    );
                }
            }
        }
        nft(&rules)?;
        Ok(self.is_isolated(i))
    }

    /// nft match for sockets of node i: its cgroup, at its depth below the root
    fn socket(&self, i: usize) -> String {
        let path = format!("{}/n{i}", self.rel);
        format!(
            "socket cgroupv2 level {} \"{}\"",
            path.split('/').count(),
            path
        )
    }
}

fn nft(rules: &str) -> Result<()> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child.stdin.take().unwrap().write_all(rules.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("nft: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}
