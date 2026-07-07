//! /s mode: one topmost popup per monitor, shared message loop,
//! exit on first real input.

use crate::settings::Settings;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS_NAME: PCWSTR = w!("flipsaverwnd");

static OSWALD_BOLD: &[u8] = include_bytes!("../assets/Oswald-Bold.ttf");

/// Resolved font: system Helvetica LT Std Cond when installed, else the
/// embedded Oswald collection. Weight is always Bold; stretch differs
/// (the typographic "Helvetica LT Std" family needs Condensed).
pub struct FontChoice {
    /// None means system font collection (or last-resort Segoe UI).
    pub collection: Option<IDWriteFontCollection1>,
    pub family: &'static str,
    pub stretch: DWRITE_FONT_STRETCH,
}

/// Process-wide device-independent graphics resources, shared by every
/// window (fullscreen and preview) via Rc.
pub struct Gfx {
    pub d2d: ID2D1Factory,
    pub dwrite: IDWriteFactory5,
    pub font: FontChoice,
}

impl Gfx {
    pub fn new() -> Result<Gfx> {
        unsafe {
            let d2d: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let font = Self::pick_font(&dwrite);
            debug_log(&format!("flipsaver: font: {}", font.family));
            Ok(Gfx { d2d, dwrite, font })
        }
    }

    /// Prefer the licensed Helvetica when installed on the system (probe by
    /// family name only — the font itself never ships with the binary);
    /// otherwise load the embedded Oswald.
    unsafe fn pick_font(dwrite: &IDWriteFactory5) -> FontChoice {
        if let Some(c) = probe_system_font(dwrite) {
            let stretch = if c.condensed {
                DWRITE_FONT_STRETCH_CONDENSED
            } else {
                DWRITE_FONT_STRETCH_NORMAL
            };
            return FontChoice { collection: None, family: c.family, stretch };
        }
        match Self::load_embedded_font(dwrite) {
            Ok(fonts) => FontChoice {
                collection: Some(fonts),
                family: "Oswald",
                stretch: DWRITE_FONT_STRETCH_NORMAL,
            },
            Err(e) => {
                // Embedded bytes failing to load is a build defect, not
                // a runtime condition: assert in debug, degrade in release.
                debug_assert!(false, "embedded font load failed: {e:?}");
                FontChoice {
                    collection: None,
                    family: "Segoe UI",
                    stretch: DWRITE_FONT_STRETCH_NORMAL,
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

/// First preferred family present in the system collection, by name only.
unsafe fn probe_system_font(dwrite: &IDWriteFactory5) -> Option<&'static crate::fontsel::Candidate> {
    let mut sys: Option<IDWriteFontCollection1> = None;
    // No downloadable fonts: only locally installed families count.
    dwrite.GetSystemFontCollection(false, &mut sys, false).ok()?;
    let sys = sys?;
    crate::fontsel::pick(|family| {
        let name = HSTRING::from(family);
        let (mut index, mut exists) = (0u32, BOOL::default());
        sys.FindFamilyName(&name, &mut index, &mut exists).is_ok() && exists.as_bool()
    })
}

/// Font the saver will render with, for display in the /c dialog.
/// Same probe as Gfx::new, without loading the Oswald collection.
pub fn font_display_name() -> &'static str {
    unsafe {
        let dwrite: Result<IDWriteFactory5> = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED);
        match dwrite {
            Ok(d) => probe_system_font(&d).map(|c| c.family).unwrap_or("Oswald (embedded)"),
            Err(_) => "Oswald (embedded)",
        }
    }
}

pub fn debug_log(line: &str) {
    let wide: Vec<u16> = line.encode_utf16().chain([0]).collect();
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(PCWSTR(wide.as_ptr()));
    }
}

/// A box's in-flight flip: old value folding to the new one since start_ms.
pub struct Anim {
    pub from: u32,
    pub to: u32,
    pub start_ms: u64,
}

/// A cell's in-flight flip: old glyph folding to new since start_ms.
pub struct CellAnim {
    pub from: char,
    pub to: char,
    pub start_ms: u64,
}

/// A board cell's rendered glyph, with room for an in-flight flip.
pub struct CellState {
    pub glyph: char,
    pub anim: Option<CellAnim>,
}

/// Per-window render mode, decided once at creation.
pub enum Mode {
    Clock {
        cache: Option<crate::clock::draw::FaceCache>,
        /// (61,61) is the not-yet-primed sentinel.
        shown: (u32, u32),
        hours_anim: Option<Anim>,
        minutes_anim: Option<Anim>,
    },
    Board {
        zones: Vec<crate::tz::Zone>,
        cache: Option<crate::board::draw::BoardCache>,
        cells: Vec<CellState>,
    },
}

pub struct WindowState {
    pub is_preview: bool,
    pub mouse: Option<(i32, i32)>,
    pub settings: Settings,
    pub gfx: std::rc::Rc<Gfx>,
    pub target: Option<ID2D1HwndRenderTarget>,
    pub flip_enabled: bool,
    pub device: String,
    pub mode: Mode,
}

/// Machine-local full SYSTEMTIME (for hour/minute, date-differs and the UTC
/// used by tz conversion).
fn local_now() -> SYSTEMTIME {
    unsafe { windows::Win32::System::SystemInformation::GetLocalTime() }
}

/// Current row-major glyph grid for a board, from its resolved zones. One
/// GetSystemTime feeds every zone. Unresolved zones render as `--:--`.
fn board_cells(zones: &[crate::tz::Zone], grid: &crate::board::Grid, is_24h: bool) -> Vec<char> {
    let utc = unsafe { windows::Win32::System::SystemInformation::GetSystemTime() };
    let now = local_now();
    let mut cells: Vec<char> = Vec::with_capacity(grid.rows * grid.cols);
    for zone in zones.iter().take(grid.rows) {
        let parts = zone
            .info
            .as_ref()
            .and_then(|info| crate::tz::zone_time(info, &utc, &now));
        let row = crate::board::format_row(&zone.label, parts, is_24h);
        // format_row already returns exactly grid.cols chars.
        cells.extend(row);
    }
    cells
}

/// Choose the render mode for a window. World mode needs at least one
/// resolvable zone; otherwise (empty list or all invalid) fall back to the
/// clock, decided once here.
fn make_mode(settings: &Settings, device: &str) -> Mode {
    use crate::settings::Mode as SettingsMode;
    if settings.screen_mode(device) == SettingsMode::World {
        let zones = crate::tz::resolve_all(&settings.world_clocks);
        if zones.iter().any(|z| z.info.is_some()) {
            return Mode::Board { zones, cache: None, cells: Vec::new() };
        }
        debug_log("flipsaver: no resolvable zones, falling back to clock");
    }
    Mode::Clock { cache: None, shown: (61, 61), hours_anim: None, minutes_anim: None }
}

/// Drop device-dependent caches and abandon any in-flight animation, keeping
/// the mode and (for a board) its resolved zones.
fn reset_caches(state: &mut WindowState) {
    match &mut state.mode {
        Mode::Clock { cache, hours_anim, minutes_anim, .. } => {
            *cache = None;
            *hours_anim = None;
            *minutes_anim = None;
        }
        Mode::Board { cache, cells, .. } => {
            *cache = None;
            cells.clear();
        }
    }
}

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

unsafe fn register_class(instance: HINSTANCE) {
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        hbrBackground: HBRUSH::default(),
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
    let hwnd = CreateWindowExW(
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
    .unwrap_or_default();
    if hwnd.0 != std::ptr::null_mut() {
        // 1000 ms in both modes: the preview minute must also tick live.
        SetTimer(Some(hwnd), 1, 1000, None);
    }
    hwnd
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
            let was_preview = state.is_preview;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(state_ptr));
            if was_preview {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1), // D2D owns the surface; avoid GDI flicker
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            if let Some(rt) = ensure_target(hwnd, state) {
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let w = rc.right - rc.left;
                let h = rc.bottom - rc.top;
                rt.BeginDraw();
                let now = windows::Win32::System::SystemInformation::GetTickCount64();
                let is_24h = state.settings.display_24hr;
                match &mut state.mode {
                    Mode::Clock { cache, shown, hours_anim, minutes_anim } => {
                        if cache.is_none() {
                            let vertical = state.settings.effective_orientation(&state.device, w, h);
                            *cache = crate::clock::draw::FaceCache::new(
                                &rt, &state.gfx, w, h, &state.settings, vertical, state.is_preview,
                            )
                            .ok();
                        }
                        match cache {
                            Some(face) => {
                                if *shown == (61, 61) {
                                    let st = local_now();
                                    *shown = (st.wHour as u32, st.wMinute as u32);
                                }
                                let _ = crate::clock::draw::draw_face(
                                    &rt, face, *shown, hours_anim.as_ref(), minutes_anim.as_ref(), now,
                                );
                            }
                            None => {
                                rt.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }));
                            }
                        }
                    }
                    Mode::Board { zones, cache, cells } => {
                        if cache.is_none() {
                            let grid = crate::board::compute_grid(
                                w, h, state.settings.scale, zones.len(), is_24h,
                            );
                            *cache = crate::board::draw::BoardCache::new(&rt, &state.gfx, grid).ok();
                            if let Some(bc) = cache {
                                let glyphs = board_cells(zones, &bc.grid, is_24h);
                                *cells = glyphs
                                    .into_iter()
                                    .map(|g| CellState { glyph: g, anim: None })
                                    .collect();
                            }
                        }
                        match cache {
                            Some(bc) => {
                                let _ = crate::board::draw::draw_board(&rt, bc, cells, now);
                            }
                            None => {
                                rt.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }));
                            }
                        }
                    }
                }
                if let Err(e) = rt.EndDraw(None, None) {
                    if e.code() == D2DERR_RECREATE_TARGET {
                        // Device lost: drop the target and this window's cache;
                        // next paint rebuilds. Also abandon any half-played fold.
                        state.target = None;
                        reset_caches(state);
                        let _ = KillTimer(Some(hwnd), 2);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                crate::perf::log_first_frame();
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            // Layout is derived from physical pixel geometry; a DPI change
            // can change that geometry, so drop this window's caches and
            // repaint. Mixed-DPI is otherwise handled by per-monitor-V2
            // physical sizing (target stays at 96 DPI, see ensure_target).
            state.target = None;
            reset_caches(state);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_TIMER => {
            let now = windows::Win32::System::SystemInformation::GetTickCount64();
            match wp.0 {
                1 => {
                    match &mut state.mode {
                        Mode::Clock { shown, hours_anim, minutes_anim, .. } => {
                            let st = local_now();
                            let (h, m) = (st.wHour as u32, st.wMinute as u32);
                            let primed = *shown != (61, 61);
                            let mut started = false;
                            if primed && h != shown.0 && state.flip_enabled {
                                *hours_anim = Some(Anim { from: shown.0, to: h, start_ms: now });
                                started = true;
                            }
                            if primed && m != shown.1 && state.flip_enabled {
                                *minutes_anim = Some(Anim { from: shown.1, to: m, start_ms: now });
                                started = true;
                            }
                            if (h, m) != *shown {
                                *shown = (h, m);
                                if started {
                                    SetTimer(Some(hwnd), 2, 16, None);
                                }
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                        }
                        Mode::Board { zones, cache, cells } => {
                            if let Some(bc) = cache {
                                let is_24h = state.settings.display_24hr;
                                let next = board_cells(zones, &bc.grid, is_24h);
                                let old: Vec<char> = cells.iter().map(|c| c.glyph).collect();
                                let changed = crate::board::diff_cells(&old, &next);
                                if !changed.is_empty() {
                                    if state.flip_enabled {
                                        for &i in &changed {
                                            cells[i].anim = Some(CellAnim {
                                                from: cells[i].glyph,
                                                to: next[i],
                                                start_ms: now,
                                            });
                                        }
                                        SetTimer(Some(hwnd), 2, 16, None);
                                    }
                                    // Settle the logical glyph regardless; a
                                    // disabled flip just snaps on next paint.
                                    for &i in &changed {
                                        cells[i].glyph = next[i];
                                    }
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                }
                            }
                        }
                    }
                    LRESULT(0)
                }
                2 => {
                    // Fast tick: advance the fold; retire finished anims.
                    let mut any_active = false;
                    match &mut state.mode {
                        Mode::Clock { hours_anim, minutes_anim, .. } => {
                            let done = |a: &Option<Anim>| {
                                a.as_ref().map_or(true, |x| {
                                    now.saturating_sub(x.start_ms) as f64 / crate::clock::FLIP_MS >= 1.0
                                })
                            };
                            if done(hours_anim) {
                                *hours_anim = None;
                            }
                            if done(minutes_anim) {
                                *minutes_anim = None;
                            }
                            any_active = hours_anim.is_some() || minutes_anim.is_some();
                        }
                        Mode::Board { cells, .. } => {
                            for c in cells.iter_mut() {
                                if let Some(a) = &c.anim {
                                    if now.saturating_sub(a.start_ms) as f64 / crate::clock::FLIP_MS >= 1.0 {
                                        c.anim = None;
                                    } else {
                                        any_active = true;
                                    }
                                }
                            }
                        }
                    }
                    if !any_active {
                        let _ = KillTimer(Some(hwnd), 2);
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    LRESULT(0)
                }
                _ => LRESULT(0),
            }
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run_fullscreen(settings: Settings) {
    let gfx = match Gfx::new() {
        Ok(g) => std::rc::Rc::new(g),
        Err(_) => return, // no D2D at all: nothing sane to render, exit quietly
    };
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        register_class(instance);
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
                    flip_enabled: settings.flip_animation,
                    settings: settings.clone(),
                    gfx: gfx.clone(),
                    target: None,
                    mode: make_mode(&settings, &device),
                    device,
                },
            );
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

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
        let flip_enabled = settings.flip_animation;
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
                flip_enabled,
                device: String::new(),
                mode: Mode::Clock { cache: None, shown: (61, 61), hours_anim: None, minutes_anim: None },
            },
        );
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
