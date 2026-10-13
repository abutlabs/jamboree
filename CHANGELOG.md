# Changelog

## 0.1.0

- Watch the logs of several JAM nodes side by side: pause all panes at once, scroll each
  node, zoom into one, highlight up to six search terms, grep mode and vi navigation.
- `--cmd` starts the nodes of any client through `sh -c`, with `{i}` replaced by the node
  index; `--nodes` sets how many (default 6). Jamixir runs its `node.sh` through `--cmd`
  (see the README).
- Output is saved to `logs/<timestamp>/node_<id>.log`; `--logs <dir>` replays any folder of
  `*.log` files, one pane per file.
- `--summary-regex` writes `summary.log`, the authored blocks per node, from a pattern for
  the client's block line.
- Install with `cargo install --git https://github.com/jamixir/jam-ui`.
