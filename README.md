# herdr-resource-manager

A herdr popup that shows how much CPU and memory each workspace, tab and pane
uses — for when the fan gets loud and you want the culprit.

```
┌─ herdr-resource-manager ──────────────────────── CPU 100% = 1 core of 11 ─┐
│ Name                                   CPU      Memory  Memory trend       │
│ ────────────────────────────────────────────────────────────────────────── │
│ ▾ [2] notes                          123.2%    523.1 MB  ▁▁▂▂▄▆█           │
│   ▾ [2] Phase 3 plan & impl          123.2%    395.7 MB                    │
│     ● claude  phase-3-plan           123.2%    395.7 MB                    │
│ ▾ Outside herdr  other apps…         144.4%     9.51 GB  ▁▄▄▆▆██           │
│   · Google Chrome                     95.0%     6.32 GB                    │
│ ────────────────────────────────────────────────────────────────────────── │
│ Total in herdr                       123.2%    696.2 MB  ~4% of system RAM │
│ ↑↓ move  ·  space fold  ·  enter focus  ·  x close pane  ·  q quit         │
└────────────────────────────────────────────────────────────────────────────┘
```

macOS only. Python 3.9+ standard library (curses), no build step. Needs herdr ≥ 0.9.3.

## Install

```sh
herdr plugin link /path/to/herdr-resource-manager
```

Bind a key in `~/.config/herdr/config.toml`, then `herdr server reload-config`:

```toml
[[keys.command]]
key = "prefix+m"
type = "plugin_action"
command = "herdr-resource-manager.open"
description = "resource manager"
```

## Keys

| Key | Does |
| --- | --- |
| `↑` `↓` / `j` `k`, `PgUp` `PgDn` | move |
| `space` | fold / unfold a workspace, tab or the Outside row |
| `enter` | focus that workspace / tab / pane and close the popup |
| `x` | close the selected pane (asks `y/N`) |
| `q` / `esc` | quit |

## What the numbers mean

- **Pane** = its shell and every process below it (agents, their language servers, test runs…).
  Tab and workspace rows are sums. Processes that daemonize or are started by launchd (Gradle
  daemon, simulator, `nohup` jobs) leave the pane's tree and show up under **Outside herdr**.
- **CPU** is per core, from the CPU time each process used since the last sample (every 1 s):
  100% = one core busy, so it can exceed 100%. Processes that live less than a second can be missed.
  The first frame shows `…` until two samples exist.
- **Memory** is resident memory (RSS) summed over the processes. Activity Monitor shows the
  "memory footprint" instead, so its numbers differ; shared pages are counted once per process, so
  sums overcount. "% of system RAM" is therefore approximate.
- **Outside herdr** = every other process on the machine, with the top 3 apps by CPU (helpers grouped
  under their `.app`).
- **Memory trend** keeps history only while the popup is open; a change under 5% of the row's peak
  draws flat.
- The popup's own pane is left out. Sampling costs about 2% of one core while it is open.

## Develop

```sh
python3 -m unittest discover -s tests -v       # also with /usr/bin/python3 (3.9)
./herdr-resource-manager --once                # one sample as JSON, run inside herdr
```

`tests/fixtures/` holds real `session.snapshot`, `pane.process_info` and `ps` output. The plugin
talks to herdr over `$HERDR_SOCKET_PATH` (default `~/.config/herdr/herdr.sock`), using
`session.snapshot`, `pane.process_info`, `workspace.focus`, `tab.focus`, `pane.focus`, `pane.close`.
The popup is not a herdr pane (it runs as a child of the herdr server), so it shows up under
Outside herdr.
