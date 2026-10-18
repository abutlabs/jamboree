//! Network faults: a UDP relay on each node's genesis port that drops a cut-off node's traffic.
//! Default: shares the genesis port with the node. --move-ports: the node moves to $PORT.

use anyhow::Result;
use socket2::{Domain, Socket, Type};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub struct Net {
    shared: Arc<Shared>,
}

struct Shared {
    genesis_port: u16,
    /// Port each node listens on: its genesis port, or its `$PORT` with --move-ports
    listen: Vec<u16>,
    moved: bool,
    /// Whether node j's relay is up
    relayed: Mutex<Vec<bool>>,
    /// Partition group of each node; datagrams between different groups are dropped
    groups: Mutex<Vec<usize>>,
    /// Which node owns a local UDP port, as last looked up
    owners: Mutex<HashMap<u16, (Option<usize>, Instant)>>,
}

impl Net {
    /// Relay `n` nodes. With `move_ports`, node i listens on `listen_base + i` or a free port.
    pub fn start(
        genesis_port: u16,
        n: usize,
        move_ports: bool,
        listen_base: Option<u16>,
    ) -> Result<Self> {
        let listen = (0..n)
            .map(|i| match (move_ports, listen_base) {
                (false, _) => Ok(genesis_port + i as u16),
                (true, Some(base)) => Ok(base + i as u16),
                (true, None) => Ok(UdpSocket::bind(("127.0.0.1", 0))?.local_addr()?.port()),
            })
            .collect::<std::io::Result<_>>()?;
        let shared = Arc::new(Shared {
            genesis_port,
            listen,
            moved: move_ports,
            relayed: Mutex::new(vec![false; n]),
            groups: Mutex::new(vec![0; n]),
            owners: Mutex::new(HashMap::new()),
        });
        let watcher = shared.clone();
        thread::spawn(move || watcher.relay_listening_nodes());
        Ok(Net { shared })
    }

    /// Port each node listens on
    pub fn listen_ports(&self) -> &[u16] {
        &self.shared.listen
    }

    pub fn is_isolated(&self, i: usize) -> bool {
        self.shared.groups.lock().unwrap()[i] != 0
    }

    /// Cut node i off from every other node, or heal it. Returns whether it is now isolated,
    /// or why it cannot be: every node must be behind the relay, or traffic leaks around it.
    pub fn toggle_isolated(&self, i: usize) -> Result<bool, String> {
        let direct: Vec<String> = (self.shared.relayed.lock().unwrap().iter())
            .enumerate()
            .filter(|(_, relayed)| !**relayed)
            .map(|(k, _)| k.to_string())
            .collect();
        if !direct.is_empty() {
            let (noun, verb) = if direct.len() == 1 {
                ("node", "does")
            } else {
                ("nodes", "do")
            };
            let why = match self.shared.moved {
                true => format!("{verb} not listen on $PORT"),
                false => format!("{verb} not share the genesis port; restart with --move-ports"),
            };
            return Err(format!(
                "cannot cut nodes off: {noun} {} {why}",
                direct.join(", ")
            ));
        }
        let mut groups = self.shared.groups.lock().unwrap();
        groups[i] = if groups[i] == 0 { i + 1 } else { 0 };
        Ok(groups[i] != 0)
    }
}

impl Shared {
    /// For the first two minutes, start node j's relay once the node listens
    fn relay_listening_nodes(self: Arc<Self>) {
        let until = Instant::now() + Duration::from_secs(120);
        while Instant::now() < until && self.relayed.lock().unwrap().contains(&false) {
            for j in 0..self.listen.len() {
                if self.relayed.lock().unwrap()[j] || pids_on(self.listen[j]).is_empty() {
                    continue;
                }
                let port = self.genesis_port + j as u16;
                let (fronts, node) = if self.moved {
                    // Clients that dial ::1 get the same relay, if IPv6 is there
                    let fronts =
                        [("127.0.0.1", port), ("::1", port)].map(|a| UdpSocket::bind(a).ok());
                    let node = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.listen[j]);
                    (fronts, node)
                } else {
                    // Forward on another loopback address, which the node's wildcard socket
                    // receives but the relay does not: 127.0.0.2 on Linux, ::1 on macOS
                    let node_ip = match cfg!(target_os = "linux") {
                        true => IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
                        false => IpAddr::V6(Ipv6Addr::LOCALHOST),
                    };
                    ([share_port(port), None], SocketAddr::new(node_ip, port))
                };
                // Without the 127.0.0.1 front the node stays direct
                let relayed = fronts[0].is_some();
                for front in fronts.into_iter().flatten() {
                    let shared = self.clone();
                    thread::spawn(move || relay(front, node, j, shared));
                }
                self.relayed.lock().unwrap()[j] = relayed;
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    /// Whether traffic between the sender on local port `port` and node j is cut
    fn cut(&self, port: u16, j: usize) -> bool {
        let groups = self.groups.lock().unwrap().clone();
        if groups.iter().all(|&g| g == 0) {
            return false;
        }
        self.owner(port, groups.len())
            .is_some_and(|k| groups[k] != groups[j])
    }

    /// The node whose process owns local UDP port `port`: the one that also owns a listen
    /// port. Unknown owners are looked up again after a second.
    fn owner(&self, port: u16, n: usize) -> Option<usize> {
        if let Some((node, at)) = self.owners.lock().unwrap().get(&port) {
            if node.is_some() || at.elapsed() < Duration::from_secs(1) {
                return *node;
            }
        }
        let pids = pids_on(port);
        let node = (0..n).find(|&k| {
            let listening = pids_on(self.listen[k]);
            pids.iter().any(|p| listening.contains(p))
        });
        self.owners
            .lock()
            .unwrap()
            .insert(port, (node, Instant::now()));
        node
    }
}

/// Bind 127.0.0.1:`port` next to the node's own socket on the same port
fn share_port(port: u16) -> Option<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).ok()?;
    socket.set_reuse_address(true).ok()?;
    #[cfg(unix)]
    socket.set_reuse_port(true).ok()?;
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    socket.bind(&addr.into()).ok()?;
    Some(socket.into())
}

/// Forward datagrams that arrive for node j to `node`, one upstream socket per sending
/// address so replies find their way back.
fn relay(front: UdpSocket, node: SocketAddr, j: usize, shared: Arc<Shared>) {
    let mut flows: HashMap<SocketAddr, UdpSocket> = HashMap::new();
    let mut buf = [0u8; 65536];
    while let Ok((len, from)) = front.recv_from(&mut buf) {
        if shared.cut(from.port(), j) {
            continue;
        }
        if !flows.contains_key(&from) {
            let Ok(up) = UdpSocket::bind(SocketAddr::new(node.ip(), 0)) else {
                continue;
            };
            let (up_rx, front_tx, shared) = (
                up.try_clone().unwrap(),
                front.try_clone().unwrap(),
                shared.clone(),
            );
            thread::spawn(move || {
                let mut buf = [0u8; 65536];
                while let Ok(len) = up_rx.recv(&mut buf) {
                    if !shared.cut(from.port(), j) {
                        let _ = front_tx.send_to(&buf[..len], from);
                    }
                }
            });
            flows.insert(from, up);
        }
        let _ = flows[&from].send_to(&buf[..len], node);
    }
}

/// Pids of the other processes with a UDP socket bound to local port `port`. `lsof -i` also
/// matches remote ports, so its socket names (`local->remote`) are checked.
fn pids_on(port: u16) -> Vec<String> {
    let own = std::process::id().to_string();
    let local = format!(":{port}");
    let Ok(out) = Command::new("lsof")
        .args(["-nP", "-Fpn", &format!("-iUDP:{port}")])
        .output()
    else {
        return Vec::new();
    };
    let mut pids = Vec::new();
    let mut pid = "";
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p;
        } else if let Some(name) = line.strip_prefix('n') {
            let bound_here = name.split("->").next().is_some_and(|l| l.ends_with(&local));
            if bound_here && pid != own && !pids.iter().any(|p| p == pid) {
                pids.push(pid.to_string());
            }
        }
    }
    pids
}
