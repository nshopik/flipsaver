# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
