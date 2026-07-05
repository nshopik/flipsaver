#![cfg_attr(windows, windows_subsystem = "windows")]

mod clock;
mod settings;

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

fn main() {}

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
