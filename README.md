# Popup Resource Manager for herdr

Which agent is making your fan spin? A [herdr](https://herdr.dev) popup with live CPU and memory for every
workspace, tab and pane — plus everything outside herdr — one keybinding away. Zero dependencies.

<!-- Screenshot: replace the block below with ![Popup Resource Manager](assets/screenshot.png) -->
```
┌─ popup-resource-manager ──────────────────────── CPU 100% = 1 core of 11 ─┐
│ Name                                   CPU      Memory  Memory trend       │
│ ────────────────────────────────────────────────────────────────────────── │
│ ▾ [2] notes                          123.2%    523.1 MB  ▁▁▂▂▄▆█           │
│   ▾ [2] Phase 3 work                 123.2%    395.7 MB                    │
│     ● claude  phase-3-plan           123.2%    395.7 MB                    │
│ ▾ Outside herdr  other apps…         144.4%     9.51 GB  ▁▄▄▆▆██           │
│   · Google Chrome                     95.0%     6.32 GB                    │
│ ────────────────────────────────────────────────────────────────────────── │
│ Total in herdr                       123.2%    696.2 MB  ~4% of system RAM │
│ ↑↓ move  ·  space fold  ·  enter focus  ·  x close pane  ·  q quit         │
└────────────────────────────────────────────────────────────────────────────┘
```

## Features

- CPU and memory per **workspace → tab → pane**, summed over each pane's whole process tree (agents, their language
  servers, test runs…).
- Agent status dots (working, blocked, idle) and the agent's task title next to each pane.
- **Memory trend** sparkline per workspace while the popup is open.
- **Outside herdr** row: everything else on the Mac, with the top 3 apps — browsers, daemons and simulators often are
  the real culprit.
- Linked git-worktree workspaces are listed under their main workspace.
- Jump to a pane (`enter`) or close it (`x`, with confirmation) straight from the list.
- One Python file using only the standard library: no build step, nothing to install besides herdr.

## Requirements

- macOS (Linux is planned)
- herdr 0.9.3 or newer
- `python3` 3.9 or newer — the one from the Xcode Command Line Tools is enough. On a Mac without them,
  `/usr/bin/python3` opens Apple's install dialog; the plugin shows a herdr notification if Python is missing.

## Install

```sh
herdr plugin install Ruslanbek0809/herdr-popup-resource-manager
```

herdr plugins can't bring their own shortcut, so bind one in `~/.config/herdr/config.toml` (any free key; `prefix+m`
is just a suggestion) and run `herdr server reload-config`:

```toml
[[keys.command]]
key = "prefix+m"
type = "plugin_action"
command = "herdr-popup-resource-manager.open"
description = "resource manager"
```

Uninstall: `herdr plugin uninstall herdr-popup-resource-manager`, then remove the binding.

## Keys

| Key | Does |
| --- | --- |
| `↑` `↓` / `j` `k`, `PgUp` `PgDn` | move |
| `space` | fold / unfold a workspace, tab or the Outside row |
| `enter` | focus that workspace / tab / pane and close the popup |
| `x` | close the selected pane (asks `y/N`) |
| `q` / `esc` | quit |

## What the numbers mean

- **Pane** = its shell and every process below it. Tab and workspace rows are sums. Processes that daemonize or are
  started by launchd (Gradle daemon, simulator, `nohup` jobs) leave the pane's tree and show up under **Outside herdr**.
- **CPU** is per core, from the CPU time each process used since the last sample (every 1 s): 100% = one core busy,
  so it can exceed 100%. Processes that live less than a second can be missed.
- **Memory** is resident memory (RSS) summed over the processes. Activity Monitor shows the "memory footprint"
  instead, so its numbers differ; shared pages are counted once per process, so sums overcount. "% of system RAM" is
  therefore approximate.
- **Outside herdr** = every other process on the machine, with the top 3 apps by CPU (helpers grouped under their
  `.app`). The popup's own process is left out of every row.
- **Memory trend** keeps history only while the popup is open; a change under 5% of the row's peak draws flat.
- Sampling costs about 2% of one core while the popup is open, nothing when it's closed.

## What it runs

herdr doesn't review or sandbox plugins, so here is everything this one does:

- `scripts/open.sh` (the action) checks for `python3` (if it's missing: `herdr notification show`) and asks herdr
  to open the popup.
- `herdr-popup-resource-manager` (the popup) reads total RAM once with `sysctl`, the process table once per second
  with `ps -A` (plus `ps -p` to name processes it hasn't seen before), and talks to herdr over its local socket (`$HERDR_SOCKET_PATH`): `session.snapshot`, `pane.process_info`,
  and — only when you press the key — `workspace.focus` / `tab.focus` / `pane.focus` or `pane.close` (after `y`).
- No network access, no files written, no build step, no background process when the popup is closed.
- Started outside herdr (no socket), it shows the error and `q` quits.

## Develop

```sh
python3 -m unittest discover -s tests -v       # also with /usr/bin/python3 (3.9)
./herdr-popup-resource-manager --once          # one sample as JSON, run inside herdr
herdr plugin link "$PWD"                       # use your working copy instead of the installed one
```

`tests/fixtures/` holds `session.snapshot`, `pane.process_info` and `ps` output captured on a real machine
(anonymised).

## License

MIT — see [LICENSE](LICENSE).
