# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
