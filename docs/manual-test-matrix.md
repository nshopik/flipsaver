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
| 17 | Minute flip: minute box folds old→new over ~600ms (upper leaf down, then new lower falls) | |
| 18 | Top of hour (e.g. 12:59→13:00): hour and minute boxes flip simultaneously | |
| 19 | `/c`: uncheck "Flip animation", `/s` → value snaps instantly, no fold; re-check restores fold | |
| 20 | Preview (`/p`) animates the fold on each minute tick | |
| 21 | Multi-monitor: each screen's flip runs independently | |
| 22 | CPU idle between flips (fast timer runs only during the ~600ms fold, not continuously) | |
| 23 | Hinge line stays visually static through the fold — no vertical smear (catches reversed matrix multiply) | |
| 24 | No spurious flip in the first second after `/s` launch (first paint primes, no startup fold) | |
| 25 | Absent `FlipAnimation` key in old `Settings.ini` → animation on by default | |
| 26 | `/c`: select World on a monitor, `/s` → that monitor shows the board, others the clock; persists across restarts (`Mode=1` under `[Screen <name>]`) | |
| 27 | First dialog OK writes the six default cities to `[WorldClocks]`; hand-edit (reorder/add/remove) is honored, row order matches file order | |
| 28 | Mixed DST zones: `*` only on rows whose zone is currently in DST (e.g. London summer yes, Dubai never) | |
| 29 | Date-boundary zones (Sydney/LA vs local): day column (`WED`) shown only when the zone's date differs from local; correct across local midnight | |
| 30 | 12/24 h setting applies to board rows (AM/PM cells only in 12 h) | |
| 31 | Invalid timezone ID row renders city + `--:--`, logged via OutputDebugString; other rows unaffected | |
| 32 | Empty `[WorldClocks]` section (all cities deleted) → World monitor falls back to the clock | |
| 33 | Board minute tick flips only changed cells (fold animation); `FlipAnimation` off → instant swap | |
| 34 | Board on single monitor, and preview (`/p`) when primary is World | |
| 35 | No grid reflow at midnight or DST transition (day/`*` columns always reserved) | |
| 36 | Non-ASCII city label (e.g. `Zürich`) renders; label > 16 chars truncated | |

## Fidelity (side-by-side vs FlipIt, same machine)

Proportions, colors (#121212→#0A0A0A boxes, #B7B7B7 digits), split line
per box, AM top / PM bottom marker at 9% size, border scaling across the
size slider. Pixel-perfection not required; structure and proportions are.

