//! Network faults: a UDP relay on each node's genesis port that drops traffic between nodes
//! that are cut off from each other (an isolated node, or the two sides of a split).
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
    /// Docker container name of each node that runs in one (`{container}`)
    containers: Vec<Option<String>>,
    /// Whether node j's relay is up
    relayed: Mutex<Vec<bool>>,
    /// Nodes cut off from all others
    isolated: Mutex<Vec<bool>>,
    /// Nodes from this index on are split off from the ones before
    split_at: Mutex<Option<usize>>,
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
        containers: Vec<Option<String>>,
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
            containers,
            relayed: Mutex::new(vec![false; n]),
            isolated: Mutex::new(vec![false; n]),
            split_at: Mutex::new(None),
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
        self.shared.isolated.lock().unwrap()[i]
    }

    /// Side of the split node i is on, if the network is split
    pub fn split_side(&self, i: usize) -> Option<char> {
        let at = (*self.shared.split_at.lock().unwrap())?;
        Some(if i < at { 'A' } else { 'B' })
    }

    /// Cut node i off from every other node, or heal it. Returns whether it is now isolated.
    pub fn toggle_isolated(&self, i: usize) -> Result<bool, String> {
        self.shared.all_relayed()?;
        let mut isolated = self.shared.isolated.lock().unwrap();
        isolated[i] = !isolated[i];
        Ok(isolated[i])
    }

    /// Split nodes `at..` off from nodes `..at`, or heal the split. Returns whether it is now
    /// split.
    pub fn toggle_split(&self, at: usize) -> Result<bool, String> {
        self.shared.all_relayed()?;
        let mut split_at = self.shared.split_at.lock().unwrap();
        *split_at = match *split_at {
            Some(_) => None,
            None => Some(at),
        };
        Ok(split_at.is_some())
    }
}

impl Shared {
    /// Every node must be behind the relay, or traffic leaks around a cut
    fn all_relayed(&self) -> Result<(), String> {
        let direct: Vec<String> = (self.relayed.lock().unwrap().iter())
            .enumerate()
            .filter(|(_, relayed)| !**relayed)
            .map(|(k, _)| k.to_string())
            .collect();
        if direct.is_empty() {
            return Ok(());
        }
        let (noun, verb) = if direct.len() == 1 {
            ("node", "does")
        } else {
            ("nodes", "do")
        };
        // A port is shared only between sockets of one user, so a root container cannot share
        let user = match self.containers.iter().any(Option::is_some) {
            true => " (containers: run them with --user $(id -u):$(id -g))",
            false => "",
        };
        let why = match self.moved {
            true => format!("{verb} not listen on $PORT"),
            false => {
                format!("{verb} not share the genesis port{user}; or restart with --move-ports")
            }
        };
        Err(format!(
            "cannot cut nodes off: {noun} {} {why}",
            direct.join(", ")
        ))
    }

    /// For the first two minutes, start node j's relay once the node listens
    fn relay_listening_nodes(self: Arc<Self>) {
        let until = Instant::now() + Duration::from_secs(120);
        while Instant::now() < until && self.relayed.lock().unwrap().contains(&false) {
            for j in 0..self.listen.len() {
                if self.relayed.lock().unwrap()[j] || !self.listening(j) {
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

    /// Whether node j listens on its port. A container's root-owned socket is invisible to
    /// `lsof`, so for one its cgroup is checked instead.
    fn listening(&self, j: usize) -> bool {
        let port = self.listen[j];
        match self.containers[j] {
            Some(_) => container_on(port).is_some(),
            None => !pids_on(port).is_empty(),
        }
    }

    /// Whether traffic between the sender on local port `port` and node j is cut
    fn cut(&self, port: u16, j: usize) -> bool {
        let isolated = self.isolated.lock().unwrap().clone();
        let split_at = *self.split_at.lock().unwrap();
        if split_at.is_none() && !isolated.contains(&true) {
            return false;
        }
        // Nodes in different groups are cut off: an isolated node is a group of its own
        let group = |k: usize| match isolated[k] {
            true => usize::MAX - k,
            false => split_at.map_or(0, |at| usize::from(k >= at)),
        };
        self.owner(port, isolated.len())
            .is_some_and(|k| group(k) != group(j))
    }

    /// The node whose process owns local UDP port `port`: the one that also owns a listen
    /// port. Unknown owners are looked up again after a second.
    fn owner(&self, port: u16, n: usize) -> Option<usize> {
        if let Some((node, at)) = self.owners.lock().unwrap().get(&port) {
            if node.is_some() || at.elapsed() < Duration::from_secs(1) {
                return *node;
            }
        }
        // A node sending from its own listen port; else match the port's container or process
        let node = self.listen.iter().position(|&p| p == port).or_else(|| {
            let container = container_on(port);
            let pids = pids_on(port);
            (0..n).find(|&k| match (&self.containers[k], &container) {
                (Some(name), Some(id)) => container_id(name).is_some_and(|c| c.starts_with(id)),
                _ => {
                    let listening = pids_on(self.listen[k]);
                    pids.iter().any(|p| listening.contains(p))
                }
            })
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

/// Id (prefix) of the Docker container with a UDP socket on local port `port` (Linux: `ss`
/// shows each socket's cgroup, `docker-<id>.scope`, without root)
fn container_on(port: u16) -> Option<String> {
    let out = Command::new("ss")
        .args(["-uanH", "--cgroup", &format!("sport = :{port}")])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let id = text.split("docker-").nth(1)?.split(".scope").next()?;
    Some(id.to_string())
}

/// Full id of the running container `name`
fn container_id(name: &str) -> Option<String> {
    let out = Command::new("docker")
        .args(["inspect", "-f", "{{.Id}}", name])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}
