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
