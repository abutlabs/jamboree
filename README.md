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

**PolkaJam** ([release binary](https://github.com/paritytech/polkajam-releases), nightly 2026-10-13):
works. A 6-validator dev chain that authors, imports and finalizes blocks.

```bash
jam-ui --nodes 6 --cmd 'polkajam --chain dev run --temp --dev-validator {i} --rpc-port 1980{i} --pvm-backend interpreter' \
  --summary-regex 'Authored block (0x[0-9a-f]+)'
```

`--pvm-backend interpreter` is only needed where `vm.unprivileged_userfaultfd` is off.
PolkaJam shortens hashes in its log, so the summary shows the first 8 bytes.

**Typeberry** ([source](https://github.com/FluffyLabs/typeberry), v0.11.0): works from a source
checkout. Six dev nodes connect, author and import blocks, the same setup as its
`npm run tiny-network`.

```bash
git clone --branch v0.11.0 https://github.com/FluffyLabs/typeberry && cd typeberry && npm ci
jam-ui --nodes 6 --cmd 'npm start -- dev {i}'
```

The published `ghcr.io/fluffylabs/typeberry` image and the `@typeberry/jam` npm package
(0.10.0, 0.11.0) start, but stop as soon as they author (`Cannot find package 'tsx'`).

**SpaceJam** ([release binary](https://github.com/spacejamapp/spacejam), 0.1.0): authors, as
independent single-node dev chains. It needs a Gray Paper 0.7.2 chain spec; its built-in
dev spec fails with `failed to decode recent blocks`. PolkaJam v0.1.28 (GP 0.7.2) writes one:

```bash
polkajam --chain dev dump-spec spec.json
jam-ui --nodes 3 --cmd 'spacejam run -v --dev --chain spec.json -d data/{i} --prune --rpc 127.0.0.1:1990{i}'
```

It can also join a PolkaJam v0.1.28 dev network as validator 5. There it imports and finalizes
the PolkaJam blocks, but it did not author its own slot in our run:

```bash
jam-ui --nodes 6 --cmd 'if [ {i} = 5 ]; then spacejam run -v --chain spec.json --validator 5 -d sjdata --prune --address 127.0.0.1:40005 --rpc 127.0.0.1:19805 --bootnode eecgwpgwq3noky4ijm4jmvjtmuzv44qvigciusxakq5epnrfj2utb@127.0.0.1:40000; else polkajam --chain dev run --temp --dev-validator {i} --rpc-port 1980{i} --pvm-backend interpreter; fi'
```

**JamZig** ([conformance binaries](https://github.com/jamzig/conformance-releases), GP 0.7.2):
imports only. The release is a conformance target plus a fuzzer, with no network or authoring
mode. jam-ui can show both side by side:

```bash
jam-ui --nodes 2 --cmd 'if [ {i} = 0 ]; then jam_conformance_target -v --socket /tmp/jamzig.sock; else sleep 1; jam_conformance_fuzzer --socket /tmp/jamzig.sock --blocks 50; fi'
```

**Boka** (`acala/boka:latest`): doesn't run.
`docker run --rm acala/boka:latest --dev --dev-seed 0 --rpc no` logs its genesis and shuts down
right away.

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
