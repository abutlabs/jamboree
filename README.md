<p align="center"><img src="assets/logo.svg" width="96" alt="jamboree logo"></p>

<h1 align="center">jamboree</h1>

<p align="center">Launch a local network of any JAM client, watch every node's logs side by side, and break it on purpose.</p>

For milestone M2, JAM client teams run testnets of many nodes, often from different clients.
jamboree starts them all from one command, shows their live logs in one terminal grid and
saves every log. It can also stop, isolate, split or restart nodes, which is where many of
the hard bugs show up.

<!-- TODO: demo GIF here (start 6 nodes, freeze, search a block hash, press p to split, heal). Until then: -->
<img width="2145" height="1179" alt="Six JAM nodes' logs side by side in jamboree" src="https://github.com/user-attachments/assets/29b5ef57-5c6f-46c3-983e-5f387fe8bdec" />

```sh
cargo install --git https://github.com/jamixir/jamboree
jamboree --nodes 6 --cmd './my-jam-node <flags for node {i}>'
```

Linux and macOS, no root. From a clone, `cargo run -- <args>` works the same.

Built while developing [Jamixir](https://jamixir.org).

## Start a network

`--cmd` runs once per node with `sh -c`. `{i}` becomes the node index (0, 1, 2, ...), and
`--nodes` sets how many (default 6). Each node's output goes to its own pane.

For a mixed network, repeat `--cmd` to run one node per command, and prefix `label=` to title
the panes:

```sh
jamboree --cmd 'theirs=./their-jam-node <flags for node {i}>' \
         --cmd 'mine=./my-jam-node <flags for node {i}>'
```

Every run is saved to `logs/<timestamp>/node_<i>.log`, with the command line in `cmd.txt`.
`jamboree --logs <dir>` replays any folder of `*.log` files, one pane per file.

## Follow an event across nodes

- `Space` freezes all panes at once (the nodes keep running), so you can scroll each one.
- `/` highlights up to 6 terms in every pane, each in its own colour. Paste a block hash
  to follow it through the network. `n`/`N` jump between matches, `Ctrl+G` hides other lines.
- `Enter` zooms into the focused pane, `Esc` goes back, `0`-`9` hide or show a pane.
- `--search 'error,warning'` sets the terms at start, also with `--logs`.

## Break the network

This is what a tmux layout or a shell script cannot do. Focus a node with `Tab`, then:

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

## Recipes

Tried with PolkaJam, Typeberry, SpaceJam and Jamixir dev networks.

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
PJ='polkajam=polkajam --chain dev run --temp --dev-validator {i} --rpc-port 1980{i}'
jamboree --cmd "$PJ" --cmd "$PJ" --cmd "$PJ" --cmd "$PJ" \
  --cmd 'mine=./my-jam-node --chain spec.json --validator {i} --bootnode <polkajam node 0 address>'
```

Panes are titled `polkajam 0` to `polkajam 3` and `mine 4`. Your client's flags will
differ. A command that starts with `NAME=value` needs a label or `env` in front.
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
</details>

## Keys

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | Focus next / previous node |
| `Enter` / `Esc` | Zoom in / back to grid |
| `0`-`9` | Hide or show a pane |
| `Space` | Freeze or unfreeze all panes |
| `j` `k` `gg` `G` `Ctrl+D/U/F/B`, arrows, PgUp/PgDn | Scroll |
| `/` | Search terms (`←` `→` pick a term, `Del` removes it) |
| `n` / `N` | Next / previous match |
| `Ctrl+G` | Show only matching lines |
| `s` `i` `p` `r` | Stop, isolate, split, restart |
| `x` | Shut down all nodes (TERM, then KILL after 3 s) |
| `?` | Help |
| `q` | Quit (stops all nodes first) |

## Licence

[Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
