use std::path::{Path, PathBuf};

use crossterm::event::KeyEvent;
use serde::Deserialize;

use crate::ui::{ChoiceMenu, MenuOutcome, choice_key, render_choice};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ColorMode {
    #[default]
    Height,
    Frequency,
    Mono,
}

impl ColorMode {
    pub fn all() -> [ColorMode; 3] {
        [ColorMode::Height, ColorMode::Frequency, ColorMode::Mono]
    }

    pub fn index(self) -> usize {
        match self {
            ColorMode::Height => 0,
            ColorMode::Frequency => 1,
            ColorMode::Mono => 2,
        }
    }

    pub fn next(self) -> Self {
        match self {
            ColorMode::Height => ColorMode::Frequency,
            ColorMode::Frequency => ColorMode::Mono,
            ColorMode::Mono => ColorMode::Height,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ColorMode::Height => "Height",
            ColorMode::Frequency => "Frequency",
            ColorMode::Mono => "Mono",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            ColorMode::Height => "gradient by bar height",
            ColorMode::Frequency => "gradient by frequency",
            ColorMode::Mono => "uniform color",
        }
    }

    pub fn from_str(s: &str) -> Option<ColorMode> {
        match s.to_lowercase().as_str() {
            "height" | "1" => Some(ColorMode::Height),
            "frequency" | "freq" | "rainbow" | "2" => Some(ColorMode::Frequency),
            "mono" | "white" | "3" => Some(ColorMode::Mono),
            _ => None,
        }
    }
}

/// Parsed and ready-to-render color configuration.
#[derive(Clone, Debug)]
pub struct ColorConfig {
    pub height_stops: Vec<(f32, String)>,
    pub freq_stops: Vec<(f32, String)>,
    pub mono: String,
    pub peak: String,
    pub title: String,
    pub hint: String,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            height_stops: vec![
                (0.0, "\x1b[34m".to_string()),
                (0.6, "\x1b[35m".to_string()),
                (0.85, "\x1b[95m".to_string()),
            ],
            freq_stops: vec![
                (0.0, "\x1b[34m".to_string()),
                (0.2, "\x1b[35m".to_string()),
                (0.4, "\x1b[95m".to_string()),
                (0.6, "\x1b[1;35m".to_string()),
                (0.8, "\x1b[1;95m".to_string()),
            ],
            mono: "\x1b[95m".to_string(),
            peak: "\x1b[95m".to_string(),
            title: "\x1b[1;35m".to_string(),
            hint: "\x1b[90m".to_string(),
        }
    }
}

impl ColorConfig {
    /// Returns the ANSI escape code for a bar segment in the chosen mode.
    pub fn bar_color(
        &self,
        row: usize,
        area_h: usize,
        bar_idx: usize,
        n_bars: usize,
        mode: ColorMode,
    ) -> &str {
        match mode {
            ColorMode::Height => {
                let ratio = row as f32 / area_h.max(1) as f32;
                self.color_for_ratio(&self.height_stops, ratio)
            }
            ColorMode::Frequency => {
                let ratio = bar_idx as f32 / n_bars.max(1) as f32;
                self.color_for_ratio(&self.freq_stops, ratio)
            }
            ColorMode::Mono => &self.mono,
        }
    }

    fn color_for_ratio<'a>(&'a self, stops: &'a [(f32, String)], ratio: f32) -> &'a str {
        if stops.is_empty() {
            return &self.mono;
        }
        let mut best = &stops[0].1;
        for (th, col) in stops {
            if ratio >= *th {
                best = col;
            } else {
                break;
            }
        }
        best.as_str()
    }

    pub fn peak(&self) -> &str {
        &self.peak
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn hint(&self) -> &str {
        &self.hint
    }

    /// Finds and loads color configuration from file, falling back to defaults.
    pub fn load(explicit: Option<&str>) -> Self {
        let path = find_colors_file(explicit);
        if let Some(p) = path
            && let Ok(text) = std::fs::read_to_string(&p)
            && let Ok(cfg) = Self::from_json(&text)
        {
            return cfg;
        }
        Self::default()
    }

    /// Parses color configuration from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: RawColorConfig = serde_json::from_str(json)?;
        let mut cfg = Self::default();

        if let Some(modes) = raw.modes {
            if let Some(h) = modes.height {
                let stops = parse_color_stops(h);
                if !stops.is_empty() {
                    cfg.height_stops = stops;
                }
            }
            if let Some(f) = modes.frequency {
                let stops = parse_color_stops(f);
                if !stops.is_empty() {
                    cfg.freq_stops = stops;
                }
            }
            if let Some(m) = modes.mono {
                cfg.mono = parse_color_str(&m);
            }
        }

        if let Some(p) = raw.peak {
            cfg.peak = parse_color_str(&p);
        }

        if let Some(ui) = raw.ui {
            if let Some(t) = ui.title {
                cfg.title = parse_color_str(&t);
            }
            if let Some(h) = ui.hint {
                cfg.hint = parse_color_str(&h);
            }
        }

        Ok(cfg)
    }
}

pub fn find_colors_file(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let cwd_file = Path::new("colors.json");
    if cwd_file.exists() {
        return Some(cwd_file.to_path_buf());
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        let p = dir.join("xvisualizer").join("colors.json");
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let p = home.join(".config").join("xvisualizer").join("colors.json");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Converts a user-friendly color string (named, hex, or ANSI) to an ANSI escape code.
pub fn parse_color_str(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.starts_with("\x1b[") || trimmed.starts_with("\u{1b}[") {
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();
    let (bold, rem) = if let Some(rest) = lower.strip_prefix("bold ") {
        (true, rest.trim())
    } else {
        (false, lower.as_str())
    };

    if rem.starts_with('#')
        && let Some((r, g, b)) = parse_hex_color(rem)
    {
        return if bold {
            format!("\x1b[1;38;2;{r};{g};{b}m")
        } else {
            format!("\x1b[38;2;{r};{g};{b}m")
        };
    }

    if let Some(code) = named_color_code(rem) {
        return if bold {
            format!("\x1b[1;{code}m")
        } else {
            format!("\x1b[{code}m")
        };
    }

    let ansi_num = rem.strip_prefix("ansi:").unwrap_or(rem).parse::<u8>().ok();
    if let Some(n) = ansi_num {
        return if bold {
            format!("\x1b[1;38;5;{n}m")
        } else {
            format!("\x1b[38;5;{n}m")
        };
    }

    // Default fallback
    "\x1b[95m".to_string()
}

fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let s = hex.strip_prefix('#')?;
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some((r, g, b))
    } else if s.len() == 3 {
        let r = u8::from_str_radix(&s[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&s[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&s[2..3], 16).ok()? * 17;
        Some((r, g, b))
    } else {
        None
    }
}

fn named_color_code(name: &str) -> Option<&'static str> {
    match name {
        "black" => Some("30"),
        "red" => Some("31"),
        "green" => Some("32"),
        "yellow" => Some("33"),
        "blue" => Some("34"),
        "magenta" | "purple" => Some("35"),
        "cyan" => Some("36"),
        "white" => Some("37"),
        "bright_black" | "bright black" | "gray" | "grey" | "dark_gray" | "dark gray" => Some("90"),
        "bright_red" | "bright red" => Some("91"),
        "bright_green" | "bright green" => Some("92"),
        "bright_yellow" | "bright yellow" => Some("93"),
        "bright_blue" | "bright blue" => Some("94"),
        "bright_magenta" | "bright magenta" | "bright_purple" | "bright purple" | "pink" => {
            Some("95")
        }
        "bright_cyan" | "bright cyan" => Some("96"),
        "bright_white" | "bright white" => Some("97"),
        _ => None,
    }
}

#[derive(Deserialize)]
struct RawColorConfig {
    modes: Option<RawModes>,
    peak: Option<String>,
    ui: Option<RawUi>,
}

#[derive(Deserialize)]
struct RawModes {
    height: Option<RawColorStops>,
    frequency: Option<RawColorStops>,
    mono: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawColorStops {
    List(Vec<RawStop>),
    Single(String),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawStop {
    WithThreshold { threshold: f32, color: String },
    ColorOnly(String),
}

#[derive(Deserialize)]
struct RawUi {
    title: Option<String>,
    hint: Option<String>,
}

fn parse_color_stops(raw: RawColorStops) -> Vec<(f32, String)> {
    match raw {
        RawColorStops::Single(color) => {
            vec![(0.0, parse_color_str(&color))]
        }
        RawColorStops::List(list) => {
            if list.is_empty() {
                return Vec::new();
            }
            let has_thresholds = list
                .iter()
                .any(|s| matches!(s, RawStop::WithThreshold { .. }));
            let mut out = Vec::with_capacity(list.len());
            if has_thresholds {
                for s in list {
                    match s {
                        RawStop::WithThreshold { threshold, color } => {
                            out.push((threshold.clamp(0.0, 1.0), parse_color_str(&color)));
                        }
                        RawStop::ColorOnly(color) => {
                            out.push((0.0, parse_color_str(&color)));
                        }
                    }
                }
                out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                let n = list.len();
                for (i, s) in list.into_iter().enumerate() {
                    let th = i as f32 / n as f32;
                    let color = match s {
                        RawStop::ColorOnly(c) | RawStop::WithThreshold { color: c, .. } => c,
                    };
                    out.push((th, parse_color_str(&color)));
                }
            }
            out
        }
    }
}

pub struct ColorMenu {
    pub current: ColorMode,
    pub selected: usize,
    pub open: bool,
}

impl ColorMenu {
    pub fn new() -> Self {
        Self {
            current: ColorMode::default(),
            selected: ColorMode::default().index(),
            open: false,
        }
    }

    pub fn with(current: ColorMode) -> Self {
        let mut m = Self::new();
        m.current = current;
        m.selected = current.index();
        m.open = false;
        m
    }

    pub fn open(&mut self) {
        self.selected = self.current.index();
        self.open = true;
    }

    pub fn cycle(&mut self) {
        self.current = self.current.next();
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> MenuOutcome {
        match choice_key(key, &mut self.selected, ColorMode::all().len()) {
            MenuOutcome::Confirmed => {
                self.current = ColorMode::all()[self.selected];
                self.open = false;
                MenuOutcome::Confirmed
            }
            MenuOutcome::Cancelled => {
                self.selected = self.current.index();
                self.open = false;
                MenuOutcome::Cancelled
            }
            MenuOutcome::StillOpen => MenuOutcome::StillOpen,
        }
    }
}

pub fn render(
    frame: &mut String,
    cols: usize,
    rows: usize,
    selected: usize,
    current: ColorMode,
    use_color: bool,
    colors: &ColorConfig,
) {
    let modes = ColorMode::all();
    let options: Vec<(String, bool)> = modes
        .iter()
        .map(|m| (format!("{}  [{}]", m.name(), m.desc()), *m == current))
        .collect();
    render_choice(
        frame,
        cols,
        rows,
        &ChoiceMenu {
            title: "Setup 2/4 - Select bar color",
            options: &options,
            selected,
            quick_hint: "1 / 2 / 3 - quick select",
        },
        use_color,
        colors,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_named_colors() {
        assert_eq!(parse_color_str("blue"), "\x1b[34m");
        assert_eq!(parse_color_str("bold magenta"), "\x1b[1;35m");
        assert_eq!(parse_color_str("purple"), "\x1b[35m");
        assert_eq!(parse_color_str("#ff0000"), "\x1b[38;2;255;0;0m");
        assert_eq!(parse_color_str("bold #00ff00"), "\x1b[1;38;2;0;255;0m");
        assert_eq!(parse_color_str("#00f"), "\x1b[38;2;0;0;255m");
        assert_eq!(parse_color_str("ansi:208"), "\x1b[38;5;208m");
    }

    #[test]
    fn parses_full_json_schema() {
        let json = r##"{
            "modes": {
                "height": [
                    { "threshold": 0.0, "color": "blue" },
                    { "threshold": 0.7, "color": "#00ff00" }
                ],
                "frequency": ["cyan", "magenta"],
                "mono": "white"
            },
            "peak": "red",
            "ui": {
                "title": "bold yellow",
                "hint": "gray"
            }
        }"##;
        let cfg = ColorConfig::from_json(json).unwrap();
        assert_eq!(cfg.height_stops.len(), 2);
        assert_eq!(cfg.height_stops[0].1, "\x1b[34m");
        assert_eq!(cfg.height_stops[1].1, "\x1b[38;2;0;255;0m");
        assert_eq!(cfg.freq_stops.len(), 2);
        assert_eq!(cfg.freq_stops[0].0, 0.0);
        assert_eq!(cfg.freq_stops[1].0, 0.5);
        assert_eq!(cfg.mono, "\x1b[37m");
        assert_eq!(cfg.peak, "\x1b[31m");
        assert_eq!(cfg.title, "\x1b[1;33m");
        assert_eq!(cfg.hint, "\x1b[90m");
    }

    #[test]
    fn ratio_resolution() {
        let cfg = ColorConfig::default();
        // Height: 0.0 -> blue, 0.6 -> magenta, 0.85 -> bright_magenta
        assert_eq!(cfg.bar_color(1, 10, 0, 10, ColorMode::Height), "\x1b[34m");
        assert_eq!(cfg.bar_color(7, 10, 0, 10, ColorMode::Height), "\x1b[35m");
        assert_eq!(cfg.bar_color(9, 10, 0, 10, ColorMode::Height), "\x1b[95m");
    }
}
