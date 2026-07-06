use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFAULT_SCALE: i32 = 70;

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
            } else if let Some(name) = section.strip_prefix("Screen ") {
                if key == "Orientation" {
                    let v = value.trim().parse::<i32>().unwrap_or(0);
                    s.screens.insert(name.to_string(), Orientation::from_ini(v));
                }
            }
        }
        s
    }

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
}

pub fn load(path: &Path) -> Settings {
    match std::fs::read_to_string(path) {
        Ok(text) => Settings::from_ini_text(&text),
        Err(_) => Settings::default(),
    }
}

pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
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
        let s = Settings {
            display_24hr: true,
            scale: 90,
            screens: std::collections::BTreeMap::new(),
        };
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
        let s = Settings {
            display_24hr: true,
            scale: 20,
            screens: std::collections::BTreeMap::new(),
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

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
}
