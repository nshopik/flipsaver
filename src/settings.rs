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
