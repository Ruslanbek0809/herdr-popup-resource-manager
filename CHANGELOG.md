# Changelog

## 0.1.0 — first public release

- A popup that shows CPU and memory for each herdr workspace, tab and pane. A pane counts everything started from it.
- A memory trend chart for each workspace, an "Outside herdr" row with the 3 busiest apps, and a total with its share
  of your RAM.
- Worktree workspaces are listed under their main workspace. Each agent shows its state and current task.
- Keys: move, fold, `enter` to go to a workspace, tab or pane, `x` to close a pane (asks first), `q` or `esc` to quit.
- Mouse: click to select, click again to go there, click the arrow to fold, scroll to move.
- Runs on macOS and Linux as one small program. The install downloads a checked, ready-made copy, or builds one with
  cargo.
- If the program is missing, herdr shows a notification.
