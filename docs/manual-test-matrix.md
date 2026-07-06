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

## Fidelity (side-by-side vs FlipIt, same machine)

Proportions, colors (#121212→#0A0A0A boxes, #B7B7B7 digits), split line
per box, AM top / PM bottom marker at 9% size, border scaling across the
size slider. Pixel-perfection not required; structure and proportions are.

## Cold-start measurement (spec target: < 100 ms median)

1. `set FLIPSAVER_LOG=%TEMP%\flipsaver.log`
2. Flush the standby list (RAMMap → Empty → Empty Standby List, or
   `EmptyStandbyList.exe standbylist`).
3. Run `flipsaver.scr /s`, exit. Repeat for 5 measured runs, flushing
   before each.
4. Median of the 5 `first frame in N ms` lines → README results table.
5. Same protocol against FlipIt.scr (timestamp instrumentation:
   DebugView first-paint OutputDebugString is absent in FlipIt — use a
   stopwatch-by-eye or ETW `Microsoft-Windows-Win32k` focus events; note
   the method next to the number).
