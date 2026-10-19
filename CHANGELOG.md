# Changelog

## Unreleased

- The help fits short terminals: no banner, two columns when one is too tall, and it scrolls
  only when neither fits. Shift+drag text selection is listed under NODES.
- `m` turns the mouse off and on, so a plain drag selects text; the zoomed pane has no side
  borders, so a selection there is just the log text.

## 0.6.0

- `client_colors = false` in the config file shows the panes as plain text.
- A node given `{telemetry}` that has not connected after 60 s gets a `no JIP-3 telemetry` line
  in its pane and log, which catches a misspelt telemetry flag.
- Unit tests for the key handling.

## 0.5.0

- Scrolling a pane freezes only that pane, tagged `[FROZEN: Space resumes]`; the others keep
  following. `Space` (or `End`, `G`, `F`) resumes it; with nothing frozen, `Space` freezes all.
- `c` also empties the search box, and the filter turns off when no terms are left.
- Focus skips hidden panes; hiding the focused pane moves focus on.
- `x` no longer freezes the panes, so a node restarted after it shows its output.
- The help scrolls with the arrows on a short terminal; the menu starts with `[?] Help  [q] Quit`
  and has shorter state hints, so they fit in 80 columns.
- Setup errors exit 2, so CI can tell them from different heads (1). `--dry-run` runs every
  setup check first.
- `--chaos` warns when the run is too short for any fault; `--logs` names a folder it can't read.

## 0.4.1

- `Ctrl+G` with no search terms opens the search box, so typed terms no longer reach the node keys.
- Config file `~/.config/jamboree/config.toml`; `--print-config` prints the defaults. First option:
  `esc_clears_search` (off: `Esc` only leaves zoom).
- Search and filter are named apart in the menu: `[/] Search`, `[^G] Filter` (`Show all` while on,
  with `[FILTER: matching lines only]`). `c` clears both; in the search box `Ctrl+W` drops the
  last term and `Ctrl+U` all of them.

## 0.4.0

- `--cmd 'label[K]=...'` (or `[K]=` without a label) runs K nodes from one command; it replaces
  `label*K=`.
- JIP-3: finalized block, sync status, failed authoring, verification and execution, and
  misbehaving peers also show in the panes; `--head jip3-final` checks the finalized block.

## 0.3.0

- JIP-3 telemetry: `{telemetry}` in `--cmd` gives each node a telemetry address, its best block
  changes show in the pane as `[jip3] best block <slot> <hash>`, and `--head jip3` checks them.
- `--cmd 'label*K=...'` runs K nodes from one command.
- An unknown `{word}` in `--cmd`, such as `{prot}`, is an error before any node starts.
- `--dry-run` prints each node's command and exits.
- `$PORT` is always set (it was only with `--move-ports`), and `$TELEMETRY` with `{telemetry}`.
- Panes show the client's own log colours; saved logs, search and `--head` still use plain text.
- Fix: with `Ctrl+G`, a full pane hid its newest matching line.

## 0.2.0

- `--headless DURATION` runs without the UI and exits when done; `--head '[label=]REGEX'`
  checks that all nodes ended on the same head (exit 1 if not), waiting up to `--timeout`.
- Arrows and the mouse: `←` `→` (and `h` `l`) move focus, `↑` `↓` PgUp/PgDn Home/End scroll
  and freeze like the vi keys, click focuses, a second click or `z` zooms, the wheel scrolls
  the pane under the pointer, `F` follows again. Search terms are removed by editing `/`.
- `--chaos SEED` adds a random, replayable fault schedule; every run with faults saves them
  as `--fault` arguments in `faults.txt`.
- `--delay`, `--jitter` and `--loss` slow down and drop packets between nodes.
- `1`-`6` hide or show the 1st to 6th pane (was `0`-`9`).
- `--help` shows examples, a key summary and grouped options; `--version` added.
- Fix: a command starting with an environment variable such as `RUST_LOG=debug` is no longer
  read as a label.

## 0.1.0

- Watch the logs of several JAM nodes side by side: pause all panes at once, scroll each
  node, zoom into one, highlight up to six search terms, grep mode and vi navigation.
- `--cmd` starts the nodes of any client through `sh -c`, with `{i}` replaced by the node
  index; `--nodes` sets how many (default 6). Jamixir runs its `node.sh` through `--cmd`
  (see the README).
- Repeat `--cmd` to run different clients side by side, one node per command; a
  `<label>=` prefix titles the panes.
- Output is saved to `logs/<timestamp>/node_<id>.log`; `--logs <dir>` replays any folder of
  `*.log` files, one pane per file.
- Install with `cargo install --git https://github.com/jamixir/jamboree`.
- `x` and `q` stop each node's whole process group (TERM, then KILL after 3 s), so no
  client outlives jamboree.
- `--fault '[<time>] <action>'` schedules stop, isolate, split, restart and heal, e.g.
  `--fault '60s split 0,1,2/3,4,5'`.
- `s` stops/continues the focused node (SIGSTOP/SIGCONT) and writes a marker line into
  every node's log. `Space` is now called freeze, since the nodes keep running.
- `i` cuts the focused node off from the others and heals it, on Linux and macOS without
  root and without changing the node command: jamboree shares each node's genesis port and
  relays its traffic. `--move-ports` is the fallback for clients whose port cannot be shared.
- `p` splits the network at the focused node and heals it; `r` restarts the focused node.
- Each run saves its command line to `logs/<run>/cmd.txt`.
- `{container}` in `--cmd` names a node's Docker container: `s` pauses it, restart and quit remove
  it, and `i`/`p` find its traffic without root.