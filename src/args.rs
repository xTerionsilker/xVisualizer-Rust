use crate::color::ColorMode;
use crate::layout::LayoutMode;
use crate::theme::Theme;
use crate::ui::Choice;

pub struct Args {
    pub device: Option<String>,
    pub no_color: bool,
    pub theme: Option<Theme>,
    pub color: Option<ColorMode>,
    pub layout: Option<LayoutMode>,
    pub gain: Option<f32>,
    pub source: Option<String>,
    pub colors_file: Option<String>,
    pub no_setup: bool,
}

pub fn print_help() {
    println!("xvisualizer - ASCII visualizer for system audio");
    println!();
    println!("Usage: xvisualizer [OPTIONS]");
    println!();
    println!("Options:");
    println!(
        "  -d, --device NAME Pulse source (default: auto-detected monitor of the default sink)"
    );
    println!("  -t, --theme NAME  start theme: classic | solid (default: classic)");
    println!("      --color NAME  start color: height | frequency | mono");
    println!("      --layout NAME start layout: symmetric | left-right");
    println!("  -g, --gain X      start sensitivity 0.2-8.0 (default: 1.0)");
    println!("      --source NAME start source: system or app name (default: system)");
    println!(
        "      --colors PATH custom colors JSON file (default: colors.json or ~/.config/xvisualizer/colors.json)"
    );
    println!("      --no-setup    skip the startup setup wizard");
    println!("      --no-color    disable ANSI colors");
    println!("  -v, --version     show version");
    println!("  -h, --help        show this help");
    println!();
    println!("Settings are saved to ~/.config/xvisualizer/config (or $XDG_CONFIG_HOME)");
    println!("and reused on the next run. CLI flags override the saved values.");
    println!();
    println!("Keys (in program):");
    println!("  Up/Down or Left/Right  choose theme in the menu (also opens it)");
    println!("  Enter                  apply theme");
    println!("  T / Tab / M / F2       open theme menu, Esc - back");
    println!("  S                      capture source: system mix or one app");
    println!("  R (in source menu)     refresh the app list");
    println!("  1 / 2                  quick theme switch");
    println!("  Space / P              pause / resume");
    println!("  + / -                  sensitivity up / down, G - reset");
    println!("  L                      toggle layout (Symmetric / Left-Right)");
    println!("  C                      cycle bar color");
    println!("  H / ? / F1             help overlay");
    println!("  Q or Ctrl+C            quit");
}

pub fn parse_args() -> Args {
    // Honor https://no-color.org/ in addition to the flag.
    let no_color_env = std::env::var_os("NO_COLOR").is_some();
    match parse_from(std::env::args().skip(1), no_color_env) {
        Ok(ParseOutcome::Args(args)) => args,
        Ok(ParseOutcome::Help) => {
            print_help();
            std::process::exit(0);
        }
        Ok(ParseOutcome::Version) => {
            println!("xvisualizer {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    }
}

pub enum ParseOutcome {
    Args(Args),
    Help,
    Version,
}

pub(crate) fn parse_from(
    it: impl Iterator<Item = String>,
    no_color_env: bool,
) -> Result<ParseOutcome, String> {
    let mut device: Option<String> = None;
    let mut no_color = no_color_env;
    let mut theme: Option<Theme> = None;
    let mut color: Option<ColorMode> = None;
    let mut layout: Option<LayoutMode> = None;
    let mut gain: Option<f32> = None;
    let mut source: Option<String> = None;
    let mut colors_file: Option<String> = None;
    let mut no_setup = false;
    let mut it = it.peekable();
    while let Some(a) = it.next() {
        // Support both `--flag value`, `--flag=value` and `-d value`, `-d=value`.
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") || f == "-d" || f == "-t" || f == "-g" => {
                (f.to_string(), Some(v.to_string()))
            }
            _ => (a.clone(), None),
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            "-v" | "--version" => return Ok(ParseOutcome::Version),
            "-d" | "--device" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err(format!("error: {flag} requires a device name"));
                };
                if v.trim().is_empty() {
                    return Err(format!("error: {flag} requires a non-empty device name"));
                }
                device = Some(v);
            }
            "-t" | "--theme" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err(format!("error: {flag} requires classic|solid"));
                };
                match Theme::from_str(&v) {
                    Some(t) => theme = Some(t),
                    None => {
                        return Err(format!("error: unknown theme '{v}' (classic|solid)"));
                    }
                }
            }
            "--color" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err("error: --color requires height|frequency|mono".to_string());
                };
                match ColorMode::from_str(&v) {
                    Some(m) => color = Some(m),
                    None => {
                        return Err(format!(
                            "error: unknown color '{v}' (height|frequency|mono)"
                        ));
                    }
                }
            }
            "--layout" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err("error: --layout requires symmetric|left-right".to_string());
                };
                match LayoutMode::from_str(&v) {
                    Some(m) => layout = Some(m),
                    None => {
                        return Err(format!(
                            "error: unknown layout '{v}' (symmetric|left-right)"
                        ));
                    }
                }
            }
            "-g" | "--gain" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err(format!("error: {flag} requires a number 0.2-8.0"));
                };
                match v.parse::<f32>() {
                    Ok(g) if g.is_finite() && (0.2..=8.0).contains(&g) => gain = Some(g),
                    _ => return Err(format!("error: bad gain '{v}' (need 0.2-8.0)")),
                }
            }
            "--source" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err("error: --source requires system or an app name".to_string());
                };
                if v.trim().is_empty() {
                    return Err("error: --source requires system or an app name".to_string());
                }
                source = Some(v);
            }
            "--colors" | "--colors-file" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err("error: --colors requires a file path".to_string());
                };
                if v.trim().is_empty() {
                    return Err("error: --colors requires a file path".to_string());
                }
                colors_file = Some(v);
            }
            "--no-setup" => no_setup = true,
            "--no-color" => no_color = true,
            _ => {
                return Err(format!("error: unknown flag '{a}'. See xvisualizer --help"));
            }
        }
    }
    Ok(ParseOutcome::Args(Args {
        device,
        no_color,
        theme,
        color,
        layout,
        gain,
        source,
        colors_file,
        no_setup,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(flags: &[&str], env: bool) -> Result<Args, String> {
        let it = flags.iter().map(|s| s.to_string());
        match parse_from(it, env)? {
            ParseOutcome::Args(a) => Ok(a),
            ParseOutcome::Help => panic!("unexpected help"),
            ParseOutcome::Version => panic!("unexpected version"),
        }
    }

    #[test]
    fn parses_device_and_theme_forms() {
        let a = args_of(&["--device", "mic", "-t", "solid"], false).unwrap();
        assert_eq!(a.device.as_deref(), Some("mic"));
        assert_eq!(a.theme, Some(Theme::Solid));
        let b = args_of(&["--device=mic", "--theme=1"], false).unwrap();
        assert_eq!(b.device.as_deref(), Some("mic"));
        assert_eq!(b.theme, Some(Theme::Classic));
    }

    #[test]
    fn parses_new_startup_flags() {
        let a = args_of(
            &[
                "--color",
                "frequency",
                "--layout=left-right",
                "--gain",
                "2.5",
                "--source",
                "firefox",
                "--no-setup",
            ],
            false,
        )
        .unwrap();
        assert_eq!(a.color, Some(ColorMode::Frequency));
        assert_eq!(a.layout, Some(LayoutMode::LeftToRight));
        assert_eq!(a.gain, Some(2.5));
        assert_eq!(a.source.as_deref(), Some("firefox"));
        assert!(a.no_setup);
        let b = args_of(&["-g=0.5", "--color=mono"], false).unwrap();
        assert_eq!(b.gain, Some(0.5));
        assert_eq!(b.color, Some(ColorMode::Mono));
    }

    #[test]
    fn rejects_unknown_flag_theme_and_missing_value() {
        assert!(args_of(&["--nope"], false).is_err());
        assert!(args_of(&["-t", "neon"], false).is_err());
        assert!(args_of(&["-d"], false).is_err());
        assert!(args_of(&["--color", "neon"], false).is_err());
        assert!(args_of(&["--layout", "neon"], false).is_err());
        assert!(args_of(&["--gain", "99"], false).is_err());
        assert!(args_of(&["--gain", "abc"], false).is_err());
        assert!(args_of(&["--source"], false).is_err());
        assert!(args_of(&["--source", ""], false).is_err());
        assert!(args_of(&["-d", ""], false).is_err());
        assert!(args_of(&["--colors", ""], false).is_err());
    }

    #[test]
    fn parses_version_flag() {
        let it = ["--version".to_string()].into_iter();
        assert!(matches!(parse_from(it, false), Ok(ParseOutcome::Version)));
        let it_short = ["-v".to_string()].into_iter();
        assert!(matches!(
            parse_from(it_short, false),
            Ok(ParseOutcome::Version)
        ));
    }

    #[test]
    fn help_and_no_color_env() {
        let it = ["-h".to_string()].into_iter();
        assert!(matches!(parse_from(it, false), Ok(ParseOutcome::Help)));
        let a = args_of(&[], true).unwrap();
        assert!(a.no_color);
        let b = args_of(&["--no-color"], false).unwrap();
        assert!(b.no_color);
        let c = args_of(&["--colors", "custom.json"], false).unwrap();
        assert_eq!(c.colors_file.as_deref(), Some("custom.json"));
    }
}
