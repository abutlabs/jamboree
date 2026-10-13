# Jam UI

Terminal UI for watching the logs of several JAM nodes side by side. Built while
developing [Jamixir](https://jamixir.org), to follow one event (a block being authored
and imported, say) across a local testnet: freeze every pane at once, scroll each node,
and highlight the same block hash in all of them.

Normal mode starts the nodes through Jamixir's `node.sh`, or through any command you
pass with `--cmd`, so it works with other JAM clients too. Log-viewer mode (`--logs`)
works on any folder of `*.log` files.

<img width="2145" height="1179" alt="Image" src="https://github.com/user-attachments/assets/29b5ef57-5c6f-46c3-983e-5f387fe8bdec" />

## Install

```bash
cargo install --git https://github.com/jamixir/jam-ui
```

This puts `jam-ui` in `~/.cargo/bin`. From a clone, `cargo run --` works the same way.

## Usage

### Any JAM client (`--cmd`)

`--cmd` is run once per node with `sh -c`, from the current directory. Every `{i}` in it
is replaced by the node index (0, 1, 2, ...). `--nodes` sets how many nodes to start
(default 6).

```bash
# Docker: one container per node
jam-ui --nodes 4 --cmd 'docker run --rm --name jam-node-{i} <image> <flags for node {i}>'

# A local binary
jam-ui --nodes 3 --cmd './my-jam-node <flags for node {i}>'
```

`<image>`, `./my-jam-node` and the flags are placeholders: use whatever starts one node
of your client (keys, ports and data directory per index). Each line the node prints
to stdout/stderr shows up in its pane.

With `--cmd`, `summary.log` is only written when you pass `--summary-regex` with a
pattern for your client's "block authored" line, where capture group 1 is the block hash:

```bash
jam-ui --cmd '...' --summary-regex 'Authored block (0x[0-9a-f]+)'
```

### Jamixir

With no `--cmd`, jam-ui runs `jamixir/node.sh <id>` from a Jamixir checkout. It looks for
a `jamixir/` folder with a `node.sh` next to the current directory or next to the jam-ui
binary (so `jamixir/` and `jam-ui/` as sibling directories work with `cargo run`), or
takes the path from `--jamixir`. If neither finds a `node.sh`, it exits and points you to
`--cmd`.

```bash
# jamixir/ and jam-ui/ are siblings, run from jam-ui/
cargo run

# Any Jamixir checkout
jam-ui --jamixir ~/src/jamixir

# Skip announcements from node 0 to node 4 (passed to node.sh as -x)
cargo run -- -x 0:4
```

In this mode a stale `jamixir/.build.lock` from an interrupted build is removed on start,
and `summary.log` lists Jamixir's "Block created successfully" lines.

### Search and log viewer

```bash
# With search terms (comma-separated, max 6)
cargo run -- --search "error,warning"

# Log viewer mode: one pane per *.log file in the folder
cargo run -- --logs logs/20260111_161505

# Log viewer mode with search
cargo run -- --logs logs/20260111_161505 --search "error,warning"
```

**CLI Options:**
- `--cmd <command>`: Shell command that starts one node; `{i}` is the node index
- `--nodes <n>`: Number of nodes to start (default 6); with `--logs`, number of files to load (default all)
- `--jamixir <path>`: Jamixir checkout to run `node.sh` from
- `--summary-regex <regex>`: Block-authored line for `summary.log`, capture group 1 = block hash
- `-x` / `--skip-announcements-to`: Jamixir only, passed through to `node.sh`
- `--search` / `-s`: Comma-separated search terms (max 6) to highlight on startup
- `--logs` / `-l`: Load logs from folder instead of running nodes (log viewer mode)

## Menu Controls

### Normal Mode (Running Nodes)
- **`0-9`** - Toggle node visibility
- **`Tab`** - Cycle focus between nodes
- **`/`** - Search/highlight text in logs (comma-separated terms, max 6)
- **`Space`** - Pause/unpause all nodes (enables scrolling)
- **`X`** - Shutdown all nodes (auto-pauses, allows viewing logs)
- **`Enter`** - Zoom into focused node
- **`Esc`** - Return to grid view
- **`Q`** - Quit

### Log Viewer Mode (Loading from Files)
- **`0-9`** - Toggle node visibility
- **`Tab`** - Cycle focus between nodes
- **`/`** - Search/highlight text in logs
- **`↑↓` / **`PgUp`** / **`PgDn`** - Scroll logs (always enabled)
- **`Enter`** - Zoom into focused node
- **`Esc`** - Return to grid view
- **`Q`** - Quit

**Scrolling (when paused or in log viewer mode):**
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
- `summary.log` in the same folder lists the authored blocks per node (see `--summary-regex`)

## License

[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
