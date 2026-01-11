# Jam UI

Terminal UI for monitoring multiple node processes.

<img width="2145" height="1179" alt="Image" src="https://github.com/user-attachments/assets/29b5ef57-5c6f-46c3-983e-5f387fe8bdec" />

## Usage

**Expected folder structure:**
- `jamixir/` and `jam-ui/` should be sibling directories
- The app expects `node.sh` to exist in the `jamixir/` folder

**Running:**
```bash
# Normal mode: spawn and monitor nodes
cargo run

# With search terms (comma-separated, max 6)
cargo run -- --search "error,warning"

# Log viewer mode: load logs from folder
cargo run -- --logs logs/20260111_161505

# Log viewer mode with search
cargo run -- --logs logs/20260111_161505 --search "error,warning"
```

**CLI Options:**
- `--search` / `-s`: Comma-separated search terms (max 6) to highlight on startup
- `--logs` / `-l`: Load logs from folder instead of running nodes (log viewer mode)

## Menu Controls

### Normal Mode (Running Nodes)
- **`0-5`** - Toggle node visibility
- **`Tab`** - Cycle focus between nodes
- **`/`** - Search/highlight text in logs (comma-separated terms, max 6)
- **`Space`** - Pause/unpause all nodes (enables scrolling)
- **`X`** - Shutdown all nodes (auto-pauses, allows viewing logs)
- **`Enter`** - Zoom into focused node
- **`Esc`** - Return to grid view
- **`Q`** - Quit

### Log Viewer Mode (Loading from Files)
- **`0-5`** - Toggle node visibility
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

