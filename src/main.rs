#![cfg_attr(windows, windows_subsystem = "windows")]

mod clock;
mod board;
mod settings;

#[cfg(windows)]
mod screensaver;

#[cfg(windows)]
mod config;

#[cfg(windows)]
mod tz;

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
    let lower = first_raw.to_lowercase();
    let lower = lower.trim();
    let mut chars = lower.chars();
    let first: String = chars.by_ref().take(2).collect();
    let second = if lower.chars().count() > 2 {
        // Original discards the char at index 2 unchecked, so any
        // separator works: /p:1234, /p_1234, ...
        Some(chars.skip(1).collect::<String>().trim().to_string())
    } else {
        args.get(1).cloned()
    };
    match first.as_str() {
        "/s" => Mode::Screensaver,
        "/p" => Mode::Preview(second.and_then(|s| s.parse::<i64>().ok()).map(|v| v as isize)),
        // "/c" and unknown args both open config (declared deviation:
        // original shows an error MessageBox for unknown args).
        _ => Mode::Config,
    }
}

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
    let settings = || settings::load_or_default(&settings::default_path());
    match parse_args(&args) {
        Mode::Screensaver => screensaver::run_fullscreen(settings()),
        Mode::Preview(Some(parent)) => screensaver::run_preview(settings(), parent),
        Mode::Preview(None) => {}          // declared deviation: exit 0 silently
        Mode::Config => config::run_config(),
        Mode::Version => print_version(),
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("flipsaver targets Windows; this host build exists for `cargo test` only.");
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn parse_args_table() {
        for (args, want) in [
            (&[][..], Mode::Config),
            (&["/s"][..], Mode::Screensaver),
            (&["/S"][..], Mode::Screensaver),
            (&["/c"][..], Mode::Config),
            (&["/c:12345"][..], Mode::Config),
            (&["/p", "1234"][..], Mode::Preview(Some(1234))),
            (&["/p:1234"][..], Mode::Preview(Some(1234))),
            (&["/p_1234"][..], Mode::Preview(Some(1234))),
            (&["/p"][..], Mode::Preview(None)),
            (&["/p", "abc"][..], Mode::Preview(None)),
            (&["/p:"][..], Mode::Preview(None)),
            (&["/foo"][..], Mode::Config),
            // Non-ASCII must not panic on a char boundary.
            (&["/ü"][..], Mode::Config),
            (&["/é1"][..], Mode::Config),
            (&["/sé"][..], Mode::Screensaver),
            (&["/p€1234"][..], Mode::Preview(Some(1234))),
            (&["--version"][..], Mode::Version),
            (&["-V"][..], Mode::Version),
        ] {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            assert_eq!(parse_args(&args), want, "{args:?}");
        }
    }
}
