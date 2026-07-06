# Implementation plan — Vertical clock layout + per-monitor orientation

Date: 2026-07-06
Spec: `docs/superpowers/specs/2026-07-06-vertical-clock-design.md`

Adds a vertical flip-clock layout and lets each monitor pick Auto /
Horizontal / Vertical, keyed by device name in the INI. Layout math and
settings stay pure and host-testable; the Win32 pieces (monitor identity,
dialog rows) verify via `cargo check --target x86_64-pc-windows-msvc`.

## Verification model

- **Host tests** (`cargo test`) compile only non-`#[cfg(windows)]` code —
  they fully exercise `compute()` and the settings parser/serializer.
- **Windows compile** (`cargo check --target x86_64-pc-windows-msvc`) type-
  checks the Direct2D / dialog code without linking. Cross-toolchain is
  already present (`scripts/deploy.sh` uses `cargo xwin`).
- The non-`Copy` change to `Settings` ripples through the Win32 modules;
  the windows build only goes green once Task 3 lands (Task 3 carries a
  one-line stopgap in `config.rs`, replaced wholesale by Task 4).

## Team Roster

| Task | Implementer | Spec Reviewer | Quality Reviewer | Depends On |
|---|---|---|---|---|
| Task 1: Settings — Orientation + per-screen map | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` | — |
| Task 2: Layout math — vertical `compute()` | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` | — |
| Task 3: Monitor identity + orientation wiring | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` | Task 1, Task 2 |
| Task 4: Settings dialog — per-monitor rows | `voltagent-lang:cpp-pro` | `voltagent-lang:cpp-pro` | `voltagent-qa-sec:code-reviewer` | Task 3 |
| Task 5: Docs — changelog + manual matrix | `voltagent-biz:technical-writer` | `generic` | `generic` | — |

---

## Task 1: Settings — Orientation enum + per-screen map

**Files:** Modify `src/settings.rs`.

Pure, host-testable. Adds the `Orientation` enum, gives `Settings` a
`screens: BTreeMap<String, Orientation>` (dropping `Copy`), parses and
serializes `[Screen <name>]` sections, and adds the single orientation
resolution point `effective_orientation`.

### Step 1 — write the tests first

Append these to the `mod tests` block in `src/settings.rs`:

```rust
    #[test]
    fn parses_screen_section() {
        let s = Settings::from_ini_text(
            "[General]\nScale=70\n[Screen DISPLAY1]\nOrientation=2\n",
        );
        assert_eq!(s.screens.get("DISPLAY1"), Some(&Orientation::Vertical));
    }

    #[test]
    fn screen_garbage_orientation_is_auto() {
        let s = Settings::from_ini_text("[Screen DISPLAY1]\nOrientation=abc\n");
        assert_eq!(s.screens.get("DISPLAY1"), Some(&Orientation::Auto));
        let s = Settings::from_ini_text("[Screen DISPLAY1]\nOrientation=9\n");
        assert_eq!(s.screens.get("DISPLAY1"), Some(&Orientation::Auto));
    }

    #[test]
    fn auto_screens_are_omitted_on_save() {
        let mut s = Settings::default();
        s.screens.insert("DISPLAY1".into(), Orientation::Auto);
        s.screens.insert("DISPLAY2".into(), Orientation::Vertical);
        let text = s.to_ini_text();
        assert!(!text.contains("[Screen DISPLAY1]"));
        assert!(text.contains("[Screen DISPLAY2]\r\nOrientation=2"));
    }

    #[test]
    fn screen_round_trip_preserves_explicit() {
        let mut s = Settings {
            display_24hr: true,
            scale: 40,
            screens: std::collections::BTreeMap::new(),
        };
        s.screens.insert("DISPLAY1".into(), Orientation::Horizontal);
        // A monitor not currently attached must survive a save.
        s.screens.insert("DISPLAY9".into(), Orientation::Vertical);
        assert_eq!(Settings::from_ini_text(&s.to_ini_text()), s);
    }

    #[test]
    fn effective_orientation_explicit_wins() {
        let mut s = Settings::default();
        s.screens.insert("DISPLAY1".into(), Orientation::Vertical);
        assert!(s.effective_orientation("DISPLAY1", 1920, 1080));
        s.screens.insert("DISPLAY1".into(), Orientation::Horizontal);
        assert!(!s.effective_orientation("DISPLAY1", 1080, 1920));
    }

    #[test]
    fn effective_orientation_auto_by_aspect() {
        let s = Settings::default();
        assert!(s.effective_orientation("DISPLAY1", 1080, 1920));
        assert!(!s.effective_orientation("DISPLAY1", 1920, 1080));
    }
```

Then fix the two existing literals that now need the `screens` field —
in `round_trip` and `save_creates_directory_and_loads_back`:

```rust
        let s = Settings {
            display_24hr: true,
            scale: 90,
            screens: std::collections::BTreeMap::new(),
        };
```

```rust
        let s = Settings {
            display_24hr: true,
            scale: 20,
            screens: std::collections::BTreeMap::new(),
        };
```

And the `save` call in that last test becomes `save(&path, &s)` (the
signature changes below).

### Step 2 — implement

Top of `src/settings.rs`, after the existing `use`:

```rust
use std::collections::BTreeMap;
```

Add the enum after `DEFAULT_SCALE`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Auto,
    Horizontal,
    Vertical,
}

impl Orientation {
    /// Anything outside {1,2} is Auto — consistent with the lenient parser.
    pub fn from_ini(v: i32) -> Orientation {
        match v {
            1 => Orientation::Horizontal,
            2 => Orientation::Vertical,
            _ => Orientation::Auto,
        }
    }

    pub fn to_ini(self) -> i32 {
        match self {
            Orientation::Auto => 0,
            Orientation::Horizontal => 1,
            Orientation::Vertical => 2,
        }
    }
}
```

Replace the `Settings` struct and its `Default` (note: `Copy` is gone,
`Clone` stays):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub display_24hr: bool,
    pub scale: i32, // 0..=100, slider value x 10
    pub screens: BTreeMap<String, Orientation>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { display_24hr: false, scale: DEFAULT_SCALE, screens: BTreeMap::new() }
    }
}
```

In `from_ini_text`, extend the section dispatch. Replace the
`if section == "General" { ... }` block's closing with an added arm:

```rust
            if section == "General" {
                match key {
                    "Display24Hr" => {
                        s.display_24hr = value.trim().parse::<i32>().map(|v| v == 1).unwrap_or(false)
                    }
                    "Scale" => s.scale = value.trim().parse().unwrap_or(DEFAULT_SCALE),
                    _ => {}
                }
            } else if let Some(name) = section.strip_prefix("Screen ") {
                if key == "Orientation" {
                    let v = value.trim().parse::<i32>().unwrap_or(0);
                    s.screens.insert(name.to_string(), Orientation::from_ini(v));
                }
            }
```

Replace `to_ini_text` — Auto entries are skipped, so omission round-trips
back to Auto and the file stays free of noise sections:

```rust
    pub fn to_ini_text(&self) -> String {
        let mut out = format!(
            "[General]\r\nDisplay24Hr={}\r\nScale={}\r\n\r\n",
            if self.display_24hr { 1 } else { 0 },
            self.scale
        );
        for (name, orient) in &self.screens {
            if *orient == Orientation::Auto {
                continue;
            }
            out.push_str(&format!(
                "[Screen {}]\r\nOrientation={}\r\n\r\n",
                name,
                orient.to_ini()
            ));
        }
        out
    }
```

Add the resolution method inside `impl Settings` (after `to_ini_text`):

```rust
    /// Single orientation resolution point. Explicit setting wins; Auto or
    /// an absent monitor falls back to aspect (portrait → vertical).
    /// Returns `true` for vertical.
    pub fn effective_orientation(&self, device: &str, width: i32, height: i32) -> bool {
        match self.screens.get(device) {
            Some(Orientation::Horizontal) => false,
            Some(Orientation::Vertical) => true,
            _ => height > width,
        }
    }
```

Change `save` to borrow (callers updated in later tasks):

```rust
pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, s.to_ini_text())
}
```

### Step 3 — verify

```bash
cargo test 2>&1 | tail -3
```

Expected: `test result: ok.` with the six new tests among the count (41
tests total).

### Step 4 — commit

```bash
git add src/settings.rs
git commit -m "settings: add per-monitor orientation to INI schema

Introduce Orientation { Auto, Horizontal, Vertical } and a
screens: BTreeMap<String, Orientation> on Settings, parsed from and
serialized to [Screen <name>] sections. Auto entries are never written,
so omission already parses back as Auto and absent monitors' explicit
overrides survive a save. effective_orientation is the single resolution
point: explicit wins, Auto resolves by aspect.

Settings loses Copy (BTreeMap is not Copy); ripple to the Win32 modules
follows in later tasks.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 2: Layout math — vertical `compute()`

**Files:** Modify `src/clock.rs`.

`compute()` gains a `vertical: bool`. Vertical swaps the box counts
(1 across, 2 down) and stacks hours over minutes; everything else — split
line, fonts, corner radius, rect-relative AM/PM markers — is unchanged and
transposes for free. Also updates `FaceCache::new` (Win32-only) to take
`&Settings` plus a resolved `vertical` flag.

### Step 1 — update existing tests, add the vertical test

In `mod tests`, add the new positional argument (`false`) to the three
`compute()` calls:

- `layout_1080p_scale70_fullscreen_12h`: `compute(1920, 1080, 70, false, false, false)`
- `layout_24h_has_no_markers`: `compute(1920, 1080, 70, true, false, false)`
- `layout_preview_320x240_scale70`: `compute(320, 240, 70, false, false, true)`

Add the new vertical geometry test (values are the mirror of the existing
1920x1080 landscape case, verified by hand):

```rust
    #[test]
    fn layout_portrait_1080x1920_scale70_vertical() {
        let l = compute(1080, 1920, 70, false, true, false);
        assert_eq!(l.hours.rect, Rect { x: 184, y: 230, w: 712, h: 712 });
        assert_eq!(l.minutes.rect, Rect { x: 184, y: 978, w: 712, h: 712 });
        assert_eq!(l.corner_radius, 35);
        assert_eq!(l.large_font_px, 605);
        assert_eq!(l.small_font_px, 64);
        assert_eq!(l.hours.text, Rect { x: 120, y: 258, w: 854, h: 712 });
        assert_eq!(l.hours.split_y, 584);
        assert_eq!(l.hours.marker_top, Some((219, 301)));
        assert_eq!(l.hours.marker_bottom, Some((219, 871)));
        // markers only ever on the hours box
        assert_eq!(l.minutes.marker_top, None);
    }
```

### Step 2 — implement `compute`

Replace the whole `compute` function body:

```rust
pub fn compute(
    width: i32,
    height: i32,
    scale_percent: i32,
    is_24h: bool,
    vertical: bool,
    is_preview: bool,
) -> Layout {
    let bp = border_percent(scale_percent);
    let (hours_rect, minutes_rect, box_size) = if vertical {
        // One box across, two stacked; separation applied on the Y axis.
        let box_size = calc_box_size(width, bp, 1).min(calc_box_size(height, bp, 2));
        let sep = (box_size as f64 * BOX_SEPARATION_PERCENT).round() as i32;
        let start_x = calc_offset(width, 1, box_size, 0);
        let start_y = calc_offset(height, 2, box_size, sep);
        let hours = Rect { x: start_x, y: start_y, w: box_size, h: box_size };
        let minutes = Rect { y: start_y + box_size + sep, ..hours };
        (hours, minutes, box_size)
    } else {
        let box_size = calc_box_size(width, bp, 2).min(calc_box_size(height, bp, 1));
        let sep = (box_size as f64 * BOX_SEPARATION_PERCENT).round() as i32;
        let start_x = calc_offset(width, 2, box_size, sep);
        let start_y = calc_offset(height, 1, box_size, 0);
        let hours = Rect { x: start_x, y: start_y, w: box_size, h: box_size };
        let minutes = Rect { x: start_x + box_size + sep, ..hours };
        (hours, minutes, box_size)
    };
    Layout {
        hours: box_layout(hours_rect, !is_24h, is_preview),
        minutes: box_layout(minutes_rect, false, is_preview),
        corner_radius: box_size / 20,
        large_font_px: box_size * 85 / 100,
        small_font_px: box_size * 9 / 100,
        split_stroke: if is_preview { 1.0 } else { SPLIT_WIDTH as f32 },
    }
}
```

### Step 3 — update `FaceCache::new` (Win32 draw module)

Change the signature to borrow `Settings` and accept the resolved flag,
and thread `vertical` into `compute`. Replace the `pub fn new` signature
and its first two statements:

```rust
        pub fn new(
            rt: &ID2D1HwndRenderTarget,
            gfx: &Gfx,
            width: i32,
            height: i32,
            settings: &Settings,
            vertical: bool,
            is_preview: bool,
        ) -> Result<FaceCache> {
            unsafe {
                let layout = compute(
                    width,
                    height,
                    settings.scale,
                    settings.display_24hr,
                    vertical,
                    is_preview,
                );
```

(The `is_24h: settings.display_24hr` field init later in the function is
unchanged — reading through the shared reference still compiles.)

### Step 4 — verify

```bash
cargo test 2>&1 | tail -3
```

Expected: `test result: ok.` including `layout_portrait_1080x1920_scale70_vertical`.

The `FaceCache::new` change is `#[cfg(windows)]`, so the host build does
not compile it; the windows build is not green until Task 3 rewires the
caller. That is expected at this point.

### Step 5 — commit

```bash
git add src/clock.rs
git commit -m "clock: add vertical layout to compute()

compute() gains a vertical flag that swaps the box counts (one across,
two stacked) and places the hours box above minutes. Split line, fonts,
corner radius and the rect-relative AM/PM markers are unchanged and
transpose without special-casing. FaceCache::new now borrows Settings and
takes the resolved orientation.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 3: Monitor identity + orientation wiring

**Files:** Modify `src/screensaver.rs`; one-line stopgap in `src/config.rs`
(replaced by Task 4).

Enumerate monitors with their device names, thread the name through
`WindowState`, and resolve orientation via `effective_orientation` at
paint time. The stopgap keeps `config.rs` compiling against the non-`Copy`
`Settings` so the windows build goes green.

Depends on Task 1 (`effective_orientation`, non-`Copy` `Settings`) and
Task 2 (`FaceCache::new` signature).

### Step 1 — enumerate device names

Replace `enumerate_monitors` (make it `pub` — Task 4 calls it):

```rust
pub fn enumerate_monitors() -> Vec<(RECT, String)> {
    unsafe extern "system" fn enum_proc(
        mon: HMONITOR,
        _hdc: HDC,
        rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let v = &mut *(lparam.0 as *mut Vec<(RECT, String)>);
        let mut info = MONITORINFOEXW::default();
        // szDevice is only populated when cbSize covers the Ex struct.
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(mon, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
            let name = String::from_utf16_lossy(&info.szDevice);
            let name = name.trim_end_matches('\0').trim_start_matches(r"\\.\").to_string();
            v.push((info.monitorInfo.rcMonitor, name));
        } else {
            // Failure → empty device name → Auto resolution downstream.
            v.push((*rect, String::new()));
        }
        TRUE
    }
    let mut v: Vec<(RECT, String)> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(enum_proc), LPARAM(&mut v as *mut _ as isize));
    }
    v
}
```

`HMONITOR`, `MONITORINFO`, `MONITORINFOEXW`, `GetMonitorInfoW` all come
from the existing `use ...Graphics::Gdi::*;` glob.

### Step 2 — carry the device name on `WindowState`

Add a field to the struct:

```rust
pub struct WindowState {
    pub is_preview: bool,
    pub mouse: Option<(i32, i32)>,
    pub settings: Settings,
    pub gfx: std::rc::Rc<Gfx>,
    pub target: Option<ID2D1HwndRenderTarget>,
    pub face: Option<crate::clock::draw::FaceCache>,
    pub last_minute: u32,
    pub device: String,
}
```

### Step 3 — resolve orientation in WM_PAINT

Replace the `if state.face.is_none() { ... }` block inside `WM_PAINT`:

```rust
                if state.face.is_none() {
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    let w = rc.right - rc.left;
                    let h = rc.bottom - rc.top;
                    let vertical = state.settings.effective_orientation(&state.device, w, h);
                    state.face = crate::clock::draw::FaceCache::new(
                        &rt,
                        &state.gfx,
                        w,
                        h,
                        &state.settings,
                        vertical,
                        state.is_preview,
                    )
                    .ok();
                }
```

### Step 4 — populate `device` at window creation

In `run_fullscreen`, replace the enumerate loop:

```rust
        for (bounds, device) in enumerate_monitors() {
            create_saver_window(
                instance,
                WS_POPUP,
                WS_EX_TOPMOST,
                None,
                bounds,
                WindowState {
                    is_preview: false,
                    mouse: None,
                    settings: settings.clone(),
                    gfx: gfx.clone(),
                    target: None,
                    face: None,
                    // Impossible minute so the first paint always draws.
                    last_minute: 61,
                    device,
                },
            );
        }
```

In `run_preview`, add the empty device (map miss → Auto → landscape
preview → horizontal) to the `WindowState` literal:

```rust
            WindowState {
                is_preview: true,
                mouse: None,
                settings,
                gfx,
                target: None,
                face: None,
                last_minute: 61,
                device: String::new(),
            },
```

### Step 5 — stopgap `config.rs` (replaced in Task 4)

The non-`Copy` change broke the OK handler's `Settings` literal and the
`save` call. Replace the `id if id == IDOK.0 =>` arm in `dlgproc` with a
minimal compiling version that preserves any existing screen sections:

```rust
            id if id == IDOK.0 => {
                let pos = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                let mut s = settings::load(&settings::default_path());
                s.display_24hr = IsDlgButtonChecked(hwnd, IDC_24H) == 1;
                s.scale = pos * 10;
                let _ = settings::save(&settings::default_path(), &s);
                let _ = EndDialog(hwnd, 1);
                1
            }
```

### Step 6 — verify

```bash
cargo check --target x86_64-pc-windows-msvc 2>&1 | tail -1
cargo test 2>&1 | tail -3
```

Expected: `Finished` from the windows check, and `test result: ok.` on
host (unchanged count from Task 1/2).

### Step 7 — commit

```bash
git add src/screensaver.rs src/config.rs
git commit -m "screensaver: resolve per-monitor orientation from device name

enumerate_monitors now returns each monitor's device name (GetMonitorInfoW
with a MONITORINFOEXW buffer, \\\\.\\ prefix stripped). WindowState carries
the name and WM_PAINT resolves orientation through
Settings::effective_orientation using the window client dims, so fullscreen
and preview share one resolution path. Preview passes an empty name and
lands on Auto.

config.rs OK handler is kept compiling against the non-Copy Settings; the
full per-monitor dialog lands next.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 4: Settings dialog — per-monitor rows

**Files:** Modify `src/config.rs`.

Enumerate monitors once in `run_config`, feed the same list to
`build_template` and the dialog proc, and add one label + three radios per
monitor. The OK handler mutates the original `screens` map (present
monitors only, so absent ones are preserved) and saves.

Depends on Task 3 (`enumerate_monitors` is now `pub` and returns names).

### Step 1 — imports and context type

Change the top `use` for settings and add the `RECT`/`DWLP_USER` needs
(`RECT` already arrives via `Foundation::*`; `DWLP_USER`,
`SetWindowLongPtrW`, `GetWindowLongPtrW` via `WindowsAndMessaging::*`):

```rust
use crate::settings::{self, Orientation, Settings};
```

Add a context alias near the top of the file (after the `const` block):

```rust
// (mutable settings, monitors as (rect, device name)) — lives for the
// dialog's lifetime, pointer stashed in DWLP_USER.
type DlgCtx = (Settings, Vec<(RECT, String)>);
```

### Step 2 — dynamic template with monitor rows

Replace `build_template`:

```rust
fn build_template(font_name: &str, monitors: &[(RECT, String)]) -> Vec<u16> {
    let row_h: i16 = 14;
    let rows = monitors.len();
    let item_count = (9 + rows * 4) as u16;
    let cy = 92 + rows as i16 * row_h;
    let mut b = DlgBuilder::new("FlipSaver Settings", 260, cy, item_count);
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
    b.item_atom(0, 7, 52, 45, 8, 0, 0x0082, "Font:");
    b.item_atom(0, 60, 52, 108, 8, 0, 0x0082, font_name);

    // One row per monitor: label + Auto/Horizontal/Vertical radios. Each
    // row's first radio carries WS_GROUP so the rows are independent radio
    // groups; the OK button's WS_GROUP closes the last row.
    for (row, (rect, _device)) in monitors.iter().enumerate() {
        let y = 64 + row as i16 * row_h;
        let base = 200 + row as u16 * 4;
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        let label = format!("Screen {} ({}x{}):", row + 1, w, h);
        b.item_atom(0, 7, y + 1, 100, 8, 0, 0x0082, &label);
        b.item_atom(
            BS_AUTORADIOBUTTON as u32 | WS_TABSTOP.0 | WS_GROUP.0,
            110, y, 38, 10, base, 0x0080, "Auto",
        );
        b.item_atom(BS_AUTORADIOBUTTON as u32, 150, y, 52, 10, base + 1, 0x0080, "Horizontal");
        b.item_atom(BS_AUTORADIOBUTTON as u32, 205, y, 45, 10, base + 2, 0x0080, "Vertical");
    }

    let by = 71 + rows as i16 * row_h;
    b.item_atom(
        BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        63, by, 50, 14, IDOK.0 as u16, 0x0080, "OK",
    );
    b.item_atom(BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 118, by, 50, 14, IDCANCEL.0 as u16, 0x0080, "Cancel");
    b.words
}
```

### Step 3 — dialog proc reads/writes the context

Replace the whole `dlgproc`:

```rust
unsafe extern "system" fn dlgproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            // lp is &mut DlgCtx; stash it so WM_COMMAND can reach it.
            SetWindowLongPtrW(hwnd, DWLP_USER, lp.0);
            let ctx = &*(lp.0 as *const DlgCtx);
            let (settings, monitors) = ctx;
            let _ = CheckRadioButton(
                hwnd, IDC_12H, IDC_24H,
                if settings.display_24hr { IDC_24H } else { IDC_12H },
            );
            let _ = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_SETRANGE, WPARAM(1), LPARAM(10 << 16));
            let _ = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_SETPOS, WPARAM(1), LPARAM((settings.scale / 10) as isize));
            for (row, (_rect, device)) in monitors.iter().enumerate() {
                let base = 200 + row as i32 * 4;
                let orient = settings.screens.get(device).copied().unwrap_or(Orientation::Auto);
                let checked = base + match orient {
                    Orientation::Auto => 0,
                    Orientation::Horizontal => 1,
                    Orientation::Vertical => 2,
                };
                let _ = CheckRadioButton(hwnd, base, base + 2, checked);
            }
            1
        }
        WM_COMMAND => match (wp.0 & 0xFFFF) as i32 {
            id if id == IDOK.0 => {
                let ctx = &mut *(GetWindowLongPtrW(hwnd, DWLP_USER) as *mut DlgCtx);
                let pos = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                ctx.0.display_24hr = IsDlgButtonChecked(hwnd, IDC_24H) == 1;
                ctx.0.scale = pos * 10;
                // Only present monitors are touched; absent sections in the
                // map are left as-is and preserved on save.
                for (row, (_rect, device)) in ctx.1.iter().enumerate() {
                    let base = 200 + row as i32 * 4;
                    let orient = if IsDlgButtonChecked(hwnd, base + 1) == 1 {
                        Orientation::Horizontal
                    } else if IsDlgButtonChecked(hwnd, base + 2) == 1 {
                        Orientation::Vertical
                    } else {
                        Orientation::Auto
                    };
                    ctx.0.screens.insert(device.clone(), orient);
                }
                let _ = settings::save(&settings::default_path(), &ctx.0);
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
```

### Step 4 — enumerate once in `run_config`

Replace `run_config`:

```rust
pub fn run_config() {
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
        let settings = settings::load(&settings::default_path());
        // One enumeration feeds both the template and the proc, so the
        // row→device mapping cannot desync.
        let monitors = crate::screensaver::enumerate_monitors();
        let template = build_template(crate::screensaver::font_display_name(), &monitors);
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let mut ctx: Box<DlgCtx> = Box::new((settings, monitors));
        let _ = DialogBoxIndirectParamW(
            Some(instance),
            template.as_ptr() as *const DLGTEMPLATE,
            None,
            Some(dlgproc),
            LPARAM(&mut *ctx as *mut DlgCtx as isize),
        );
        // ctx dropped here, after the modal dialog returns.
    }
}
```

If `DWLP_USER` is typed as `WINDOW_LONG_PTR_INDEX` in the crate and the
call does not type-check bare, pass it as-is first; only wrap if the
compiler complains.

### Step 5 — verify

```bash
cargo check --target x86_64-pc-windows-msvc 2>&1 | tail -1
cargo test 2>&1 | tail -3
```

Expected: `Finished` from the windows check; host `test result: ok.`
unchanged.

### Step 6 — commit

```bash
git add src/config.rs
git commit -m "config: add per-monitor orientation rows to settings dialog

The dialog enumerates monitors once and emits one label + three radios
(Auto/Horizontal/Vertical) per monitor, each row an independent radio
group via WS_GROUP on its first radio. The item count and dialog height
grow with the monitor count. A boxed (Settings, monitors) context is
passed through lparam and stashed in DWLP_USER so OK mutates the original
screens map — present monitors only, absent sections preserved — then
saves.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 5: Docs — changelog + manual test matrix

**Files:** Modify `CHANGELOG.md`, `docs/manual-test-matrix.md`.

### Step 1 — changelog

Under `## [Unreleased]` add an `### Added` block:

```markdown
## [Unreleased]

### Added

- **Vertical clock layout** (hours above minutes), chosen automatically on
  portrait monitors.
- **Per-monitor orientation** in the settings dialog: Auto / Horizontal /
  Vertical, stored per display in `Settings.ini` under `[Screen <name>]`.
```

### Step 2 — manual test matrix

Append rows to the Functional table in `docs/manual-test-matrix.md`:

```markdown
| 11 | Portrait monitor with no `[Screen]` section renders vertical by default; landscape unchanged | |
| 12 | `/c`: one row per monitor; setting Vertical on a landscape monitor then `/s` renders vertical | |
| 13 | Mixed multi-monitor: one Horizontal + one Vertical override both honored simultaneously | |
| 14 | Orientation persists per monitor across restarts; `Settings.ini` gains `[Screen <name>]` only for non-Auto | |
| 15 | Undock a monitor, save from `/c`, redock: the undocked monitor's override survived | |
| 16 | Old `Settings.ini` (no `[Screen]` sections) → all monitors Auto, landscape identical to v0.1 | |
```

### Step 3 — verify

```bash
cargo test 2>&1 | tail -3
```

Expected: `test result: ok.` (docs-only change; sanity check the tree
still builds).

### Step 4 — commit

```bash
git add CHANGELOG.md docs/manual-test-matrix.md
git commit -m "docs: note vertical layout and per-monitor orientation

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```
