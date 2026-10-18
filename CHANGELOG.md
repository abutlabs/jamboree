# Changelog

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
- Install with `cargo install --git https://github.com/jamixir/jam-ui`.
- `x` and `q` stop each node's whole process group (TERM, then KILL after 3 s), so no
  client outlives jam-ui.
- `s` stops/continues the focused node (SIGSTOP/SIGCONT) and writes a marker line into
  every node's log. `Space` is now called freeze, since the nodes keep running.
- `i` cuts the focused node off from the others and heals it, on Linux and macOS without
  root and without changing the node command: jam-ui shares each node's genesis port and
  relays its traffic. `--move-ports` is the fallback for clients whose port cannot be shared.
- `p` splits the network at the focused node and heals it; `r` restarts the focused node.
- Each run saves its command line to `logs/<run>/cmd.txt`.
- `{container}` in `--cmd` names a node's Docker container: `s` pauses it, restart and quit remove
  it, and `i`/`p` find its traffic without root.