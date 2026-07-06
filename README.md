# flipsaver

Rust rewrite of the [FlipIt](https://github.com/phaselden/FlipIt)
flip-clock screensaver for Windows: a single small native `.scr`, no
runtime dependencies, cold start well under FlipIt's ~1 s.

v0.1 ships the local flip clock on all monitors, live preview, and a
minimal settings dialog (12/24 h, size). World times come later.

## Install

Build (see `docs/BUILDING.md`) or take a release `flipsaver.scr`, copy
it anywhere on a Windows 10 1703+ machine, right-click → Install. Or
test-run directly: `flipsaver.scr /s`.

Settings live in `%LOCALAPPDATA%\flipsaver\Settings.ini`.

## Cold start

Median of 5 cold runs (standby list flushed), first frame timed by
built-in QueryPerformanceCounter instrumentation
(`FLIPSAVER_LOG=<path>` to capture; protocol in
`docs/manual-test-matrix.md`):

| Binary | First frame (median) |
|---|---|
| flipsaver v0.1 | _measure me_ |
| FlipIt (same machine) | _measure me_ |

## Font

Digits render in [Oswald](https://github.com/googlefonts/OswaldFont)
Bold (static cut), embedded in the binary. Licensed under the SIL Open
Font License — see `assets/OFL.txt`.
