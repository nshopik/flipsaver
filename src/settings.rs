use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFAULT_SCALE: i32 = 70;
const DEFAULT_BOARD_SCALE: i32 = 100;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Clock,
    World,
}

impl Mode {
    /// Anything other than 1 is Clock — lenient, like Orientation::from_ini.
    pub fn from_ini(v: i32) -> Mode {
        match v {
            1 => Mode::World,
            _ => Mode::Clock,
        }
    }

    pub fn to_ini(self) -> i32 {
        match self {
            Mode::Clock => 0,
            Mode::World => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenSettings {
    pub orientation: Orientation,
    pub mode: Mode,
}

impl Default for ScreenSettings {
    fn default() -> Self {
        ScreenSettings { orientation: Orientation::Auto, mode: Mode::Clock }
    }
}

// Labels cap at the board's cell budget so the parser and the renderer
// can never disagree on width.
use crate::board::LABEL_CELLS;

fn default_world_clocks() -> Vec<(String, String)> {
    [
        ("Los Angeles", "Pacific Standard Time"),
        ("New York", "Eastern Standard Time"),
        ("London", "GMT Standard Time"),
        ("Dubai", "Arabian Standard Time"),
        ("Tokyo", "Tokyo Standard Time"),
        ("Sydney", "AUS Eastern Standard Time"),
    ]
    .iter()
    .map(|(c, z)| (c.to_string(), z.to_string()))
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub display_24hr: bool,
    pub scale: i32, // 0..=100, slider value x 10
    pub board_scale: i32, // world-board size, same encoding as scale
    pub flip_animation: bool,
    pub screens: BTreeMap<String, ScreenSettings>,
    /// City -> Windows timezone key. A Vec (not a map): row order is the
    /// user's chosen display order.
    pub world_clocks: Vec<(String, String)>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            display_24hr: false,
            scale: DEFAULT_SCALE,
            board_scale: DEFAULT_BOARD_SCALE,
            flip_animation: true,
            screens: BTreeMap::new(),
            world_clocks: default_world_clocks(),
        }
    }
}

impl Settings {
    pub fn from_ini_text(text: &str) -> Settings {
        let mut s = Settings::default();
        let mut section = String::new();
        // The preloaded defaults are cleared the first time a [WorldClocks]
        // header appears, so a present-but-empty section = "no cities".
        let mut world_cleared = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].to_string();
                if section == "WorldClocks" && !world_cleared {
                    s.world_clocks.clear();
                    world_cleared = true;
                }
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
                    "BoardScale" => {
                        s.board_scale = value.trim().parse().unwrap_or(DEFAULT_BOARD_SCALE)
                    }
                    // Default on; only an explicit 0 disables. Unparseable
                    // (and absent) stays on so pre-existing INIs animate.
                    "FlipAnimation" => {
                        s.flip_animation =
                            value.trim().parse::<i32>().map(|v| v != 0).unwrap_or(true)
                    }
                    _ => {}
                }
            } else if let Some(name) = section.strip_prefix("Screen ") {
                let entry = s.screens.entry(name.to_string()).or_default();
                match key {
                    "Orientation" => {
                        let v = value.trim().parse::<i32>().unwrap_or(0);
                        entry.orientation = Orientation::from_ini(v);
                    }
                    "Mode" => {
                        let v = value.trim().parse::<i32>().unwrap_or(0);
                        entry.mode = Mode::from_ini(v);
                    }
                    _ => {}
                }
            } else if section == "WorldClocks" {
                // key = label, value = Windows timezone key name. Labels cap
                // at LABEL_CELLS; order preserved as written.
                let label: String = key.trim().chars().take(LABEL_CELLS).collect();
                s.world_clocks.push((label, value.trim().to_string()));
            }
        }
        s
    }

    pub fn to_ini_text(&self) -> String {
        let mut out = format!(
            "[General]\r\nDisplay24Hr={}\r\nScale={}\r\nBoardScale={}\r\nFlipAnimation={}\r\n\r\n",
            if self.display_24hr { 1 } else { 0 },
            self.scale,
            self.board_scale,
            if self.flip_animation { 1 } else { 0 },
        );
        for (name, sc) in &self.screens {
            // Write a section only when it carries a non-default; then always
            // write both keys so the file is self-describing.
            if sc.orientation == Orientation::Auto && sc.mode == Mode::Clock {
                continue;
            }
            out.push_str(&format!(
                "[Screen {}]\r\nOrientation={}\r\nMode={}\r\n\r\n",
                name,
                sc.orientation.to_ini(),
                sc.mode.to_ini(),
            ));
        }
        // Always emit the header; an empty body round-trips to "no cities".
        out.push_str("[WorldClocks]\r\n");
        for (label, zone) in &self.world_clocks {
            out.push_str(&format!("{}={}\r\n", label, zone));
        }
        out.push_str("\r\n");
        out
    }

    /// Single orientation resolution point. Explicit setting wins; Auto or
    /// an absent monitor falls back to aspect (portrait → vertical).
    /// Returns `true` for vertical.
    pub fn effective_orientation(&self, device: &str, width: i32, height: i32) -> bool {
        match self.screens.get(device).map(|s| s.orientation) {
            Some(Orientation::Horizontal) => false,
            Some(Orientation::Vertical) => true,
            _ => height > width,
        }
    }

    /// Configured display mode for a monitor; absent -> Clock.
    pub fn screen_mode(&self, device: &str) -> Mode {
        self.screens.get(device).map(|s| s.mode).unwrap_or(Mode::Clock)
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
    fn board_scale_defaults_to_100() {
        assert_eq!(Settings::default().board_scale, 100);
        // absent key (pre-existing INIs) -> 100
        assert_eq!(Settings::from_ini_text("[General]\nScale=70\n").board_scale, 100);
        // garbage -> 100
        assert_eq!(Settings::from_ini_text("[General]\nBoardScale=abc\n").board_scale, 100);
    }

    #[test]
    fn board_scale_round_trips() {
        let mut s = Settings::default();
        s.board_scale = 40;
        assert_eq!(Settings::from_ini_text(&s.to_ini_text()).board_scale, 40);
        assert!(s.to_ini_text().contains("BoardScale=40"));
    }

    #[test]
    fn flip_animation_defaults_on() {
        assert!(Settings::default().flip_animation);
        // absent key -> on
        assert!(Settings::from_ini_text("[General]\nScale=70\n").flip_animation);
        // garbage -> on
        assert!(Settings::from_ini_text("[General]\nFlipAnimation=yes\n").flip_animation);
    }

    #[test]
    fn flip_animation_explicit_zero_disables() {
        assert!(!Settings::from_ini_text("[General]\nFlipAnimation=0\n").flip_animation);
        assert!(Settings::from_ini_text("[General]\nFlipAnimation=1\n").flip_animation);
    }

    #[test]
    fn flip_animation_round_trips() {
        let mut s = Settings::default();
        s.flip_animation = false;
        assert!(!Settings::from_ini_text(&s.to_ini_text()).flip_animation);
        assert!(s.to_ini_text().contains("FlipAnimation=0"));
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
            board_scale: 100,
            flip_animation: true,
            screens: std::collections::BTreeMap::new(),
            world_clocks: Vec::new(),
        };
        assert_eq!(Settings::from_ini_text(&s.to_ini_text()), s);
    }

    #[test]
    fn full_default_settings_round_trip() {
        // The most common real file: everything default, including the
        // preloaded city list through the always-emitted [WorldClocks]
        // header and its clear-on-first-header rule.
        let s = Settings::default();
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
            board_scale: 100,
            flip_animation: true,
            screens: std::collections::BTreeMap::new(),
            world_clocks: Vec::new(),
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
        assert_eq!(s.screens.get("DISPLAY1").map(|c| c.orientation), Some(Orientation::Vertical));
        assert_eq!(s.screens.get("DISPLAY1").map(|c| c.mode), Some(Mode::Clock));
    }

    #[test]
    fn screen_garbage_orientation_is_auto() {
        let s = Settings::from_ini_text("[Screen DISPLAY1]\nOrientation=abc\n");
        assert_eq!(s.screens.get("DISPLAY1").map(|c| c.orientation), Some(Orientation::Auto));
        let s = Settings::from_ini_text("[Screen DISPLAY1]\nOrientation=9\n");
        assert_eq!(s.screens.get("DISPLAY1").map(|c| c.orientation), Some(Orientation::Auto));
    }

    #[test]
    fn effective_orientation_explicit_wins() {
        let mut s = Settings::default();
        s.screens.insert("DISPLAY1".into(), ScreenSettings { orientation: Orientation::Vertical, mode: Mode::Clock });
        assert!(s.effective_orientation("DISPLAY1", 1920, 1080));
        s.screens.insert("DISPLAY1".into(), ScreenSettings { orientation: Orientation::Horizontal, mode: Mode::Clock });
        assert!(!s.effective_orientation("DISPLAY1", 1080, 1920));
    }

    #[test]
    fn effective_orientation_auto_by_aspect() {
        let s = Settings::default();
        assert!(s.effective_orientation("DISPLAY1", 1080, 1920));
        assert!(!s.effective_orientation("DISPLAY1", 1920, 1080));
        // square is horizontal: the rule is strictly height > width
        assert!(!s.effective_orientation("DISPLAY1", 1000, 1000));
    }

    #[test]
    fn screen_section_omitted_only_when_auto_and_clock() {
        let mut s = Settings::default();
        s.screens.insert("DISPLAY1".into(), ScreenSettings { orientation: Orientation::Auto, mode: Mode::Clock });
        s.screens.insert("DISPLAY2".into(), ScreenSettings { orientation: Orientation::Vertical, mode: Mode::Clock });
        s.screens.insert("DISPLAY3".into(), ScreenSettings { orientation: Orientation::Auto, mode: Mode::World });
        let text = s.to_ini_text();
        assert!(!text.contains("[Screen DISPLAY1]"));
        assert!(text.contains("[Screen DISPLAY2]\r\nOrientation=2\r\nMode=0"));
        assert!(text.contains("[Screen DISPLAY3]\r\nOrientation=0\r\nMode=1"));
    }

    #[test]
    fn screen_round_trip_preserves_explicit() {
        let mut s = Settings {
            display_24hr: true,
            scale: 40,
            board_scale: 100,
            flip_animation: true,
            screens: std::collections::BTreeMap::new(),
            world_clocks: Vec::new(),
        };
        s.screens.insert("DISPLAY1".into(), ScreenSettings { orientation: Orientation::Horizontal, mode: Mode::Clock });
        s.screens.insert("DISPLAY9".into(), ScreenSettings { orientation: Orientation::Auto, mode: Mode::World });
        assert_eq!(Settings::from_ini_text(&s.to_ini_text()), s);
    }

    #[test]
    fn mode_parses_and_defaults_to_clock() {
        let s = Settings::from_ini_text("[Screen D1]\nMode=1\n");
        assert_eq!(s.screens.get("D1").map(|c| c.mode), Some(Mode::World));
        // unknown mode value -> clock
        let s = Settings::from_ini_text("[Screen D1]\nMode=7\n");
        assert_eq!(s.screens.get("D1").map(|c| c.mode), Some(Mode::Clock));
        // orientation present, mode absent -> clock
        let s = Settings::from_ini_text("[Screen D1]\nOrientation=1\n");
        assert_eq!(s.screens.get("D1").map(|c| c.mode), Some(Mode::Clock));
    }

    #[test]
    fn absent_worldclocks_section_keeps_defaults() {
        let s = Settings::from_ini_text("[General]\nScale=70\n");
        assert_eq!(s.world_clocks.len(), 6);
    }

    #[test]
    fn present_empty_worldclocks_section_means_none() {
        let s = Settings::from_ini_text("[General]\nScale=70\n[WorldClocks]\n");
        assert!(s.world_clocks.is_empty());
    }

    #[test]
    fn worldclocks_preserve_order_and_truncate_labels() {
        let s = Settings::from_ini_text(
            "[WorldClocks]\nParis=Romance Standard Time\nAn Extremely Long City Name=UTC\n",
        );
        assert_eq!(s.world_clocks.len(), 2);
        assert_eq!(s.world_clocks[0], ("Paris".to_string(), "Romance Standard Time".to_string()));
        // 16-char cap
        assert_eq!(s.world_clocks[1].0, "An Extremely Lon");
        assert_eq!(s.world_clocks[1].1, "UTC");
    }

    #[test]
    fn worldclocks_round_trip() {
        let mut s = Settings::default();
        s.world_clocks = vec![
            ("Reykjavik".to_string(), "Greenwich Standard Time".to_string()),
            ("Kolkata".to_string(), "India Standard Time".to_string()),
        ];
        assert_eq!(Settings::from_ini_text(&s.to_ini_text()).world_clocks, s.world_clocks);
    }

    #[test]
    fn screen_mode_resolves_with_clock_default() {
        let mut s = Settings::default();
        assert_eq!(s.screen_mode("MISSING"), Mode::Clock);
        s.screens.insert("D1".into(), ScreenSettings { orientation: Orientation::Auto, mode: Mode::World });
        assert_eq!(s.screen_mode("D1"), Mode::World);
    }
}
