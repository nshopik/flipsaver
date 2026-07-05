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
