<p align="center"><img src="assets/logo.svg" width="160" alt=""></p>

# jamboree

Terminal UI for watching the logs of several JAM nodes side by side. Built while
developing [Jamixir](https://jamixir.org), to follow one event (a block being authored
and imported, say) across a local testnet: freeze every pane at once, scroll each node,
and highlight the same block hash in all of them.

Normal mode starts each node with a shell command you pass with `--cmd`, so it works
with any JAM client. Log-viewer mode (`--logs`) works on any folder of `*.log` files.

<img width="2145" height="1179" alt="Image" src="https://github.com/user-attachments/assets/29b5ef57-5c6f-46c3-983e-5f387fe8bdec" />

## Install

```bash
cargo install --git https://github.com/jamixir/jamboree
```

This puts `jamboree` in `~/.cargo/bin`. From a clone, `cargo run -- <args>` works the same way.

## Usage

`--cmd` is run once per node with `sh -c`, from the current directory. Every `{i}` in it
is replaced by the node index (0, 1, 2, ...), and every `{port}` by the port the node
listens on (its genesis port unless `--move-ports`). `--nodes` sets how many nodes to start
(default 6). Each line a node prints to stdout/stderr shows up in its pane. Without
`--cmd` or `--logs`, jamboree prints its usage.

```bash
# Docker: one container per node
jamboree --nodes 4 --cmd 'docker run --rm --init --name jam-node-{i} <image> <flags for node {i}>'

# A local binary
jamboree --nodes 3 --cmd './my-jam-node <flags for node {i}>'
```

`<image>`, `./my-jam-node` and the flags are placeholders: use whatever starts one node
of your client (keys, ports and data directory per index). Keep `--init` on `docker run`:
without it the containers keep running after jamboree quits.

Every run's logs are saved per node, so a timeline of authored blocks across nodes is one
grep away, e.g. `grep -h "Authored block" logs/<run>/node_*.log | sort`.

### Jamixir

```bash
jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}'
```

Extra `node.sh` flags go straight into the command, e.g. `./node.sh {i} -x 4`.

### Mixed clients

Repeat `--cmd` to run one node per command, in order (`{i}` is still the node index, and
`--nodes` defaults to the number of commands). Prefix a command with `<label>=` to title its
panes with the label instead of "Node". A command that itself starts with `NAME=value`
needs a label or `env` in front, or `NAME` is taken as the label.

For example, four PolkaJam dev validators plus your own client as validator 4, all on
PolkaJam's dev chain spec:

```bash
polkajam --chain dev dump-spec spec.json
PJ='polkajam=polkajam --chain dev run --temp --dev-validator {i} --rpc-port 1980{i}'
jamboree --cmd "$PJ" --cmd "$PJ" --cmd "$PJ" --cmd "$PJ" \
       --cmd 'mine=./my-jam-node --chain spec.json --validator {i} --bootnode <polkajam node 0 address>'
```

The panes are titled `polkajam 0` to `polkajam 3` and `mine 4`. Chain spec, validator
keys and bootnode flags differ per client; the PolkaJam part runs as written.

Tried with PolkaJam, Typeberry, SpaceJam and Jamixir dev networks.

### Network faults

`i` cuts the focused node off from all the others; `p` splits the network in two at the
focused node (e.g. focus node 4 of 6 for 0-3 | 4-5). Press the key again to heal. Packets
are dropped silently in both directions, like a broken link, and every log gets a marker
line. It works on Linux and macOS without root and without changing the node command:

```sh
jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}'
```

Peers dial each node on its genesis port (`--genesis-port`, default 40000, so node i is on
40000+i). jamboree shares that port with the node: it binds the more specific 127.0.0.1 on the
same port, so the OS hands it the peers' packets, relays them to the node and drops them
while it is cut off. The node keeps listening where its genesis says.

Some clients' ports cannot be shared (on Linux the client must set `SO_REUSEPORT`; on macOS
it must listen on `::`). Then `i` says so, and `--move-ports` is the fallback: each node is
told to listen on another port, given as `$PORT` and `{port}`, which you pass to the client's
listen-port flag (`--listen-port P` fixes them to `P+i`), and jamboree takes the genesis ports.

PolkaJam always listens on its genesis port and cannot share it; give it another loopback
address instead (Linux; on macOS add it once with `sudo ifconfig lo0 alias 127.0.0.2`). If
it fails with `failed to create an userfaultfd`, add `--pvm-backend interpreter`:

```sh
jamboree --nodes 6 --cmd 'polkajam=polkajam --chain dev run --pvm-backend interpreter --temp \
  --dev-validator {i} --listen-ip 127.0.0.2 --rpc-port $((19800+{i}))'
```

### Scripted faults

`--fault '[<time>] <action>'` applies a fault as its key would, at a time after the start (`90s`,
`2m`; none means at the start), with the same markers. Actions: `stop N`, `isolate N`,
`split A/B`, `restart N` and `heal` (continue, un-isolate and unsplit everything):

```sh
jamboree --nodes 6 --cmd '…' --fault '60s split 0,1,2/3,4,5' --fault '120s heal' \
  --fault '150s restart 2'
```

### Docker

Put `{container}` where `docker run` takes the container name; jamboree fills in a name per node
and run, and then pauses that container for `s` (`docker pause`; SIGSTOP cannot reach it) and
removes it on restart and quit. Use host networking so nodes reach each other on loopback:

```sh
jamboree --nodes 6 --cmd 'docker run --rm --init --network host --user $(id -u):$(id -g) \
  --name {container} <image> <flags for node {i}>'
```

`--user` lets jamboree share the genesis port for `i`/`p` (Linux shares a port only between
sockets of one user); for an image that must run as root, use `--move-ports` and pass `$PORT`
to the node instead. On Fedora and other SELinux systems, mount volumes with `:z`
(`-v ./spec:/spec:z`). Docker faults need Linux.

### Search and log viewer

```bash
# With search terms (comma-separated, max 6)
jamboree --cmd '...' --search "error,warning"

# Log viewer mode: one pane per *.log file in the folder
jamboree --logs logs/20260111_161505

# Log viewer mode with search
jamboree --logs logs/20260111_161505 --search "error,warning"
```

**CLI Options:**
- `--cmd [<label>=]<command>`: Shell command that starts a node; `{i}` is the node index. Repeat it for one node per command
- `--nodes <n>`: Number of nodes to start (default 6, or one per `--cmd`); with `--logs`, number of files to load (default all)
- `--search` / `-s`: Comma-separated search terms (max 6) to highlight on startup
- `--logs` / `-l`: Load logs from folder instead of running nodes (log viewer mode)

## Menu Controls

### Normal Mode (Running Nodes)
- **`0-9`** - Toggle node visibility
- **`Tab`** - Cycle focus between nodes
- **`/`** - Search/highlight text in logs (comma-separated terms, max 6)
- **`Space`** - Freeze/unfreeze all panes (enables scrolling; the nodes keep running)
- **`S`** - Stop/continue the focused node (SIGSTOP/SIGCONT to its process group). The
  node stays connected but goes silent, then wakes up behind. Every log gets a marker line
- **`R`** - Restart the focused node: kill it and run its command again (put `rm -rf db{i} &&`
  in `--cmd` to restart from an empty database)
- **`I`** - Isolate/heal the focused node (see Network faults)
- **`P`** - Split the network at the focused node: the nodes before it on one side, it and the
  ones after on the other (see Network faults). `P` again heals
- **`X`** - Shutdown all nodes: TERM, then KILL after 3 s (auto-freezes, allows viewing logs)
- **`Enter`** - Zoom into focused node
- **`Esc`** - Return to grid view
- **`Q`** - Quit (stops all nodes first)

### Log Viewer Mode (Loading from Files)
- **`0-9`** - Toggle node visibility
- **`Tab`** - Cycle focus between nodes
- **`/`** - Search/highlight text in logs
- **`↑↓` / **`PgUp`** / **`PgDn`** - Scroll logs (always enabled)
- **`Enter`** - Zoom into focused node
- **`Esc`** - Return to grid view
- **`Q`** - Quit

**Scrolling (when frozen or in log viewer mode):**
- Arrow keys / mouse wheel or Page Up/Down to scroll
- Each node scrolls independently

## Features

### Search & Highlighting
- Press **`/`** to enter search mode
- Enter comma-separated search terms (max 6)
- Each term gets a unique color highlight
- Use **`←→`** to navigate between tags, **`Del/BS`** to remove tags
- Search terms from CLI are automatically applied on startup

### Auto Logging

All node output is automatically logged to timestamped directories:
- Location: `logs/YYYYMMDD_HHMMSS/node_<id>.log`
- Logs are written in real-time as nodes produce output
- Use **`--logs`** flag to view previously saved logs

## License

[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
