# Popup Resource Manager for herdr

Which agent is making your fan spin? Press one key in [herdr](https://herdr.dev) and a popup shows how much CPU and
memory each workspace, tab and pane uses. It also shows what the rest of your machine is using. It's one small
program and runs on macOS and Linux.

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

## What you get

- CPU and memory for each **workspace, tab and pane**. A pane's numbers include everything it started: the agent,
  its language servers, its test runs.
- A colored dot shows each agent's state (working, blocked or idle), next to the task it's on.
- A small **memory trend** chart for each workspace, while the popup is open.
- An **Outside herdr** row for the rest of your machine, with the 3 busiest apps. Often the real problem is a browser
  or a simulator, not an agent.
- Workspaces for a git worktree are listed under their main workspace.
- Press `enter` or click a pane twice to jump to it, or `x` to close it. Closing asks you first.
- Nothing else to install: herdr is all you need.

## What you need

- macOS (Apple silicon or Intel) or Linux (x86_64 or arm64)
- herdr 0.9.3 or newer

The install downloads a ready-made program for your system from this repo's GitHub releases and checks that it
wasn't changed on the way. If that doesn't work (unknown system, no internet, bad download), it builds the program
from source instead. Building needs Rust 1.95 or newer ([rustup.rs](https://rustup.rs)).

## Install

```sh
herdr plugin install Ruslanbek0809/herdr-popup-resource-manager
```

herdr plugins can't add their own shortcut, so add one yourself. Put this in `~/.config/herdr/config.toml`, then run
`herdr server reload-config`. Any free key works; `prefix+m` is just an example.

```toml
[[keys.command]]
key = "prefix+m"
type = "plugin_action"
command = "herdr-popup-resource-manager.open"
description = "resource manager"
```

To remove it, run `herdr plugin uninstall herdr-popup-resource-manager` and delete those lines.

## Keys and mouse

| Key | What it does |
| --- | --- |
| `↑` `↓` or `j` `k`, `PgUp` `PgDn` | move |
| `space` | open or close a workspace, tab or the Outside row |
| `enter` | go to that workspace, tab or pane, and close the popup |
| `x` | close the selected pane (asks `y/N` first) |
| `q` or `esc` | quit |

With the mouse: click a row to select it, and click it again to go there. Click the ▾ or ▸ arrow to open or close a
row. The scroll wheel moves up and down the list.

## What the numbers mean

- **A pane** counts its shell and every program started from it. Tab and workspace rows add up their panes. Some
  programs move themselves to the background (the Gradle daemon, the iOS simulator, `nohup` jobs). They leave the
  pane and show up under **Outside herdr** instead.
- **CPU** is counted per core and updates every second. 100% means one core is fully busy, so a row can go above
  100%. A program that runs for less than a second may be missed.
- **Memory** is the memory each program holds right now (RSS), added up. Activity Monitor shows a different measure,
  so its numbers won't match. Memory that programs share gets counted more than once, so the totals run a bit high.
  That's why "% of system RAM" is only a rough guide.
- **Outside herdr** is every other program on the machine. It lists the 3 busiest apps. On macOS, an app's helper
  programs are counted with the app. The popup itself isn't counted anywhere.
- **The memory trend** starts when you open the popup. The chart's scale is never smaller than 5% of the row's peak,
  so tiny changes stay near the bottom.
- The popup uses about 3% of one core while it's open (measured on macOS), and nothing when it's closed.

## What it runs

herdr doesn't check or sandbox plugins, so here is everything this one does:

- **On install and update**, `scripts/fetch-or-build.sh` downloads `herdr-popup-resource-manager-<platform>` and
  the `SHA256SUMS` file for this version from this repo's GitHub release. It checks the file and puts the program in
  `target/release/`. If anything goes wrong, it runs `cargo build --release --locked` instead.
- **When you press your key**, `scripts/open.sh` checks that the program is there and asks herdr to open the popup.
  If the program is missing, you get a herdr notification instead.
- **While the popup is open**, it reads the list of running programs once a second: with `ps -A` on macOS, and from
  `/proc` on Linux. It asks herdr for its layout over herdr's local socket (`session.snapshot` and
  `pane.process_info`). It asks herdr to switch panes only when you press `enter` or click a selected row (`workspace.focus`, `tab.focus`,
  `pane.focus`). It asks herdr to close a pane only when you press `x` and then `y` (`pane.close`).
- The popup never goes online, writes no files and leaves nothing running after you close it.
- If you start it outside herdr, it shows the error and `q` quits.

## Develop

```sh
cargo test                                                # tests (screen snapshots via insta)
cargo clippy --all-targets -- -D warnings
cargo build --release
./target/release/herdr-popup-resource-manager --once      # one reading as JSON; run it inside herdr
herdr plugin link "$PWD"                                  # use your copy; link doesn't build, so build first
```

Pushing a `v*` tag runs `.github/workflows/release.yml`. It builds the program for all four systems and publishes
them with `SHA256SUMS`. The tag must match the version in `Cargo.toml` and `herdr-plugin.toml`.

The test data in `tests/fixtures/` was recorded on a real machine, with personal details removed.

## License

MIT. See [LICENSE](LICENSE).
