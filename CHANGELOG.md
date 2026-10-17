# Changelog

## 0.1.0 — first public release

- Popup with CPU and memory per herdr workspace, tab and pane, summed over each pane's process tree.
- Memory-trend sparkline per workspace; "Outside herdr" row with the top 3 apps; total with share of system RAM.
- Linked worktree workspaces nested under their main workspace; agent status dots and task titles.
- Keys: move, fold, `enter` to focus a workspace/tab/pane, `x` to close a pane (with confirmation), `q`/`esc` to quit.
- macOS and Linux, one native binary (Rust): install downloads a prebuilt, checksum-verified binary or builds it
  with cargo.
- herdr notification when the binary is missing.
