#![cfg_attr(windows, windows_subsystem = "windows")]

mod clock;
mod settings;

#[cfg(windows)]
mod screensaver;

#[cfg(windows)]
pub mod perf {
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

    static START: AtomicI64 = AtomicI64::new(0);
    static LOGGED: AtomicBool = AtomicBool::new(false);

    pub fn mark_start() {
        let mut t = 0i64;
        unsafe {
            let _ = QueryPerformanceCounter(&mut t);
        }
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
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

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

#[cfg(windows)]
fn main() {
    perf::mark_start();
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
        Mode::Preview(Some(parent)) => screensaver::run_preview(settings, parent),
        Mode::Preview(None) => {}          // declared deviation: exit 0 silently
        Mode::Config => {}                 // wired in Task 11
        Mode::Version => {}                // wired in Task 12
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("flipsaver targets Windows; this host build exists for `cargo test` only.");
}

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
