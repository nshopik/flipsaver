# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Per-monitor settings survive `DISPLAYn` renumbering**: `[Screen …]`
  sections are keyed by the monitor's device interface path instead of
  the `DISPLAYn` name Windows can reassign. The path includes the GPU
  port, so moving a cable to another port may reset that monitor to
  defaults. An existing `[Screen DISPLAYn]` section still applies, and
  the next dialog OK rewrites it under the new key.
- **Default world-clock cities**: now eight (Los Angeles, New York,
  London, Frankfurt, Moscow, Mumbai, Singapore, Tokyo); replaces the
  previous six. Existing `[WorldClocks]` sections are unaffected.

### Fixed

- **Settings dialog no longer overwrites an unreadable `Settings.ini`**:
  a locked, unreadable or non-UTF-8 file shows an error instead of
  opening the dialog with defaults. `/s` and `/p` decode such a file
  leniently.
- **Failed save is reported**: OK in the settings dialog shows the
  error and keeps the dialog open when `Settings.ini` cannot be written.
- **City labels over 16 characters** survive a settings-dialog save;
  the board still shows the first 16.
- **Out-of-range `Scale` / `BoardScale`** in a hand-edited
  `Settings.ini` is clamped to 0..100 instead of blanking the screen.
- **Non-ASCII command-line argument** (e.g. `/é`) opens the settings
  dialog instead of crashing.
- **Settings dialog is owned by the Control Panel window** passed in
  `/c:HWND`, so a second dialog cannot open behind it.
- **No orphaned process**: the saver exits when it could not create
  any window, or when its last window is closed from outside.
- **Saver exits when another app takes the foreground**, so keystrokes
  never go to a hidden window.
- **Display change while running**: the saver exits on a resolution or
  monitor-layout change instead of drawing stretched or missing a new
  monitor; a resized window rebuilds its render target.
- **Black monitor after a failed board build** retries every second;
  render failures are logged via OutputDebugString.
- **UTF-8 BOM** at the start of `Settings.ini` no longer hides the
  first section.

## [0.2.0] - 2026-07-07

### Added

- **World-clock board**: any monitor can show a split-flap board of
  city/time rows instead of the clock (`World` per-monitor option in
  the settings dialog; cities in `[WorldClocks]` in `Settings.ini`, six
  preloaded). Own "World size" slider (`BoardScale`, default 100%),
  independent of the clock size.
- **Flip animation**: digit changes fold like a split-flap board (~600ms),
  toggleable in the settings dialog (`FlipAnimation` in `[General]`, on by
  default).
- **Vertical clock layout** (hours above minutes), chosen automatically on
  portrait monitors.
- **Per-monitor orientation** in the settings dialog: Auto / Horizontal /
  Vertical, stored per display in `Settings.ini` under `[Screen <name>]`.

## [0.1.0] - 2026-07-06

### Added

- **System font preference**, uses installed Helvetica LT Std Condensed
  when present, falls back to embedded Oswald; font in use shown in the
  settings dialog.
- **Fullscreen flip clock** (`/s`) on every monitor, exits on input.
- **Live preview** (`/p`) for the Windows screensaver control panel.
- **Settings dialog** (`/c`), 12/24 h format and size; stored in
  `%LOCALAPPDATA%\flipsaver\Settings.ini`.
- **`--version` flag.**
