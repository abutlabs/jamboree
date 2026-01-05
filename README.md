# Jam UI

Terminal UI for monitoring multiple node processes.

## Menu Controls

- **`0-5`** - Toggle node visibility
- **`Tab`** - Cycle focus between nodes
- **`/`** - Search/highlight text in logs
- **`Space`** - Pause/unpause all nodes (enables scrolling)
- **`Enter`** - Zoom into focused node
- **`Esc`** - Return to grid view
- **`Q`** - Quit

When paused, use arrow keys / mouse or Page Up/Down to scroll.

## Auto Logging

All node output is automatically logged to timestamped directories:
- Location: `logs/YYYYMMDD_HHMMSS/node_<id>.log`
- Logs are written in real-time as nodes produce output

