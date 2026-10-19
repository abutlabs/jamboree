<p align="center"><img src="assets/logo.svg" width="96" alt="jamboree logo"></p>

<h1 align="center">jamboree</h1>

<p align="center">Launch a local network of any JAM client, watch every node's logs side by side, and break it on purpose.</p>

For milestone M2, JAM client teams run testnets of many nodes, often from different clients.
jamboree starts them all from one command, shows their live logs in one terminal grid and
saves every log. It can also stop, isolate, split or restart nodes, which is where many of
the hard bugs show up. Saved logs, from jamboree or anywhere else, open in the same grid.

<img src="assets/demo.gif" alt="jamboree with four Jamixir nodes: each pane lists the blocks its node imported; splitting the network gives the two halves different heads, and healing it brings all four back to one head">

```sh
cargo install --git https://github.com/jamixir/jamboree
jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}'
```

That starts six Jamixir nodes. For any other client, put the command that starts one node in
`--cmd`; jamboree runs it once per node with `{i}` replaced by 0, 1, 2, ... Linux and macOS,
no root. From a clone, `cargo run -- <args>` works the same.

No Rust? Download a binary for Linux or macOS (x86_64 or arm64) from
[Releases](https://github.com/jamixir/jamboree/releases/latest). If macOS blocks it, run
`xattr -d com.apple.quarantine jamboree`.

Built while developing [Jamixir](https://jamixir.org).

## Start a network

`--cmd` runs once per node with `sh -c`. `{i}` becomes the node index (0, 1, 2, ...), and
`--nodes` sets how many (default 6). Each node's output goes to its own pane.

For a mixed network, repeat `--cmd` to run one node per command, prefix `label=` to title
the panes, and `label[K]=` to run K nodes from one command (`[K]=` without a label):

```sh
jamboree --cmd 'theirs[4]=./their-jam-node <flags for node {i}>' \
         --cmd 'mine[2]=./my-jam-node <flags for node {i}>'
```

That is six nodes: `theirs 0` to `theirs 3`, then `mine 4` and `mine 5`.

The placeholders are `{i}`, `{port}` (the port the node listens on, also `$PORT`),
`{container}` (see Docker) and `{telemetry}` (see JIP-3 telemetry, also `$TELEMETRY`). Any other
`{word}` is an error. `--dry-run` prints each node's command and exits.

Every run is saved to `logs/<timestamp>/node_<i>.log`, with the command line in `cmd.txt`.

## Follow an event across nodes

- `Space` freezes all panes at once (the nodes keep running), so you can scroll each one.
- `/` highlights up to 6 terms in every pane, each in its own colour. Paste a block hash
  to follow it through the network. `n`/`N` jump between matches, `Ctrl+G` hides other lines.
- `Enter` zooms into the focused pane, `Esc` goes back, `1`-`6` hide or show the 1st to 6th pane.
- `--search 'error,warning'` sets the terms at start, also with `--logs`.

## Read saved logs

```sh
jamboree --logs logs/20261018_145800
```

`--logs` opens any folder of `*.log` files, one pane per file in natural order (`node_2` before
`node_10`): a past jamboree run, logs from CI, or files collected from other machines. Search,
`Ctrl+G`, zoom and scrolling work as in a live run; the network keys do not. `--nodes N` opens
only the first N files.

## JIP-3 telemetry

For clients that send [JIP-3](https://github.com/polkadot-fellows/JIPs/blob/main/JIP-3.md)
telemetry, put `--telemetry {telemetry}` in `--cmd`. jamboree gives each node its own address,
listens there, and writes these events in the node's pane and log:

| Event | Line |
| --- | --- |
| Best block changed | `[jip3] best block <slot> <hash>` |
| Finalized block changed | `[jip3] finalized <slot> <hash>` |
| Sync status changed | `[jip3] in sync: yes` or `no` |
| Authoring, block verification or block execution failed | `[jip3] block verification failed: <reason>` |
| Peer misbehaved | `[jip3] peer misbehaved: <first 4 bytes of its peer ID>: <reason>` |

These lines look the same for every client, so one search follows a hash or a failure across
a mixed network, and `--head jip3` (see below) checks the heads without a regex. A restarted
node connects again.

```sh
# six Jamixir nodes; Ctrl+G with the search below lists each node's best blocks
jamboree --nodes 6 --search 'best block' \
  --cmd 'cd ~/src/jamixir && ./node.sh {i} --telemetry {telemetry}'

# a mixed network: the same flag on every client that supports it
jamboree --cmd 'theirs[4]=./their-jam-node <flags for node {i}> --telemetry {telemetry}' \
         --cmd 'mine[2]=./my-jam-node <flags for node {i}> --telemetry {telemetry}'
```

`--dry-run` shows the address each node gets. For CI, see `--head jip3` below.

## Break the network

This is what a tmux layout or a shell script cannot do. Focus a node (arrows or click), then:

| Key | Fault |
| --- | --- |
| `s` | Stop the node (SIGSTOP). It stays connected but goes silent. `s` again continues it. |
| `i` | Isolate the node from all others. `i` again heals. |
| `p` | Split the network: the nodes before the focused one on one side, the rest on the other. `p` again heals. |
| `r` | Restart the node: kill it and run its command again. |

Isolate and split drop packets in both directions, like a broken link. Every fault writes a marker line
into every node's log, so each log shows when it happened.

The same faults can run on a schedule, at a time after the start:

```sh
jamboree --nodes 6 --cmd '...' \
  --fault '60s split 0,1,2/3,4,5' --fault '120s heal' --fault '150s restart 2'
```

Actions: `stop N`, `isolate N`, `split A/B`, `restart N`, and `heal` (continue, un-isolate
and unsplit everything). Times look like `90s` or `2m`; no time means at the start.

`--chaos SEED` adds a random schedule: every 20-60s a stop, isolate, split or restart, healed
20-60s later. The same seed and node count give the same schedule, and each run saves it as
`--fault` arguments in `faults.txt`, ready to replay or trim when a run finds a bug.

For a slow or lossy network, `--delay 100ms --jitter 20ms --loss 1` holds back every packet
between nodes by 80-120ms and drops 1% of them, in each direction, for the whole run.

## Run in CI

`--headless DURATION` runs the same network without the UI, then stops the nodes. With
`--head`, it checks that every node ended on the same head and exits 1 if not:

```sh
jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}' \
  --fault '60s split 0,1,2,3/4,5' --fault '120s heal' \
  --headless 3m --head 'State Updated successfully\. H: (0x[0-9a-f]{64})'
```

The head is what the regex's first group captures in each node's last matching log line.
After DURATION, jamboree waits up to `--timeout` (default 60s) for all nodes to agree, then
prints either the head or which nodes are on which. Stopped nodes are left out. For a mixed
network, give each client its own regex with a label: `--head 'polkajam=Imported (0x[0-9a-f]+)'`.
Clients shorten hashes differently, so there capture something they print alike, such as the slot.

With [JIP-3 telemetry](#jip-3-telemetry), `--head jip3` needs no regex and reads every client
alike. `--head jip3-final` checks the finalized block instead, which can no longer reorg:

```sh
jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i} --telemetry {telemetry}' \
  --headless 3m --head jip3
```

<details>
<summary>Head regexes for some clients</summary>

| Client | `--head` |
| --- | --- |
| Jamixir | `State Updated successfully\. H: (0x[0-9a-f]{64})` |
| PolkaJam | `Imported (0x[0-9a-f]+)` |
| JAM DUNA | `(?:Imported\|Authored) Block".* h=(\S+)` |
| PBnJam | `"blockHash":"(0x[0-9a-f]+)` |
</details>

## Recipes

Runs PolkaJam (release binary) and Jamixir networks. Typeberry and SpaceJam dev nodes start
under jamboree but stop when they author their first block (their published images, Oct 13).

<details>
<summary>Jamixir</summary>

```sh
jamboree --nodes 6 --cmd 'cd ~/src/jamixir && ./node.sh {i}'
```

Extra `node.sh` flags go into the command, e.g. `./node.sh {i} -x 4`.
</details>

<details>
<summary>PolkaJam</summary>

PolkaJam's genesis port cannot be shared, so give it another loopback address (Linux; on
macOS run `sudo ifconfig lo0 alias 127.0.0.2` once). Drop `--pvm-backend interpreter` if
your machine supports `userfaultfd`.

```sh
jamboree --nodes 6 --cmd 'polkajam=polkajam --chain dev run --pvm-backend interpreter --temp \
  --dev-validator {i} --listen-ip 127.0.0.2 --rpc-port $((19800+{i}))'
```
</details>

<details>
<summary>Mixed clients: four PolkaJam nodes plus yours</summary>

```sh
polkajam --chain dev dump-spec spec.json
jamboree --cmd 'polkajam[4]=polkajam --chain dev run --temp --dev-validator {i} --rpc-port 1980{i}' \
  --cmd 'mine=./my-jam-node --chain spec.json --validator {i} --bootnode <polkajam node 0 address>'
```

Panes are titled `polkajam 0` to `polkajam 3` and `mine 4`. Your client's flags will
differ. An all-caps `NAME=value` at the start, like `RUST_LOG=debug`, is a variable; a lowercase one is read as a label, so put `env` in front.
</details>

<details>
<summary>Docker</summary>

Put `{container}` where `docker run` takes the name. jamboree names one container per node,
pauses it for `s` and removes it on restart and quit. Use host networking, and keep `--init`
so containers stop with jamboree.

```sh
jamboree --nodes 6 --cmd 'docker run --rm --init --network host --user $(id -u):$(id -g) \
  --name {container} <image> <flags for node {i}>'
```

`--user` lets jamboree share the genesis port. For an image that must run as root, use
`--move-ports`. On SELinux systems mount volumes with `:z`. Docker faults need Linux.
</details>

## How it works

Peers dial node `i` on its genesis port, `40000+i` by default (`--genesis-port`). jamboree
binds `127.0.0.1` on the same port, so the OS hands it the peers' packets. It relays them to
the node, and drops them while the node is cut off. The node and its command stay unchanged.

<details>
<summary>When the port cannot be shared: <code>--move-ports</code></summary>

On Linux the client must set `SO_REUSEPORT`; on macOS it must listen on `::`. If it does
not, `i` says so. Then run with `--move-ports`: each node gets another port as `$PORT` and
`{port}`, which you pass to the client's listen-port flag (`--listen-port P` makes it `P+i`).
jamboree takes the genesis ports.

```sh
# peers still dial node i on 40000+i; jamboree relays to the port the node listens on
jamboree --nodes 6 --move-ports --cmd './my-jam-node --dev-validator {i} --port {port}'

# the same, with node i listening on 41000+i
jamboree --nodes 6 --move-ports --listen-port 41000 \
  --cmd './my-jam-node --dev-validator {i} --port {port}'
```
</details>

## Keys

| Key | Action |
| --- | --- |
| `←` `→`, `h` `l`, `Tab`, click | Focus previous / next node |
| `Enter`, `z`, click the focused pane / `Esc` | Zoom in / back to grid |
| `1`-`6` | Hide or show the 1st to 6th pane |
| `↑` `↓`, `j` `k`, PgUp/PgDn, Home/End, `Ctrl+D/U/F/B`, `gg` `G`, wheel | Scroll (freezes all panes first; the wheel scrolls the pane under the pointer) |
| `Space`, `F` | Freeze / follow the logs again |
| `/` | Search terms, comma-separated; edit the list to remove one |
| `n` / `N` | Next / previous match |
| `Ctrl+G` | Show only matching lines |
| `s` `i` `p` `r` | Stop, isolate, split, restart |
| `x` | Shut down all nodes (TERM, then KILL after 3 s) |
| `?` | Help |
| `q` | Quit (stops all nodes first) |

The mouse is on, so to select text hold Shift while dragging.

## Licence

[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
