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

pub struct WindowState {
    pub is_preview: bool,
    pub mouse: Option<(i32, i32)>,
    pub settings: Settings,
    pub gfx: std::rc::Rc<Gfx>,
    pub target: Option<ID2D1HwndRenderTarget>,
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
        for bounds in enumerate_monitors() {
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
