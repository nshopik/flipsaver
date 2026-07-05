# flipsaver v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rust rewrite of the FlipIt flip-clock screensaver as a single native Windows `.scr` with cold start < 100 ms to first frame.

**Architecture:** Plain Win32 shell (one topmost `WS_POPUP` window per monitor), Direct2D/DirectWrite rendering via `ID2D1HwndRenderTarget`, static redraw when the minute changes. Pure layout/parsing/settings modules compile and unit-test on the Linux host; all Win32 code is `#[cfg(windows)]`-gated and verified by cross-building with cargo-xwin.

**Tech Stack:** Rust, Microsoft `windows` crate, Direct2D + DirectWrite, cargo-xwin cross build from WSL2 (`x86_64-pc-windows-msvc`), embedded Oswald Bold (SIL OFL) static TTF.

**Spec:** `docs/superpowers/specs/2026-07-06-flipsaver-v0.1-design.md` (in the primary checkout; behavior reference: C# original at `/home/shopik/FlipIt`, read-only).

## Team Roster

| Task | Implementer | Spec Reviewer | Quality Reviewer |
|---|---|---|---|
| Task 1: Project scaffold + cross toolchain | `voltagent-dev-exp:build-engineer` | `voltagent-dev-exp:build-engineer` | `voltagent-qa-sec:code-reviewer` |
| Task 2: Command-line parsing | `generic` | `generic` | `voltagent-qa-sec:code-reviewer` |
| Task 3: INI settings module | `generic` | `generic` | `voltagent-qa-sec:code-reviewer` |
| Task 4: Layout math + time formatting | `generic` | `generic` | `voltagent-qa-sec:code-reviewer` |
| Task 5: Win32 fullscreen shell + exit-on-input | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` |
| Task 6: D2D/DWrite init + embedded font | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` |
| Task 7: Render target, first blank frame, startup measurement | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:performance-engineer` |
| Task 8: Clock face drawing | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` |
| Task 9: Device-loss recovery + WM_DPICHANGED | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` |
| Task 10: /p preview mode | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` |
| Task 11: /c config dialog (DLGTEMPLATE) | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` |
| Task 12: --version, build.rs, manifest embedding | `voltagent-dev-exp:build-engineer` | `voltagent-dev-exp:build-engineer` | `voltagent-qa-sec:code-reviewer` |
| Task 13: Docs, deploy script, manual test matrix, CHANGELOG | `voltagent-biz:technical-writer` | `voltagent-qa-sec:qa-expert` | `voltagent-qa-sec:code-reviewer` |

## Global Constraints

- Platform floor: **Windows 10 1703+** (needed by `IDWriteFactory5::CreateInMemoryFontFileLoader` and per-monitor-V2 DPI). No `implement` feature of the `windows` crate.
- Build target: `x86_64-pc-windows-msvc` via **cargo-xwin** from WSL2. `cargo test` must always pass on the **Linux host** — every Win32 item is `#[cfg(windows)]`-gated and the `windows` crate is a `[target.'cfg(windows)'.dependencies]` entry.
- Release profile exactly: `opt-level = "s"`, `lto = true`, `panic = "abort"`, `strip = true`. `panic = "abort"` is **release-only**.
- Font: **Oswald Bold static single-weight TTF** (SIL OFL), embedded via `include_bytes!`. The Google-Fonts *variable* build must NOT be used. Helvetica LT Std Condensed must never enter this repo. `assets/OFL.txt` kept.
- Settings: `%LOCALAPPDATA%\flipsaver\Settings.ini`, keys `[General] Display24Hr` (0/1, default 0) and `Scale` (0..100, default 70). Missing/corrupt → defaults, never an error.
- No error UI ever in `/s` or `/p` paths. Invalid/missing `/p` hwnd → exit 0 silently. Unknown args → config dialog.
- `--version` line: `Version: <tag> (<short-sha>), built for: windows-x86_64`; tag `dev` when untagged.
- Commit messages: Scoped Commits (`<scope>: <description>`, imperative, ≤50-char subject). Scopes used here: `build`, `cli`, `settings`, `clock`, `screensaver`, `gfx`, `preview`, `config`, `version`, `docs`.
- Comments: why-not-what, sparse.
- windows-crate API shapes drift between releases. If a call in this plan doesn't compile against the pinned crate version, consult docs.rs for that version and adapt the **call shape only** — never the logic or constants. The gate for every Win32 task is `cargo xwin build --release --target x86_64-pc-windows-msvc` succeeding.
- Steps that need a live Windows machine are marked **Manual (Windows)** — record them in `docs/manual-test-matrix.md` (Task 13); they are not execution gates for the automated loop.

---

### Task 1: Project scaffold + cross toolchain

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `docs/BUILDING.md`
- Test: none (scaffold; gates are `cargo test` and `cargo xwin build`)

**Interfaces:**
- Consumes: nothing.
- Produces: a crate named `flipsaver` where `cargo test` runs on Linux and `cargo xwin build --release --target x86_64-pc-windows-msvc` produces `target/x86_64-pc-windows-msvc/release/flipsaver.exe`. Later tasks add modules to `src/`.

- [ ] **Step 1: Install/verify the cross toolchain (idempotent)**

```bash
rustup target add x86_64-pc-windows-msvc
command -v cargo-xwin >/dev/null || cargo install cargo-xwin --locked
command -v clang >/dev/null && command -v lld >/dev/null || sudo apt-get install -y clang lld
cargo xwin --version
```

Expected: each command succeeds; last line prints the cargo-xwin version (record it in Step 4).

- [ ] **Step 2: Create `Cargo.toml`**

```toml
[package]
name = "flipsaver"
version = "0.1.0"
edition = "2021"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Direct2D",
    "Win32_Graphics_Direct2D_Common",
    "Win32_Graphics_DirectWrite",
    "Win32_Graphics_Dxgi_Common",
    "Win32_System_Com",
    "Win32_System_LibraryLoader",
    "Win32_System_Performance",
    "Win32_System_SystemInformation",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_Console",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_HiDpi",
    "Win32_UI_Controls",
] }

# panic="abort" is release-only: dev/test profiles keep unwinding so
# `cargo test` on the Linux host works.
[profile.release]
opt-level = "s"
lto = true
panic = "abort"
strip = true
```

- [ ] **Step 3: Create `src/main.rs` stub**

```rust
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {}
```

- [ ] **Step 4: Create `docs/BUILDING.md`**

```markdown
# Building flipsaver

Cross-compiled for Windows from WSL2. No Windows toolchain required.

## One-time setup

    rustup target add x86_64-pc-windows-msvc
    cargo install cargo-xwin --locked
    sudo apt-get install -y clang lld

Pinned versions (splat layout drifts across xwin releases — if a build
breaks after reinstalling, reinstall exactly these):

- cargo-xwin: <output of `cargo xwin --version`>
- rustc: <output of `rustc --version`>

First build downloads + splats the MSVC CRT and Windows SDK (~1.5 GB)
and requires accepting the Microsoft license (set `XWIN_ACCEPT_LICENSE=1`
for non-interactive builds).

## Build

    cargo xwin build --release --target x86_64-pc-windows-msvc

Output: `target/x86_64-pc-windows-msvc/release/flipsaver.exe` (< 1 MB).

## Test (Linux host)

    cargo test

Only pure modules (arg parsing, INI, layout math) compile on the host;
all Win32 code is `#[cfg(windows)]`-gated.
```

Replace the two `<output of ...>` placeholders with the real command outputs.

- [ ] **Step 5: Verify host tests and cross build**

Run: `cargo test`
Expected: `running 0 tests ... test result: ok. 0 passed`

Run: `XWIN_ACCEPT_LICENSE=1 cargo xwin build --release --target x86_64-pc-windows-msvc`
Expected: ends with `Finished \`release\` profile`; `ls -la target/x86_64-pc-windows-msvc/release/flipsaver.exe` shows the binary.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs docs/BUILDING.md
git commit -m "build: scaffold crate with cargo-xwin cross toolchain"
```

---

### Task 2: Command-line parsing

**Files:**
- Modify: `src/main.rs`
- Test: unit tests inside `src/main.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum Mode { Screensaver, Config, Preview(Option<isize>), Version }` and `pub fn parse_args(args: &[String]) -> Mode` in `src/main.rs` (args exclude argv[0]). Tasks 5/10/11/12 dispatch on this.

Reference behavior is `Program.cs` in FlipIt: first arg lowercased+trimmed; if longer than 2 chars, the argument is `substring(3)` (the char at index 2 is a separator, discarded unchecked, so `/p:1234`, `/p_1234`, `/p:1234` all work); otherwise a second argv element is the argument. Declared deviations (spec): unknown args and `/c` both → config dialog; unparsable/missing `/p` hwnd → `Preview(None)` (caller exits 0 silently).

- [ ] **Step 1: Write the failing tests (append to `src/main.rs`)**

```rust
#[cfg(test)]
mod cli_tests {
    use super::*;

    fn p(args: &[&str]) -> Mode {
        parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn no_args_is_config() { assert_eq!(p(&[]), Mode::Config); }
    #[test]
    fn s_fullscreen() { assert_eq!(p(&["/s"]), Mode::Screensaver); }
    #[test]
    fn s_uppercase() { assert_eq!(p(&["/S"]), Mode::Screensaver); }
    #[test]
    fn c_config() { assert_eq!(p(&["/c"]), Mode::Config); }
    #[test]
    fn c_with_hwnd_is_config() { assert_eq!(p(&["/c:12345"]), Mode::Config); }
    #[test]
    fn p_space_form() { assert_eq!(p(&["/p", "1234"]), Mode::Preview(Some(1234))); }
    #[test]
    fn p_colon_form() { assert_eq!(p(&["/p:1234"]), Mode::Preview(Some(1234))); }
    #[test]
    fn p_uppercase_colon() { assert_eq!(p(&["/P:1234"]), Mode::Preview(Some(1234))); }
    #[test]
    fn p_any_separator_char() { assert_eq!(p(&["/p_1234"]), Mode::Preview(Some(1234))); }
    #[test]
    fn p_missing_hwnd() { assert_eq!(p(&["/p"]), Mode::Preview(None)); }
    #[test]
    fn p_garbage_hwnd() { assert_eq!(p(&["/p", "abc"]), Mode::Preview(None)); }
    #[test]
    fn p_bare_separator() { assert_eq!(p(&["/p:"]), Mode::Preview(None)); }
    #[test]
    fn unknown_arg_is_config() { assert_eq!(p(&["/foo"]), Mode::Config); }
    #[test]
    fn version_flag() {
        assert_eq!(p(&["--version"]), Mode::Version);
        assert_eq!(p(&["-V"]), Mode::Version);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: compile error — `Mode`/`parse_args` not found.

- [ ] **Step 3: Implement (insert above the tests in `src/main.rs`)**

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Screensaver,
    Config,
    Preview(Option<isize>),
    Version,
}

pub fn parse_args(args: &[String]) -> Mode {
    let Some(first_raw) = args.first() else {
        return Mode::Config;
    };
    if first_raw == "--version" || first_raw == "-V" {
        return Mode::Version;
    }
    let mut first: String = first_raw.to_lowercase().trim().to_string();
    let mut second: Option<String> = None;
    if first.len() > 2 {
        // Original discards the char at index 2 unchecked, so any
        // separator works: /p:1234, /p_1234, ...
        second = Some(first[3.min(first.len())..].trim().to_string());
        first.truncate(2);
    } else if args.len() > 1 {
        second = Some(args[1].clone());
    }
    match first.as_str() {
        "/s" => Mode::Screensaver,
        "/p" => Mode::Preview(second.and_then(|s| s.parse::<i64>().ok()).map(|v| v as isize)),
        // "/c" and unknown args both open config (declared deviation:
        // original shows an error MessageBox for unknown args).
        _ => Mode::Config,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: `test result: ok. 14 passed`

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "cli: parse screensaver argument forms"
```

---

### Task 3: INI settings module

**Files:**
- Create: `src/settings.rs`
- Modify: `src/main.rs` (add `mod settings;`)
- Test: unit tests inside `src/settings.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 5, 8, 10, 11):
  - `pub struct Settings { pub display_24hr: bool, pub scale: i32 }` (`Copy`, `Default` = `{ false, 70 }`)
  - `pub fn load(path: &Path) -> Settings`
  - `pub fn save(path: &Path, s: Settings) -> std::io::Result<()>` (creates parent dir)
  - `pub fn default_path() -> PathBuf` → `%LOCALAPPDATA%\flipsaver\Settings.ini`

Parser replicates `IniFile.cs`: trim each whole line; blank → skip; `[...]` → section; otherwise split on the **first** `=`; a line without `=` is silently skipped (this is the only "comment" mechanism — no special `#` handling); keys are matched exactly with no extra trimming. Any unparsable value → default for that key, never an error.

- [ ] **Step 1: Write the failing tests (bottom of new `src/settings.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = Settings::default();
        assert!(!s.display_24hr);
        assert_eq!(s.scale, 70);
    }

    #[test]
    fn parses_both_keys() {
        let s = Settings::from_ini_text("[General]\r\nDisplay24Hr=1\r\nScale=40\r\n");
        assert!(s.display_24hr);
        assert_eq!(s.scale, 40);
    }

    #[test]
    fn lines_without_equals_are_skipped() {
        let s = Settings::from_ini_text("[General]\nthis is a comment\nScale=30\n");
        assert_eq!(s.scale, 30);
    }

    #[test]
    fn garbage_values_fall_back_to_defaults() {
        let s = Settings::from_ini_text("[General]\nDisplay24Hr=yes\nScale=abc\n");
        assert!(!s.display_24hr);
        assert_eq!(s.scale, 70);
    }

    #[test]
    fn keys_outside_general_are_ignored() {
        let s = Settings::from_ini_text("[Other]\nScale=10\n");
        assert_eq!(s.scale, 70);
    }

    #[test]
    fn display24hr_only_1_is_true() {
        assert!(!Settings::from_ini_text("[General]\nDisplay24Hr=2\n").display_24hr);
        assert!(!Settings::from_ini_text("[General]\nDisplay24Hr=0\n").display_24hr);
    }

    #[test]
    fn round_trip() {
        let s = Settings { display_24hr: true, scale: 90 };
        assert_eq!(Settings::from_ini_text(&s.to_ini_text()), s);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let s = load(Path::new("/nonexistent/definitely/Settings.ini"));
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn save_creates_directory_and_loads_back() {
        let dir = std::env::temp_dir()
            .join(format!("flipsaver-test-{}", std::process::id()))
            .join("nested");
        let path = dir.join("Settings.ini");
        let s = Settings { display_24hr: true, scale: 20 };
        save(&path, s).unwrap();
        assert_eq!(load(&path), s);
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test settings`
Expected: compile error — module items not found (after adding `mod settings;` to `src/main.rs`).

- [ ] **Step 3: Implement (top of `src/settings.rs`); add `mod settings;` near the top of `src/main.rs`**

```rust
use std::path::{Path, PathBuf};

const DEFAULT_SCALE: i32 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub display_24hr: bool,
    pub scale: i32, // 0..=100, slider value x 10
}

impl Default for Settings {
    fn default() -> Self {
        Settings { display_24hr: false, scale: DEFAULT_SCALE }
    }
}

impl Settings {
    pub fn from_ini_text(text: &str) -> Settings {
        let mut s = Settings::default();
        let mut section = String::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].to_string();
                continue;
            }
            // A line without '=' is the original's only comment mechanism.
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if section == "General" {
                match key {
                    // Convert.ToInt32 tolerates surrounding whitespace,
                    // hence the value trim; keys get no extra trimming.
                    "Display24Hr" => {
                        s.display_24hr = value.trim().parse::<i32>().map(|v| v == 1).unwrap_or(false)
                    }
                    "Scale" => s.scale = value.trim().parse().unwrap_or(DEFAULT_SCALE),
                    _ => {}
                }
            }
        }
        s
    }

    pub fn to_ini_text(&self) -> String {
        format!(
            "[General]\r\nDisplay24Hr={}\r\nScale={}\r\n\r\n",
            if self.display_24hr { 1 } else { 0 },
            self.scale
        )
    }
}

pub fn load(path: &Path) -> Settings {
    match std::fs::read_to_string(path) {
        Ok(text) => Settings::from_ini_text(&text),
        Err(_) => Settings::default(),
    }
}

pub fn save(path: &Path, s: Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, s.to_ini_text())
}

pub fn default_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("flipsaver").join("Settings.ini")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: `test result: ok. 23 passed` (14 cli + 9 settings)

- [ ] **Step 5: Commit**

```bash
git add src/settings.rs src/main.rs
git commit -m "settings: INI load/save replicating IniFile.cs"
```

---

### Task 4: Layout math + time formatting (pure, host-testable)

**Files:**
- Create: `src/clock.rs`
- Modify: `src/main.rs` (add `mod clock;`)
- Test: unit tests inside `src/clock.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (used by Task 8 drawing):
  - `pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }` (`Copy`)
  - `pub struct BoxLayout { pub rect: Rect, pub text: Rect, pub split_y: i32, pub marker_top: Option<(i32, i32)>, pub marker_bottom: Option<(i32, i32)> }`
  - `pub struct Layout { pub hours: BoxLayout, pub minutes: BoxLayout, pub corner_radius: i32, pub large_font_px: i32, pub small_font_px: i32, pub split_stroke: f32 }`
  - `pub fn compute(width: i32, height: i32, scale_percent: i32, is_24h: bool, is_preview: bool) -> Layout`
  - `pub enum Marker { Am, Pm }` and `pub fn format_time(hour: u32, minute: u32, is_24h: bool) -> (String, String, Option<Marker>)`

Fidelity constants replicate `CurrentTimeScreen.cs` exactly, **including its integer math** (`Int32Extensions.Percent(int)` is truncating integer division `value * percent / 100`; `Convert.ToInt32` on doubles rounds — we use `.round()`, which differs from C# banker's rounding only at exact .5, an accepted ≤1 px deviation):
- `border_percent = (100 - scale_percent) / 4 + 5` (integer division; 5–30% as scale goes 100→0)
- box gap = 5% of box size (`round(box * 0.05)`); corner radius = `box / 20`
- large font = `box * 85 / 100`; small (AM/PM) font = `box * 9 / 100`
- text rect: widened by `diff = box/10` each side, nudged `+box*1/100` horizontally, `+box*4/100` vertically
- split line: fullscreen 4 px stroke at `y = box_top + box/2 - 2`; preview 1 px at `y = box_top + box/2` (per box — never spans the face)
- markers (12h, hours box only): `AM` top-left anchor `(x + diff/2, y + diff)`; `PM` anchor `(x + diff/2, box_bottom - diff)` where the anchor is the **bottom edge** of the small text (original places the text top at `bottom - diff - font_height`; the draw code subtracts the measured DWrite line height)

- [ ] **Step 1: Write the failing tests (bottom of new `src/clock.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_percent_range() {
        assert_eq!(border_percent(100), 5);
        assert_eq!(border_percent(70), 12);
        assert_eq!(border_percent(0), 30);
    }

    #[test]
    fn layout_1080p_scale70_fullscreen_12h() {
        let l = compute(1920, 1080, 70, false, false);
        assert_eq!(l.hours.rect, Rect { x: 230, y: 184, w: 712, h: 712 });
        assert_eq!(l.minutes.rect, Rect { x: 978, y: 184, w: 712, h: 712 });
        assert_eq!(l.corner_radius, 35);
        assert_eq!(l.large_font_px, 605);
        assert_eq!(l.small_font_px, 64);
        assert_eq!(l.hours.text, Rect { x: 166, y: 212, w: 854, h: 712 });
        assert_eq!(l.hours.split_y, 538);
        assert_eq!(l.split_stroke, 4.0);
        assert_eq!(l.hours.marker_top, Some((265, 255)));
        assert_eq!(l.hours.marker_bottom, Some((265, 825)));
        // markers only ever on the hours box
        assert_eq!(l.minutes.marker_top, None);
        assert_eq!(l.minutes.marker_bottom, None);
    }

    #[test]
    fn layout_24h_has_no_markers() {
        let l = compute(1920, 1080, 70, true, false);
        assert_eq!(l.hours.marker_top, None);
        assert_eq!(l.hours.marker_bottom, None);
    }

    #[test]
    fn layout_preview_320x240_scale70() {
        let l = compute(320, 240, 70, false, true);
        assert_eq!(l.hours.rect, Rect { x: 38, y: 60, w: 119, h: 119 });
        assert_eq!(l.minutes.rect, Rect { x: 163, y: 60, w: 119, h: 119 });
        // preview split: 1 px hairline at exact box center
        assert_eq!(l.hours.split_y, 119);
        assert_eq!(l.split_stroke, 1.0);
    }

    #[test]
    fn format_12h() {
        assert_eq!(format_time(0, 5, false), ("12".into(), "05".into(), Some(Marker::Am)));
        assert_eq!(format_time(11, 59, false), ("11".into(), "59".into(), Some(Marker::Am)));
        assert_eq!(format_time(12, 0, false), ("12".into(), "00".into(), Some(Marker::Pm)));
        assert_eq!(format_time(13, 7, false), ("1".into(), "07".into(), Some(Marker::Pm)));
    }

    #[test]
    fn format_24h() {
        assert_eq!(format_time(0, 5, true), ("00".into(), "05".into(), None));
        assert_eq!(format_time(23, 5, true), ("23".into(), "05".into(), None));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test clock`
Expected: compile error — items not found (after adding `mod clock;` to `src/main.rs`).

- [ ] **Step 3: Implement (top of `src/clock.rs`); add `mod clock;` to `src/main.rs`**

```rust
//! Layout math for the flip-clock face. Pure data, no Win32 types, so it
//! unit-tests on the Linux host. Constants replicate CurrentTimeScreen.cs.

const SPLIT_WIDTH: i32 = 4;
const BOX_SEPARATION_PERCENT: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxLayout {
    pub rect: Rect,
    pub text: Rect,
    pub split_y: i32,
    /// Top-left anchor of the AM marker (12h mode, hours box only).
    pub marker_top: Option<(i32, i32)>,
    /// (x, bottom-edge y) anchor of the PM marker; drawer subtracts the
    /// measured small-text line height (original: bottom - diff - Font.Height).
    pub marker_bottom: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub hours: BoxLayout,
    pub minutes: BoxLayout,
    pub corner_radius: i32,
    pub large_font_px: i32,
    pub small_font_px: i32,
    pub split_stroke: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Am,
    Pm,
}

pub fn border_percent(scale_percent: i32) -> i32 {
    (100 - scale_percent) / 4 + 5
}

fn calc_box_size(total: i32, border_percent: i32, box_count: i32) -> i32 {
    let border = total * border_percent / 100; // truncating, like Percent(int)
    let remaining = (total - border * 2) as f64;
    let parts = (1.0 + BOX_SEPARATION_PERCENT) * box_count as f64 - BOX_SEPARATION_PERCENT;
    (remaining / parts).round() as i32
}

fn calc_offset(total: i32, box_count: i32, box_size: i32, separator: i32) -> i32 {
    let all = (box_size + separator) * box_count - separator;
    (total - all) / 2
}

fn box_layout(rect: Rect, with_markers: bool, is_preview: bool) -> BoxLayout {
    let diff = rect.w / 10;
    let x_off = rect.w * 1 / 100;
    let y_off = rect.h * 4 / 100;
    let text = Rect { x: rect.x - diff + x_off, y: rect.y + y_off, w: rect.w + diff * 2, h: rect.h };
    let split_y = if is_preview {
        rect.y + rect.h / 2
    } else {
        rect.y + rect.h / 2 - SPLIT_WIDTH / 2
    };
    let (marker_top, marker_bottom) = if with_markers {
        let left = rect.x + diff / 2;
        (Some((left, rect.y + diff)), Some((left, rect.y + rect.h - diff)))
    } else {
        (None, None)
    };
    BoxLayout { rect, text, split_y, marker_top, marker_bottom }
}

pub fn compute(width: i32, height: i32, scale_percent: i32, is_24h: bool, is_preview: bool) -> Layout {
    let bp = border_percent(scale_percent);
    let box_size = calc_box_size(width, bp, 2).min(calc_box_size(height, bp, 1));
    let sep = (box_size as f64 * BOX_SEPARATION_PERCENT).round() as i32;
    let start_x = calc_offset(width, 2, box_size, sep);
    let start_y = calc_offset(height, 1, box_size, 0);
    let hours_rect = Rect { x: start_x, y: start_y, w: box_size, h: box_size };
    let minutes_rect = Rect { x: start_x + box_size + sep, ..hours_rect };
    Layout {
        hours: box_layout(hours_rect, !is_24h, is_preview),
        minutes: box_layout(minutes_rect, false, is_preview),
        corner_radius: box_size / 20,
        large_font_px: box_size * 85 / 100,
        small_font_px: box_size * 9 / 100,
        split_stroke: if is_preview { 1.0 } else { SPLIT_WIDTH as f32 },
    }
}

pub fn format_time(hour: u32, minute: u32, is_24h: bool) -> (String, String, Option<Marker>) {
    let minute_s = format!("{minute:02}");
    if is_24h {
        (format!("{hour:02}"), minute_s, None)
    } else {
        let h12 = match hour % 12 {
            0 => 12,
            h => h,
        };
        let marker = if hour >= 12 { Marker::Pm } else { Marker::Am };
        (h12.to_string(), minute_s, Some(marker))
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: `test result: ok. 29 passed`

- [ ] **Step 5: Commit**

```bash
git add src/clock.rs src/main.rs
git commit -m "clock: layout math and time formatting"
```

---

### Task 5: Win32 fullscreen shell + exit-on-input

**Files:**
- Create: `src/screensaver.rs`
- Modify: `src/main.rs` (cfg-gated `mod screensaver;`, real `main()` dispatch)
- Test: gate is the cross build (`cargo xwin build`); behavior is Manual (Windows)

**Interfaces:**
- Consumes: `Mode`/`parse_args` (Task 2), `settings::{Settings, load, default_path}` (Task 3).
- Produces (extended by Tasks 6–10):
  - `pub fn run_fullscreen(settings: Settings)` in `src/screensaver.rs`
  - `struct WindowState { is_preview: bool, mouse: Option<(i32, i32)>, settings: Settings }` stored per-window via `GWLP_USERDATA` (later tasks add fields — keep names)
  - window class `"flipsaverwnd"`, shared `wndproc`, `unsafe fn create_saver_window(...) -> HWND`

- [ ] **Step 1: Write `src/screensaver.rs`**

```rust
//! /s mode: one topmost popup per monitor, shared message loop,
//! exit on first real input.

use crate::settings::Settings;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: PCWSTR = w!("flipsaverwnd");

pub struct WindowState {
    pub is_preview: bool,
    pub mouse: Option<(i32, i32)>,
    pub settings: Settings,
}

fn enumerate_monitors() -> Vec<RECT> {
    unsafe extern "system" fn enum_proc(
        _mon: HMONITOR,
        _hdc: HDC,
        rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let v = &mut *(lparam.0 as *mut Vec<RECT>);
        v.push(*rect);
        TRUE
    }
    let mut v: Vec<RECT> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(enum_proc), LPARAM(&mut v as *mut _ as isize));
    }
    v
}

unsafe fn register_class(instance: HINSTANCE) {
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    RegisterClassW(&wc);
}

pub unsafe fn create_saver_window(
    instance: HINSTANCE,
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    parent: Option<HWND>,
    bounds: RECT,
    state: WindowState,
) -> HWND {
    let boxed = Box::into_raw(Box::new(state));
    CreateWindowExW(
        ex_style,
        CLASS_NAME,
        w!("flipsaver"),
        style | WS_VISIBLE,
        bounds.left,
        bounds.top,
        bounds.right - bounds.left,
        bounds.bottom - bounds.top,
        parent,
        None,
        Some(instance),
        Some(boxed as *const core::ffi::c_void),
    )
    .unwrap_or_default()
}

unsafe fn state_of(hwnd: HWND) -> *mut WindowState {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState
}

pub unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = &*(lp.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let state_ptr = state_of(hwnd);
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let state = &mut *state_ptr;
    match msg {
        WM_SETCURSOR if !state.is_preview => {
            // ShowCursor alone is not enough when the pointer crosses
            // per-monitor windows; answer every WM_SETCURSOR with no cursor.
            SetCursor(None);
            LRESULT(1)
        }
        WM_MOUSEMOVE if !state.is_preview => {
            let pt = (lp.0 as i32 & 0xFFFF, (lp.0 as i32 >> 16) & 0xFFFF);
            // Exact original logic: remember the first reported position
            // (a synthetic move arrives on window creation), quit on any
            // different one. No movement threshold.
            if let Some(prev) = state.mouse {
                if prev != pt {
                    PostQuitMessage(0);
                }
            }
            state.mouse = Some(pt);
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
            if !state.is_preview =>
        {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_DESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(state_ptr));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run_fullscreen(settings: Settings) {
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        register_class(instance);
        for bounds in enumerate_monitors() {
            create_saver_window(
                instance,
                WS_POPUP,
                WS_EX_TOPMOST,
                None,
                bounds,
                WindowState { is_preview: false, mouse: None, settings },
            );
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
```

- [ ] **Step 2: Rewrite `main()` in `src/main.rs`**

Replace the stub `fn main() {}` and add the cfg-gated module declarations:

```rust
#[cfg(windows)]
mod screensaver;

#[cfg(windows)]
fn main() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    unsafe {
        // Functional backstop for the manifest (Task 12); must run before
        // any window is created.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        // No CoUninitialize: the process exits right after the loop.
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let settings = settings::load(&settings::default_path());
    match parse_args(&args) {
        Mode::Screensaver => screensaver::run_fullscreen(settings),
        Mode::Preview(Some(_parent)) => {} // wired in Task 10
        Mode::Preview(None) => {}          // declared deviation: exit 0 silently
        Mode::Config => {}                 // wired in Task 11
        Mode::Version => {}                // wired in Task 12
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("flipsaver targets Windows; this host build exists for `cargo test` only.");
}
```

- [ ] **Step 3: Verify host tests still pass**

Run: `cargo test`
Expected: `test result: ok. 29 passed`

- [ ] **Step 4: Verify cross build**

Run: `cargo xwin build --release --target x86_64-pc-windows-msvc`
Expected: `Finished \`release\` profile`. Fix any windows-crate signature drift per Global Constraints.

- [ ] **Step 5: Manual (Windows), deferred to Task 13 matrix**

Copy binary as `flipsaver.scr`, run `flipsaver.scr /s`: black topmost window on every monitor, cursor hidden, exits on mouse move/key/click.

- [ ] **Step 6: Commit**

```bash
git add src/screensaver.rs src/main.rs
git commit -m "screensaver: per-monitor windows and exit-on-input"
```

---

### Task 6: D2D/DWrite init + embedded font

**Files:**
- Create: `assets/Oswald-Bold.ttf` (downloaded, static single-weight), `assets/OFL.txt`
- Modify: `src/screensaver.rs` (add `Gfx` struct), `src/main.rs` (nothing new — `run_fullscreen` builds `Gfx`)
- Test: `fc-scan` asset check on Linux; cross build gate

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 7–10):
  - `pub struct Gfx { pub d2d: ID2D1Factory, pub dwrite: IDWriteFactory5, pub fonts: Option<IDWriteFontCollection1>, pub family: &'static str }`
  - `impl Gfx { pub fn new() -> windows::core::Result<Gfx> }`
  - `WindowState` gains field `pub gfx: std::rc::Rc<Gfx>`

- [ ] **Step 1: Fetch the static Oswald Bold TTF + OFL license**

```bash
mkdir -p assets
curl -fsSL -o assets/Oswald-Bold.ttf \
  https://raw.githubusercontent.com/googlefonts/OswaldFont/main/fonts/ttf/Oswald-Bold.ttf
curl -fsSL -o assets/OFL.txt \
  https://raw.githubusercontent.com/googlefonts/OswaldFont/main/OFL.txt
```

If the first URL 404s (repo layout drift): download the family ZIP from fonts.google.com/specimen/Oswald and use `static/Oswald-Bold.ttf` from inside it — **never** the variable `Oswald[wght].ttf`. OFL.txt must be present either way.

- [ ] **Step 2: Verify the font is the static bold cut**

Run: `fc-scan --format '%{family}|%{weight}|%{variable}\n' assets/Oswald-Bold.ttf`
Expected: family contains `Oswald`, weight `200` (fontconfig's bold), variable `False` (or empty). A `True` variable flag is a hard stop — wrong file.

- [ ] **Step 3: Add `Gfx` to `src/screensaver.rs`**

Append to the `use` block: `use windows::Win32::Graphics::Direct2D::*;` and `use windows::Win32::Graphics::DirectWrite::*;`. Then add:

```rust
static OSWALD_BOLD: &[u8] = include_bytes!("../assets/Oswald-Bold.ttf");

/// Process-wide device-independent graphics resources, shared by every
/// window (fullscreen and preview) via Rc.
pub struct Gfx {
    pub d2d: ID2D1Factory,
    pub dwrite: IDWriteFactory5,
    pub fonts: Option<IDWriteFontCollection1>,
    pub family: &'static str,
}

impl Gfx {
    pub fn new() -> Result<Gfx> {
        unsafe {
            let d2d: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            match Self::load_embedded_font(&dwrite) {
                Ok(fonts) => Ok(Gfx { d2d, dwrite, fonts: Some(fonts), family: "Oswald" }),
                Err(e) => {
                    // Embedded bytes failing to load is a build defect, not
                    // a runtime condition: assert in debug, degrade in release.
                    debug_assert!(false, "embedded font load failed: {e:?}");
                    Ok(Gfx { d2d, dwrite, fonts: None, family: "Segoe UI" })
                }
            }
        }
    }

    /// Requires IDWriteFactory5 (Windows 10 1703+), which is the platform floor.
    unsafe fn load_embedded_font(dwrite: &IDWriteFactory5) -> Result<IDWriteFontCollection1> {
        let loader = dwrite.CreateInMemoryFontFileLoader()?;
        dwrite.RegisterFontFileLoader(&loader.cast::<IDWriteFontFileLoader>()?)?;
        let file = loader.CreateInMemoryFontFileReference(
            dwrite,
            OSWALD_BOLD.as_ptr() as *const core::ffi::c_void,
            OSWALD_BOLD.len() as u32,
            None,
        )?;
        let builder = dwrite.CreateFontSetBuilder()?;
        builder.AddFontFile(&file)?;
        let set = builder.CreateFontSet()?;
        dwrite.CreateFontCollectionFromFontSet(&set)
    }
}
```

- [ ] **Step 4: Build `Gfx` in `run_fullscreen` and hand it to every window**

In `src/screensaver.rs`, add the field to `WindowState`:

```rust
pub struct WindowState {
    pub is_preview: bool,
    pub mouse: Option<(i32, i32)>,
    pub settings: Settings,
    pub gfx: std::rc::Rc<Gfx>,
}
```

and in `run_fullscreen`, before the monitor loop:

```rust
let gfx = match Gfx::new() {
    Ok(g) => std::rc::Rc::new(g),
    Err(_) => return, // no D2D at all: nothing sane to render, exit quietly
};
```

then construct each window's state with `gfx: gfx.clone()`.

- [ ] **Step 5: Verify**

Run: `cargo test`
Expected: `29 passed` (assets don't affect host tests — `include_bytes!` is inside the cfg-gated module).

Run: `cargo xwin build --release --target x86_64-pc-windows-msvc`
Expected: `Finished \`release\` profile`.

- [ ] **Step 6: Commit**

```bash
git add assets/Oswald-Bold.ttf assets/OFL.txt src/screensaver.rs
git commit -m "gfx: D2D/DWrite factories and embedded Oswald Bold"
```

---

### Task 7: Render target, first blank frame, startup measurement

Completes spec Milestone 1: the timed path now includes every expensive init (COM, DPI, factories, font collection, render target, first `EndDraw`), so the < 100 ms budget is genuinely gated even though the frame is blank.

**Files:**
- Modify: `src/main.rs` (add `perf` module, mark start first thing in `main`)
- Modify: `src/screensaver.rs` (render target + `WM_PAINT`/`WM_ERASEBKGND`)
- Test: cross build gate; timing numbers are Manual (Windows)

**Interfaces:**
- Consumes: `Gfx` (Task 6), `WindowState` (Task 5).
- Produces (used by Tasks 8–10):
  - `pub mod perf` in `src/main.rs` (cfg windows): `pub fn mark_start()`, `pub fn log_first_frame()` (idempotent — logs once)
  - `WindowState` gains `pub target: Option<ID2D1HwndRenderTarget>`
  - `unsafe fn ensure_target(hwnd: HWND, state: &mut WindowState) -> Option<ID2D1HwndRenderTarget>` in `src/screensaver.rs`

- [ ] **Step 1: Add `perf` module to `src/main.rs`**

```rust
#[cfg(windows)]
pub mod perf {
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    static START: AtomicI64 = AtomicI64::new(0);
    static LOGGED: AtomicBool = AtomicBool::new(false);

    pub fn mark_start() {
        let mut t = 0i64;
        unsafe { let _ = QueryPerformanceCounter(&mut t); }
        START.store(t, Ordering::Relaxed);
    }

    /// Called after every EndDraw; only the first call logs.
    pub fn log_first_frame() {
        if LOGGED.swap(true, Ordering::Relaxed) {
            return;
        }
        let (mut now, mut freq) = (0i64, 0i64);
        unsafe {
            let _ = QueryPerformanceCounter(&mut now);
            let _ = QueryPerformanceFrequency(&mut freq);
        }
        let ms = (now - START.load(Ordering::Relaxed)) as f64 * 1000.0 / freq as f64;
        let line = format!("flipsaver: first frame in {ms:.1} ms");
        let wide: Vec<u16> = line.encode_utf16().chain([0]).collect();
        unsafe {
            windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(
                windows::core::PCWSTR(wide.as_ptr()),
            );
        }
        // FLIPSAVER_LOG holds a file path; append so repeated runs accumulate.
        if let Ok(path) = std::env::var("FLIPSAVER_LOG") {
            use std::io::Write;
            if let Ok(mut f) =
                std::fs::OpenOptions::new().create(true).append(true).open(path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}
```

In `main()` (windows variant), add `perf::mark_start();` as the **first statement**, before the DPI/COM calls.

- [ ] **Step 2: Per-window render target in `src/screensaver.rs`**

Add to the `use` block: `use windows::Win32::Graphics::Direct2D::Common::*;` and `use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;`.

Add field to `WindowState`:

```rust
    pub target: Option<ID2D1HwndRenderTarget>,
```

(initialize with `target: None` at the `run_fullscreen` construction site; the preview site arrives in Task 10). Add:

```rust
unsafe fn ensure_target(hwnd: HWND, state: &mut WindowState) -> Option<ID2D1HwndRenderTarget> {
    if state.target.is_none() {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        // Target stays at 96 DPI so 1 DIP == 1 physical pixel: with
        // per-monitor-V2 the window is sized in physical pixels and all
        // layout math is pixel-based, so per-monitor DPI is honored
        // through geometry, not through D2D unit scaling.
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U {
                width: (rc.right - rc.left) as u32,
                height: (rc.bottom - rc.top) as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        state.target = state.gfx.d2d.CreateHwndRenderTarget(&props, &hwnd_props).ok();
    }
    state.target.clone()
}
```

- [ ] **Step 3: Paint handler (blank clear for now)**

Add two arms to `wndproc` (before the `_ =>` arm):

```rust
        WM_ERASEBKGND => LRESULT(1), // D2D owns the surface; avoid GDI flicker
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            if let Some(rt) = ensure_target(hwnd, state) {
                rt.BeginDraw();
                rt.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }));
                let end = rt.EndDraw(None, None);
                crate::perf::log_first_frame();
                let _ = end; // recreate-on-loss handled in Task 9
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
```

Also change the window class registration: `hbrBackground: HBRUSH::default(),` (no GDI background brush now that WM_ERASEBKGND/WM_PAINT own the surface).

- [ ] **Step 4: Verify**

Run: `cargo test` → `29 passed`.
Run: `cargo xwin build --release --target x86_64-pc-windows-msvc` → `Finished`.

- [ ] **Step 5: Manual (Windows), deferred to Task 13 matrix**

`set FLIPSAVER_LOG=%TEMP%\flipsaver.log & flipsaver.scr /s` → log file gains one `first frame in N ms` line per run; DebugView shows the same string.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/screensaver.rs
git commit -m "screensaver: render target, blank frame, startup timing"
```

---

### Task 8: Clock face drawing

**Files:**
- Modify: `src/clock.rs` (cfg-gated `draw` submodule)
- Modify: `src/screensaver.rs` (face cache, timer, real paint)
- Test: existing layout unit tests keep passing; cross build gate; fidelity is Manual (Windows)

**Interfaces:**
- Consumes: `Layout`/`compute`/`format_time`/`Marker` (Task 4), `Gfx` (Task 6), `ensure_target` (Task 7).
- Produces:
  - `clock::draw::FaceCache` (per-window, device-dependent: brushes + text formats + layout) with `pub fn new(rt, gfx, width, height, settings, is_preview) -> windows::core::Result<FaceCache>`
  - `pub fn draw_face(rt: &ID2D1HwndRenderTarget, cache: &FaceCache, hour: u32, minute: u32) -> windows::core::Result<()>`
  - `WindowState` gains `pub face: Option<clock::draw::FaceCache>` and `pub last_minute: u32`

Colors (spec): box gradient `#121212` top → `#0A0A0A` bottom, digits `#B7B7B7`, split line black, background black.

- [ ] **Step 1: Add the draw submodule to `src/clock.rs`**

```rust
#[cfg(windows)]
pub mod draw {
    use super::{compute, format_time, Layout, Marker, Rect};
    use crate::screensaver::Gfx;
    use crate::settings::Settings;
    use windows::core::*;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::DirectWrite::*;

    fn color(rgb: u32) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
            g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
            b: (rgb & 0xFF) as f32 / 255.0,
            a: 1.0,
        }
    }

    fn rectf(r: Rect) -> D2D_RECT_F {
        D2D_RECT_F {
            left: r.x as f32,
            top: r.y as f32,
            right: (r.x + r.w) as f32,
            bottom: (r.y + r.h) as f32,
        }
    }

    pub struct FaceCache {
        pub layout: Layout,
        pub is_24h: bool,
        digits: ID2D1SolidColorBrush,
        black: ID2D1SolidColorBrush,
        gradient: ID2D1GradientStopCollection,
        large_format: IDWriteTextFormat,
        small_format: IDWriteTextFormat,
        dwrite: IDWriteFactory5,
    }

    impl FaceCache {
        pub fn new(
            rt: &ID2D1HwndRenderTarget,
            gfx: &Gfx,
            width: i32,
            height: i32,
            settings: Settings,
            is_preview: bool,
        ) -> Result<FaceCache> {
            unsafe {
                let layout =
                    compute(width, height, settings.scale, settings.display_24hr, is_preview);
                let digits = rt.CreateSolidColorBrush(&color(0xB7B7B7), None)?;
                let black = rt.CreateSolidColorBrush(&color(0x000000), None)?;
                let stops = [
                    D2D1_GRADIENT_STOP { position: 0.0, color: color(0x121212) },
                    D2D1_GRADIENT_STOP { position: 1.0, color: color(0x0A0A0A) },
                ];
                let gradient = rt.CreateGradientStopCollection(
                    &stops,
                    D2D1_GAMMA_2_2,
                    D2D1_EXTEND_MODE_CLAMP,
                )?;
                let mk_format = |px: i32| -> Result<IDWriteTextFormat> {
                    let f = gfx.dwrite.CreateTextFormat(
                        &HSTRING::from(gfx.family),
                        gfx.fonts.as_ref().map(|c| c.cast::<IDWriteFontCollection>()).transpose()?.as_ref(),
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        px as f32,
                        w!("en-us"),
                    )?;
                    Ok(f)
                };
                let large_format = mk_format(layout.large_font_px)?;
                // Digits center in the (already offset) text rect, matching
                // the original's StringFormat Center/Center.
                large_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                large_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                let small_format = mk_format(layout.small_font_px)?;
                Ok(FaceCache {
                    layout,
                    is_24h: settings.display_24hr,
                    digits,
                    black,
                    gradient,
                    large_format,
                    small_format,
                    dwrite: gfx.dwrite.clone(),
                })
            }
        }

        unsafe fn draw_box(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &super::BoxLayout,
            text: &str,
            marker: Option<Marker>,
        ) -> Result<()> {
            let r = rectf(bl.rect);
            // Vertical gradient per box (original: LinearGradientMode.Vertical).
            let brush = rt.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: windows_numerics::Vector2 { X: r.left, Y: r.top },
                    endPoint: windows_numerics::Vector2 { X: r.left, Y: r.bottom },
                },
                None,
                &self.gradient,
            )?;
            let rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: self.layout.corner_radius as f32,
                radiusY: self.layout.corner_radius as f32,
            };
            rt.FillRoundedRectangle(&rounded, &brush);

            let wide: Vec<u16> = text.encode_utf16().collect();
            let tl = self.dwrite.CreateTextLayout(
                &wide,
                &self.large_format,
                bl.text.w as f32,
                bl.text.h as f32,
            )?;
            rt.DrawTextLayout(
                windows_numerics::Vector2 { X: bl.text.x as f32, Y: bl.text.y as f32 },
                &tl,
                &self.digits,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );

            if let Some(m) = marker {
                let s: Vec<u16> = (if m == Marker::Am { "AM" } else { "PM" }).encode_utf16().collect();
                let ml = self.dwrite.CreateTextLayout(&s, &self.small_format, 4096.0, 4096.0)?;
                match m {
                    Marker::Am => {
                        let (x, y) = bl.marker_top.unwrap();
                        rt.DrawTextLayout(
                            windows_numerics::Vector2 { X: x as f32, Y: y as f32 },
                            &ml,
                            &self.digits,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                    Marker::Pm => {
                        // Anchor is the text bottom edge (original subtracts
                        // Font.Height); use the measured line height.
                        let mut metrics = DWRITE_TEXT_METRICS::default();
                        ml.GetMetrics(&mut metrics)?;
                        let (x, y_bottom) = bl.marker_bottom.unwrap();
                        rt.DrawTextLayout(
                            windows_numerics::Vector2 {
                                X: x as f32,
                                Y: y_bottom as f32 - metrics.height,
                            },
                            &ml,
                            &self.digits,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                }
            }

            // Split line per box, never spanning the face.
            rt.DrawLine(
                windows_numerics::Vector2 { X: r.left, Y: bl.split_y as f32 },
                windows_numerics::Vector2 { X: r.right, Y: bl.split_y as f32 },
                &self.black,
                self.layout.split_stroke,
                None,
            );
            Ok(())
        }
    }

    pub fn draw_face(
        rt: &ID2D1HwndRenderTarget,
        cache: &FaceCache,
        hour: u32,
        minute: u32,
    ) -> Result<()> {
        unsafe {
            rt.Clear(Some(&color(0x000000)));
            let (h, m, marker) = format_time(hour, minute, cache.is_24h);
            // One marker only: AM top corner before noon, PM bottom after.
            cache.draw_box(rt, &cache.layout.hours, &h, marker)?;
            cache.draw_box(rt, &cache.layout.minutes, &m, None)?;
            Ok(())
        }
    }
}
```

Note: `Vector2`/`Matrix3x2` live in the `windows-numerics` companion crate for recent `windows` versions (add `windows-numerics = "0.2"` under `[target.'cfg(windows)'.dependencies]` if not re-exported); older versions re-export from `windows::Foundation::Numerics`. Adapt the path, keep the geometry.

- [ ] **Step 2: Wire cache + timer + real paint in `src/screensaver.rs`**

Add fields to `WindowState` (init `face: None, last_minute: 61` at the `run_fullscreen` construction site; 61 is an impossible minute so the first paint always draws):

```rust
    pub face: Option<crate::clock::draw::FaceCache>,
    pub last_minute: u32,
```

Add a local-time helper:

```rust
fn local_hm() -> (u32, u32) {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (st.wHour as u32, st.wMinute as u32)
}
```

In `create_saver_window`, after `CreateWindowExW` succeeds: `SetTimer(hwnd, 1, 1000, None);` (1000 ms, both modes — the preview minute must tick live).

Replace the `WM_PAINT` arm's draw section:

```rust
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            if let Some(rt) = ensure_target(hwnd, state) {
                if state.face.is_none() {
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    state.face = crate::clock::draw::FaceCache::new(
                        &rt,
                        &state.gfx,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        state.settings,
                        state.is_preview,
                    )
                    .ok();
                }
                rt.BeginDraw();
                let draw_ok = match &state.face {
                    Some(face) => {
                        let (h, m) = local_hm();
                        state.last_minute = m;
                        crate::clock::draw::draw_face(&rt, face, h, m).is_ok()
                    }
                    None => {
                        rt.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }));
                        true
                    }
                };
                let _ = draw_ok;
                let end = rt.EndDraw(None, None);
                crate::perf::log_first_frame();
                let _ = end; // Task 9 inspects this for device loss
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
```

Add a `WM_TIMER` arm:

```rust
        WM_TIMER => {
            // Redraw only when the minute changes (no seconds in v0.1).
            let (_, m) = local_hm();
            if m != state.last_minute {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
```

- [ ] **Step 3: Verify**

Run: `cargo test` → `29 passed`.
Run: `cargo xwin build --release --target x86_64-pc-windows-msvc` → `Finished`.

- [ ] **Step 4: Manual (Windows), deferred to Task 13 matrix**

`/s` shows two rounded gradient boxes with Oswald digits, split line per box, AM/PM marker in 12h mode; minute rolls over without flicker.

- [ ] **Step 5: Commit**

```bash
git add src/clock.rs src/screensaver.rs Cargo.toml Cargo.lock
git commit -m "clock: D2D face drawing with minute timer"
```

---

### Task 9: Device-loss recovery + WM_DPICHANGED

**Files:**
- Modify: `src/screensaver.rs`
- Test: cross build gate; behavior is Manual (Windows)

**Interfaces:**
- Consumes: `WM_PAINT` arm and `WindowState.{target,face}` (Tasks 7–8).
- Produces: per-window recovery — no new public items.

- [ ] **Step 1: Handle `D2DERR_RECREATE_TARGET` in the `WM_PAINT` arm**

Add `use windows::Win32::Graphics::Direct2D::D2DERR_RECREATE_TARGET;` and replace the two `let end = rt.EndDraw(None, None); ... let _ = end;` lines with:

```rust
                if let Err(e) = rt.EndDraw(None, None) {
                    if e.code() == D2DERR_RECREATE_TARGET {
                        // Device lost (driver reset, remote session, ...):
                        // this window rebuilds its own target and
                        // device-dependent resources; others are untouched.
                        state.target = None;
                        state.face = None;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                crate::perf::log_first_frame();
```

(`perf::log_first_frame` stays after `EndDraw` — a lost first frame still marks the timing point.)

- [ ] **Step 2: Handle `WM_DPICHANGED`**

Add an arm to `wndproc`:

```rust
        WM_DPICHANGED => {
            // Layout is derived from physical pixel geometry; a DPI change
            // can change that geometry, so drop this window's caches and
            // repaint. Mixed-DPI is otherwise handled by per-monitor-V2
            // physical sizing (target stays at 96 DPI, see ensure_target).
            state.target = None;
            state.face = None;
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
```

- [ ] **Step 3: Verify**

Run: `cargo test` → `29 passed`.
Run: `cargo xwin build --release --target x86_64-pc-windows-msvc` → `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/screensaver.rs
git commit -m "screensaver: device-loss and DPI-change recovery"
```

---

### Task 10: /p preview mode

**Files:**
- Modify: `src/screensaver.rs` (add `run_preview`)
- Modify: `src/main.rs` (wire `Mode::Preview(Some(_))`)
- Test: cross build gate; control-panel behavior is Manual (Windows)

**Interfaces:**
- Consumes: `create_saver_window`, `WindowState`, `Gfx` (Tasks 5–8); `Mode::Preview` (Task 2).
- Produces: `pub fn run_preview(settings: Settings, parent: isize)` in `src/screensaver.rs`.

- [ ] **Step 1: Add `run_preview` to `src/screensaver.rs`**

```rust
pub fn run_preview(settings: Settings, parent: isize) {
    unsafe {
        let parent = HWND(parent as *mut core::ffi::c_void);
        // Declared deviation: bogus hwnd exits 0 silently, no error UI.
        if !IsWindow(Some(parent)).as_bool() {
            return;
        }
        let mut rc = RECT::default();
        if GetClientRect(parent, &mut rc).is_err() {
            return;
        }
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        register_class(instance);
        let gfx = match Gfx::new() {
            Ok(g) => std::rc::Rc::new(g),
            Err(_) => return,
        };
        // Same draw path as /s, including the 1 s timer, so the preview
        // minute stays live. Input-exit is disabled via is_preview; the
        // control panel terminates the process by destroying the parent.
        create_saver_window(
            instance,
            WS_CHILD,
            WINDOW_EX_STYLE::default(),
            Some(parent),
            rc,
            WindowState {
                is_preview: true,
                mouse: None,
                settings,
                gfx,
                target: None,
                face: None,
                last_minute: 61,
            },
        );
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
```

Also make the message loop end when the preview window dies: in the `WM_DESTROY` arm, quit for preview windows (fullscreen windows only quit on input):

```rust
        WM_DESTROY => {
            let was_preview = state.is_preview;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(state_ptr));
            if was_preview {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
```

- [ ] **Step 2: Wire the dispatch in `src/main.rs`**

```rust
        Mode::Preview(Some(parent)) => screensaver::run_preview(settings, parent),
```

- [ ] **Step 3: Verify**

Run: `cargo test` → `29 passed`.
Run: `cargo xwin build --release --target x86_64-pc-windows-msvc` → `Finished`.

- [ ] **Step 4: Manual (Windows), deferred to Task 13 matrix**

Screensaver control panel shows the live miniature clock; hairline split; minute ticks over; `flipsaver.scr /p 99999999` exits immediately with code 0 and no UI.

- [ ] **Step 5: Commit**

```bash
git add src/screensaver.rs src/main.rs
git commit -m "preview: child-window mode with live minute timer"
```

---

### Task 11: /c config dialog (in-code DLGTEMPLATE)

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` (cfg-gated `mod config;`, wire `Mode::Config`)
- Test: cross build gate; round-trip is Manual (Windows)

**Interfaces:**
- Consumes: `settings::{Settings, load, save, default_path}` (Task 3); `Mode::Config` (Task 2).
- Produces: `pub fn run_config()` in `src/config.rs`. Only this mode ever writes settings.

The dialog template is built as a `Vec<u16>` at runtime (`DialogBoxIndirectParamW`) — no `.rc` file, no resource compiler in the cross build. DLGTEMPLATE wire format: header, then each DLGITEMTEMPLATE aligned to a DWORD boundary; classes are `0xFFFF` + atom (`0x0080` button, `0x0082` static) or a NUL-terminated class-name string (trackbar).

- [ ] **Step 1: Write `src/config.rs`**

```rust
//! /c mode: minimal settings dialog from an in-code DLGTEMPLATE.

use crate::settings::{self, Settings};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const IDC_12H: i32 = 101;
const IDC_24H: i32 = 102;
const IDC_SCALE: i32 = 103;

struct DlgBuilder {
    words: Vec<u16>,
}

impl DlgBuilder {
    fn new(title: &str, cx: i16, cy: i16, item_count: u16) -> DlgBuilder {
        let mut b = DlgBuilder { words: Vec::new() };
        let style: u32 = (DS_SETFONT as u32)
            | (DS_MODALFRAME as u32)
            | WS_POPUP.0
            | WS_CAPTION.0
            | WS_SYSMENU.0;
        b.dword(style);
        b.dword(0); // dwExtendedStyle
        b.word(item_count);
        b.word(0); // x
        b.word(0); // y
        b.word(cx as u16);
        b.word(cy as u16);
        b.word(0); // no menu
        b.word(0); // default dialog class
        b.wstr(title);
        b.word(8); // font point size (DS_SETFONT)
        b.wstr("MS Shell Dlg");
        b
    }

    fn word(&mut self, w: u16) {
        self.words.push(w);
    }

    fn dword(&mut self, d: u32) {
        self.word((d & 0xFFFF) as u16);
        self.word((d >> 16) as u16);
    }

    fn wstr(&mut self, s: &str) {
        self.words.extend(s.encode_utf16());
        self.word(0);
    }

    fn align_dword(&mut self) {
        if self.words.len() % 2 == 1 {
            self.word(0);
        }
    }

    fn item_atom(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16, atom: u16, text: &str) {
        self.item_header(style, x, y, cx, cy, id);
        self.word(0xFFFF);
        self.word(atom);
        self.wstr(text);
        self.word(0); // no creation data
    }

    fn item_class(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16, class: &str, text: &str) {
        self.item_header(style, x, y, cx, cy, id);
        self.wstr(class);
        self.wstr(text);
        self.word(0);
    }

    fn item_header(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16) {
        self.align_dword();
        self.dword(style | WS_CHILD.0 | WS_VISIBLE.0);
        self.dword(0); // exstyle
        self.word(x as u16);
        self.word(y as u16);
        self.word(cx as u16);
        self.word(cy as u16);
        self.word(id);
    }
}

fn build_template() -> Vec<u16> {
    let mut b = DlgBuilder::new("FlipSaver Settings", 175, 92, 7);
    b.item_atom(0, 7, 9, 45, 8, 0, 0x0082, "Time format:"); // STATIC
    b.item_atom(
        BS_AUTORADIOBUTTON as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        60, 7, 45, 10, IDC_12H as u16, 0x0080, "12 hour",
    );
    b.item_atom(BS_AUTORADIOBUTTON as u32, 115, 7, 45, 10, IDC_24H as u16, 0x0080, "24 hour");
    b.item_atom(0, 7, 32, 45, 8, 0, 0x0082, "Size:");
    b.item_class(
        (TBS_AUTOTICKS | TBS_HORZ) as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        58, 29, 110, 15, IDC_SCALE as u16, "msctls_trackbar32", "",
    );
    b.item_atom(
        BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        63, 71, 50, 14, IDOK.0 as u16, 0x0080, "OK",
    );
    b.item_atom(BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 118, 71, 50, 14, IDCANCEL.0 as u16, 0x0080, "Cancel");
    b.words
}

unsafe extern "system" fn dlgproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            let s = &*(lp.0 as *const Settings);
            let _ = CheckRadioButton(hwnd, IDC_12H, IDC_24H, if s.display_24hr { IDC_24H } else { IDC_12H });
            // Slider is 0..10; INI stores slider x 10 (0..100), like FlipIt.
            let _ = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_SETRANGE, WPARAM(1), LPARAM(10 << 16));
            let _ = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_SETPOS, WPARAM(1), LPARAM((s.scale / 10) as isize));
            1
        }
        WM_COMMAND => match (wp.0 & 0xFFFF) as i32 {
            id if id == IDOK.0 => {
                let pos = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                let s = Settings {
                    display_24hr: IsDlgButtonChecked(hwnd, IDC_24H) == 1,
                    scale: pos * 10,
                };
                let _ = settings::save(&settings::default_path(), s);
                let _ = EndDialog(hwnd, 1);
                1
            }
            id if id == IDCANCEL.0 => {
                let _ = EndDialog(hwnd, 0);
                1
            }
            _ => 0,
        },
        _ => 0,
    }
}

pub fn run_config() {
    unsafe {
        // Trackbar class lives in comctl32; v6 activation comes from the
        // embedded manifest (Task 12).
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
        let settings = settings::load(&settings::default_path());
        let template = build_template();
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let _ = DialogBoxIndirectParamW(
            Some(instance),
            template.as_ptr() as *const DLGTEMPLATE,
            None,
            Some(dlgproc),
            LPARAM(&settings as *const Settings as isize),
        );
    }
}
```

- [ ] **Step 2: Wire the dispatch in `src/main.rs`**

Add `#[cfg(windows)] mod config;` next to the other cfg-gated module, and:

```rust
        Mode::Config => config::run_config(),
```

- [ ] **Step 3: Verify**

Run: `cargo test` → `29 passed`.
Run: `cargo xwin build --release --target x86_64-pc-windows-msvc` → `Finished`.

- [ ] **Step 4: Manual (Windows), deferred to Task 13 matrix**

`flipsaver.scr` (no args) opens the dialog; toggling 24h + moving the slider then OK writes `%LOCALAPPDATA%\flipsaver\Settings.ini` with `Display24Hr=1` and the new `Scale`; Cancel writes nothing; reopening shows persisted values; `/s` reflects them.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "config: settings dialog from in-code DLGTEMPLATE"
```

---

### Task 12: --version, build.rs, manifest embedding

**Files:**
- Create: `build.rs`
- Modify: `Cargo.toml` (build-dependency), `src/main.rs` (wire `Mode::Version`)
- Test: cross build gate + `strings` check on the binary

**Interfaces:**
- Consumes: `Mode::Version` (Task 2).
- Produces: env vars `FLIPSAVER_VERSION_TAG` / `FLIPSAVER_GIT_SHA` baked at build time; embedded application manifest (per-monitor-V2 DPI + comctl32 v6).

- [ ] **Step 1: Add the build dependency to `Cargo.toml`**

```toml
[build-dependencies]
embed-manifest = "1.4"
```

(Plain `[build-dependencies]`, not target-gated: `cfg()` tables for build-deps are evaluated against the *target*, which gets confusing — the crate compiles everywhere and build.rs gates on `CARGO_CFG_WINDOWS` instead.)

- [ ] **Step 2: Create `build.rs`**

```rust
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    // Version-line convention: tag "dev" for untagged/head builds.
    let tag = git(&["describe", "--tags", "--exact-match", "HEAD"]).unwrap_or_else(|| "dev".into());
    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FLIPSAVER_VERSION_TAG={tag}");
    println!("cargo:rustc-env=FLIPSAVER_GIT_SHA={sha}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");

    // Manifest (PMv2 DPI awareness + comctl32 v6 for the trackbar) via a
    // linker resource; SetProcessDpiAwarenessContext at startup is the
    // functional backstop if this ever fights cargo-xwin.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        embed_manifest::embed_manifest(embed_manifest::new_manifest("flipsaver"))
            .expect("embed manifest");
    }
}
```

(`embed_manifest::new_manifest` defaults already include the comctl32 v6 dependency and per-monitor-V2 DPI awareness — verify against the crate docs for the resolved version; if the default DPI setting differs, chain the builder's DPI-awareness setter for PerMonitorV2.)

- [ ] **Step 3: Wire `Mode::Version` in `src/main.rs`**

```rust
#[cfg(windows)]
fn print_version() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // GUI-subsystem binary: borrow the parent console so the line is
    // visible when run from a terminal.
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    println!(
        "Version: {} ({}), built for: windows-x86_64",
        env!("FLIPSAVER_VERSION_TAG"),
        env!("FLIPSAVER_GIT_SHA")
    );
}
```

and in the dispatch: `Mode::Version => print_version(),`.

- [ ] **Step 4: Verify**

Run: `cargo test`
Expected: `29 passed`.

Run: `cargo xwin build --release --target x86_64-pc-windows-msvc`
Expected: `Finished`.

Run: `strings target/x86_64-pc-windows-msvc/release/flipsaver.exe | grep -m1 -i 'PerMonitorV2'`
Expected: one matching line (manifest embedded).

Run: `strings target/x86_64-pc-windows-msvc/release/flipsaver.exe | grep -m1 'built for: windows-x86_64'`
Expected: one matching line.

- [ ] **Step 5: Manual (Windows), deferred to Task 13 matrix**

`flipsaver.scr --version` from cmd prints `Version: dev (<sha>), built for: windows-x86_64`.

- [ ] **Step 6: Commit**

```bash
git add build.rs Cargo.toml Cargo.lock src/main.rs
git commit -m "version: build metadata and embedded manifest"
```

---

### Task 13: Docs, deploy script, manual test matrix, CHANGELOG

**Files:**
- Create: `scripts/deploy.sh`, `docs/manual-test-matrix.md`, `README.md`, `CHANGELOG.md`
- Test: `bash -n` on the script; content review

**Interfaces:**
- Consumes: everything shipped in Tasks 1–12.
- Produces: the v0.1 release collateral. The Windows-side checks collected as **Manual** in earlier tasks live in `docs/manual-test-matrix.md`; running that matrix and the cold-start measurement is the human release gate for v0.1 (spec Milestone 5).

- [ ] **Step 1: Create `scripts/deploy.sh`**

```bash
#!/usr/bin/env bash
# Cross-build and drop the .scr where Windows can run it.
set -euo pipefail
DEST="${1:-/mnt/c/Temp}"
cd "$(dirname "$0")/.."
cargo xwin build --release --target x86_64-pc-windows-msvc
mkdir -p "$DEST"
cp target/x86_64-pc-windows-msvc/release/flipsaver.exe "$DEST/flipsaver.scr"
echo "deployed: $DEST/flipsaver.scr"
```

Run: `chmod +x scripts/deploy.sh && bash -n scripts/deploy.sh`
Expected: no output (syntax OK).

- [ ] **Step 2: Create `docs/manual-test-matrix.md`**

```markdown
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
```

- [ ] **Step 3: Create `README.md`**

```markdown
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
```

- [ ] **Step 4: Create `CHANGELOG.md`**

```markdown
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
```

- [ ] **Step 5: Verify repo state**

Run: `cargo test`
Expected: `29 passed`.

Run: `test -x scripts/deploy.sh && ls README.md CHANGELOG.md docs/manual-test-matrix.md docs/BUILDING.md assets/OFL.txt`
Expected: all files listed, no error.

- [ ] **Step 6: Commit**

```bash
git add scripts/deploy.sh docs/manual-test-matrix.md README.md CHANGELOG.md
git commit -m "docs: release collateral and manual test matrix"
```
