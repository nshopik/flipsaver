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
pub enum Command {
    Screensaver,
    /// Owner window from `/c:HWND`, when given.
    Config(Option<isize>),
    Preview(Option<isize>),
    Version,
}

pub fn parse_args(args: &[String]) -> Command {
    let Some(first_raw) = args.first() else {
        return Command::Config(None);
    };
    if first_raw == "--version" || first_raw == "-V" {
        return Command::Version;
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
    let hwnd = || second.as_deref().and_then(|s| s.parse::<i64>().ok()).map(|v| v as isize);
    match first.as_str() {
        "/s" => Command::Screensaver,
        "/p" => Command::Preview(hwnd()),
        "/c" => Command::Config(hwnd()),
        // Unknown args open config too (declared deviation: original
        // shows an error MessageBox for unknown args).
        _ => Command::Config(None),
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
        Command::Screensaver => screensaver::run_fullscreen(settings()),
        Command::Preview(Some(parent)) => screensaver::run_preview(settings(), parent),
        Command::Preview(None) => {}          // declared deviation: exit 0 silently
        Command::Config(owner) => config::run_config(owner),
        Command::Version => print_version(),
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
            (&[][..], Command::Config(None)),
            (&["/s"][..], Command::Screensaver),
            (&["/S"][..], Command::Screensaver),
            (&["/c"][..], Command::Config(None)),
            (&["/c:12345"][..], Command::Config(Some(12345))),
            (&["/c", "12345"][..], Command::Config(Some(12345))),
            (&["/p", "1234"][..], Command::Preview(Some(1234))),
            (&["/p:1234"][..], Command::Preview(Some(1234))),
            (&["/p_1234"][..], Command::Preview(Some(1234))),
            (&["/p"][..], Command::Preview(None)),
            (&["/p", "abc"][..], Command::Preview(None)),
            (&["/p:"][..], Command::Preview(None)),
            (&["/foo"][..], Command::Config(None)),
            // Non-ASCII must not panic on a char boundary.
            (&["/ü"][..], Command::Config(None)),
            (&["/é1"][..], Command::Config(None)),
            (&["/sé"][..], Command::Screensaver),
            (&["/p€1234"][..], Command::Preview(Some(1234))),
            (&["--version"][..], Command::Version),
            (&["-V"][..], Command::Version),
        ] {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            assert_eq!(parse_args(&args), want, "{args:?}");
        }
    }
}
