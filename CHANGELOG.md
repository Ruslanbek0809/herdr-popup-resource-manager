# Changelog

## 0.1.0 (first public release)

- Popup with live CPU and memory per herdr workspace, tab and pane. Each pane is measured as its full process tree.
- Memory trend per workspace, an Outside herdr row with the three busiest apps, and a herdr total with its share of
  system RAM.
- Worktree workspaces nested under their main workspace; agent state and task title on each pane.
- Keyboard: move, fold, `enter` to focus a workspace, tab or pane, `x` to close a pane (with a `y/N` prompt),
  `q`/`esc` to quit.
- Mouse: click to select, click again to focus, click the arrow to fold, scroll wheel to move.
- Single Rust binary for macOS and Linux. Install fetches a SHA-256-verified prebuilt binary and falls back to
  `cargo build`.
- A herdr notification when the binary is missing.
