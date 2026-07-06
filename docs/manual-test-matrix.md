# flipsaver v0.1 — manual test matrix (Windows)

Run after `scripts/deploy.sh`. Reference for fidelity: FlipIt on the
same machine.

## Functional

| # | Check | Pass |
|---|---|---|
| 1 | `/s`: clock on every monitor, cursor hidden everywhere | |
| 2 | `/s`: exits on mouse move / any key / any click (first synthetic move ignored) | |
| 3 | `/s` on mixed-DPI multi-monitor: proportions correct on each screen | |
| 4 | Control panel preview (`/p`): miniature clock, 1 px split, minute ticks over live | |
| 5 | `flipsaver.scr /p 99999999`: exits 0 silently, no UI | |
| 6 | No args / `/c`: dialog opens; OK persists 12/24h + scale to `%LOCALAPPDATA%\flipsaver\Settings.ini`; Cancel does not; `/s` reflects saved values | |
| 7 | Unknown arg (`flipsaver.scr /x`): opens config dialog (declared deviation) | |
| 8 | Install via right-click → Install; runs from lock-screen idle | |
| 9 | `--version` prints `Version: <tag> (<sha>), built for: windows-x86_64` | |
| 10 | Delete/corrupt Settings.ini → `/s` runs with defaults (12h, scale 70) | |
| 11 | Portrait monitor with no `[Screen]` section renders vertical by default; landscape unchanged | |
| 12 | `/c`: one row per monitor; setting Vertical on a landscape monitor then `/s` renders vertical | |
| 13 | Mixed multi-monitor: one Horizontal + one Vertical override both honored simultaneously | |
| 14 | Orientation persists per monitor across restarts; `Settings.ini` gains `[Screen <name>]` only for non-Auto | |
| 15 | Undock a monitor, save from `/c`, redock: the undocked monitor's override survived | |
| 16 | Old `Settings.ini` (no `[Screen]` sections) → all monitors Auto, landscape identical to v0.1 | |

## Fidelity (side-by-side vs FlipIt, same machine)

Proportions, colors (#121212→#0A0A0A boxes, #B7B7B7 digits), split line
per box, AM top / PM bottom marker at 9% size, border scaling across the
size slider. Pixel-perfection not required; structure and proportions are.

