use std::path::PathBuf;

use crate::color::ColorMode;
use crate::layout::LayoutMode;
use crate::theme::Theme;

pub struct Persisted {
    pub theme: Option<Theme>,
    pub color: Option<ColorMode>,
    pub layout: Option<LayoutMode>,
    pub gain: Option<f32>,
    pub device: Option<String>,
    pub found: bool,
}

pub fn config_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        return dir.join("xvisualizer").join("config");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("xvisualizer").join("config")
}

pub fn load() -> Persisted {
    load_from(&config_path())
}

pub fn load_from(path: &std::path::Path) -> Persisted {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = Persisted {
        theme: None,
        color: None,
        layout: None,
        gain: None,
        device: None,
        found: false,
    };
    if text.trim().is_empty() {
        return out;
    }
    out.found = true;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "theme" => out.theme = Theme::from_str(v.trim()),
            "color" => out.color = ColorMode::from_str(v.trim()),
            "layout" => out.layout = LayoutMode::from_str(v.trim()),
            "gain" => {
                if let Ok(g) = v.trim().parse::<f32>()
                    && g.is_finite()
                {
                    out.gain = Some(g.clamp(0.2, 8.0));
                }
            }
            "device" => {
                let v = v.trim();
                if !v.is_empty() {
                    out.device = Some(v.to_string());
                }
            }
            _ => {}
        }
    }
    out
}

pub fn save(
    theme: Theme,
    color: ColorMode,
    layout: LayoutMode,
    gain: f32,
    device: &Option<String>,
) {
    save_to(&config_path(), theme, color, layout, gain, device);
}

pub fn save_to(
    path: &std::path::Path,
    theme: Theme,
    color: ColorMode,
    layout: LayoutMode,
    gain: f32,
    device: &Option<String>,
) {
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let mut text = format!(
        "theme={}\ncolor={}\nlayout={}\ngain={:.2}\n",
        theme.name().to_lowercase(),
        color.name().to_lowercase(),
        layout.name().to_lowercase().replace('-', ""),
        gain,
    );
    if let Some(d) = device
        && !d.is_empty()
    {
        text.push_str(&format!("device={d}\n"));
    }
    let _ = std::fs::write(path, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_file() {
        let path = std::env::temp_dir().join(format!("xviz_cfg_{}.tmp", std::process::id()));
        save_to(
            &path,
            Theme::Solid,
            ColorMode::Frequency,
            LayoutMode::LeftToRight,
            2.5,
            &Some("my.monitor".to_string()),
        );
        let got = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert!(got.found);
        assert_eq!(got.theme, Some(Theme::Solid));
        assert_eq!(got.color, Some(ColorMode::Frequency));
        assert_eq!(got.layout, Some(LayoutMode::LeftToRight));
        assert!((got.gain.unwrap() - 2.5).abs() < 0.01);
        assert_eq!(got.device.as_deref(), Some("my.monitor"));
    }

    #[test]
    fn missing_file_gives_empty() {
        let got = load_from(std::path::Path::new("/nonexistent-xviz-config-test"));
        assert!(!got.found);
        assert_eq!(got.theme, None);
    }

    #[test]
    fn bad_lines_are_ignored() {
        let path = std::env::temp_dir().join(format!("xviz_bad_{}.tmp", std::process::id()));
        let _ = std::fs::write(&path, "theme=nope\ngain=abc\nnotakv\ncolor=mono\n");
        let got = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(got.theme, None);
        assert_eq!(got.gain, None);
        assert_eq!(got.color, Some(ColorMode::Mono));
    }
}
