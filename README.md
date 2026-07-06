# flipsaver

Rust rewrite of the [FlipIt](https://github.com/phaselden/FlipIt)
flip-clock screensaver for Windows — itself inspired by the original
[Fliqlo](https://fliqlo.com/) by Yuji Adachi. A single small native
`.scr` with no runtime dependencies.

v0.1 ships the local flip clock on all monitors, live preview, and a
minimal settings dialog (12/24 h, size). World times come later.

## Install

Build (see `docs/BUILDING.md`) or take a release `flipsaver.scr`, copy
it anywhere on a Windows 10 1703+ machine, right-click → Install. Or
test-run directly: `flipsaver.scr /s`.

Settings live in `%LOCALAPPDATA%\flipsaver\Settings.ini`.

## Font

If Helvetica LT Std Condensed (the font the original FlipIt uses) is
installed on the system, flipsaver uses it automatically. It is a
licensed font and is never shipped with the binary.

Otherwise digits render in
[Oswald](https://github.com/googlefonts/OswaldFont) Bold (static cut),
embedded in the binary. Licensed under the SIL Open Font License — see
`assets/OFL.txt`.

The settings dialog (`/c`) shows which font is in use; it is also
logged at startup via OutputDebugString (`flipsaver: font: ...`).
