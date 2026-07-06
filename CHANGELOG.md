# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Fullscreen flip clock** (`/s`) on every monitor, exits on input.
- **Live preview** (`/p`) for the Windows screensaver control panel.
- **Settings dialog** (`/c`), 12/24 h format and size; stored in
  `%LOCALAPPDATA%\flipsaver\Settings.ini`.
- **Startup instrumentation**, first-frame time via OutputDebugString
  and `FLIPSAVER_LOG`.
- **`--version` flag.**
