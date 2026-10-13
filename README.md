# Jam UI

Terminal UI for watching the logs of several JAM nodes side by side. Built while
developing [Jamixir](https://jamixir.org), to follow one event (a block being authored
and imported, say) across a local testnet: freeze every pane at once, scroll each node,
and highlight the same block hash in all of them.

Normal mode starts each node with a shell command you pass with `--cmd`, so it works
with any JAM client. Log-viewer mode (`--logs`) works on any folder of `*.log` files.

<img width="2145" height="1179" alt="Image" src="https://github.com/user-attachments/assets/29b5ef57-5c6f-46c3-983e-5f387fe8bdec" />

## Install

```bash
cargo install --git https://github.com/jamixir/jam-ui
```

This puts `jam-ui` in `~/.cargo/bin`. From a clone, `cargo run -- <args>` works the same way.

## Usage

`--cmd` is run once per node with `sh -c`, from the current directory. Every `{i}` in it
is replaced by the node index (0, 1, 2, ...). `--nodes` sets how many nodes to start
(default 6). Each line a node prints to stdout/stderr shows up in its pane. Without
`--cmd` or `--logs`, jam-ui prints its usage.

```bash
# Docker: one container per node
jam-ui --nodes 4 --cmd 'docker run --rm --init --name jam-node-{i} <image> <flags for node {i}>'

# A local binary
jam-ui --nodes 3 --cmd './my-jam-node <flags for node {i}>'
```

`<image>`, `./my-jam-node` and the flags are placeholders: use whatever starts one node
of your client (keys, ports and data directory per index). Keep `--init` on `docker run`:
without it the containers keep running after jam-ui quits.

`summary.log` is written when you pass `--summary-regex` with a pattern for your client's
"block authored" line, where capture group 1 is the block hash:

```bash
jam-ui --cmd '...' --summary-regex 'Authored block (0x[0-9a-f]+)'
```

### Jamixir

```bash
jam-ui --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}' \
  --summary-regex '\[BLOCK\] ⛓️ Block created successfully\. Header Hash (.+)'
```

Extra `node.sh` flags go straight into the command, e.g. `./node.sh {i} -x 4`.

### Tested with

| Client | Command | Status |
| ------ | ------- | ------ |
| [PolkaJam](https://github.com/paritytech/polkajam-releases) nightly 2026-10-13, release binary | `jam-ui --nodes 6 --cmd 'polkajam --chain dev run --temp --dev-validator {i} --rpc-port 1980{i} --pvm-backend interpreter' --summary-regex 'Authored block (0x[0-9a-f]+)'` | Works: a 6-validator dev chain that authors, imports and finalizes blocks. `--pvm-backend interpreter` is only needed where `vm.unprivileged_userfaultfd` is off. The hashes in its log are shortened, so the summary shows the first 8 bytes. |
| [Typeberry](https://github.com/FluffyLabs/typeberry) 0.11.0, `ghcr.io/fluffylabs/typeberry:latest` | `jam-ui --nodes 2 --cmd 'docker run --rm --init --name jam-ui-typeberry-{i} ghcr.io/fluffylabs/typeberry:latest dev {i}'` | Partial: the dev node starts and logs, then stops when block authoring begins (`Cannot find package 'tsx'` in this image). |
| [SpaceJam](https://github.com/spacejamapp/spacejam) 0.1.0 release binary and `ghcr.io/spacejamapp/spacejam:0.1.2` | `spacejam run --dev -d ./data/0` | No: the dev node exits with `failed to decode recent blocks` when it authors its first block. |
| [Boka](https://github.com/open-web3-stack/boka), `acala/boka:latest` | `docker run --rm acala/boka:latest --dev --dev-seed 0 --rpc no` | No: the node logs its genesis and shuts down right away. |

### Search and log viewer

```bash
# With search terms (comma-separated, max 6)
jam-ui --cmd '...' --search "error,warning"

# Log viewer mode: one pane per *.log file in the folder
jam-ui --logs logs/20260111_161505

# Log viewer mode with search
jam-ui --logs logs/20260111_161505 --search "error,warning"
```

**CLI Options:**
- `--cmd <command>`: Shell command that starts one node; `{i}` is the node index
- `--nodes <n>`: Number of nodes to start (default 6); with `--logs`, number of files to load (default all)
- `--summary-regex <regex>`: Block-authored line for `summary.log`, capture group 1 = block hash
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
