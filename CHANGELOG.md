# Changelog

## 0.1.0

- Watch the logs of several JAM nodes side by side: pause all panes at once, scroll each
  node, zoom into one, highlight up to six search terms, grep mode and vi navigation.
- `--cmd` starts nodes of any client through `sh -c`, with `{i}` replaced by the node index;
  `--nodes` sets how many (default 6).
- `--jamixir <path>` runs Jamixir's `node.sh`; without `--cmd` or `--jamixir`, jam-ui looks
  for a `jamixir/` checkout next to the current directory or the binary.
- Output is saved to `logs/<timestamp>/node_<id>.log`; `--logs <dir>` replays any folder of
  `*.log` files, one pane per file.
- `summary.log` lists authored blocks per node; `--summary-regex` sets the pattern for other
  clients.
- Install with `cargo install --git https://github.com/jamixir/jam-ui`.
