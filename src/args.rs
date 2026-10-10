use crate::theme::Theme;

pub struct Args {
    pub device: Option<String>,
    pub no_color: bool,
    pub theme: Option<Theme>,
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
    println!("      --no-color    disable ANSI colors");
    println!("  -h, --help        show this help");
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
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    }
}

pub enum ParseOutcome {
    Args(Args),
    Help,
}

pub(crate) fn parse_from(
    it: impl Iterator<Item = String>,
    no_color_env: bool,
) -> Result<ParseOutcome, String> {
    let mut device: Option<String> = None;
    let mut no_color = no_color_env;
    let mut theme: Option<Theme> = None;
    let mut it = it.peekable();
    while let Some(a) = it.next() {
        // Support both `--flag value`, `--flag=value` and `-d value`, `-d=value`.
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") || f == "-d" || f == "-t" => {
                (f.to_string(), Some(v.to_string()))
            }
            _ => (a.clone(), None),
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            "-d" | "--device" => {
                let Some(v) = inline.or_else(|| it.next()) else {
                    return Err(format!("error: {flag} requires a device name"));
                };
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
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn args_of(flags: &[&str], env: bool) -> Result<Args, String> {
        let it = flags.iter().map(|s| s.to_string());
        match parse_from(it, env)? {
            ParseOutcome::Args(a) => Ok(a),
            ParseOutcome::Help => panic!("unexpected help"),
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
    fn rejects_unknown_flag_theme_and_missing_value() {
        assert!(args_of(&["--nope"], false).is_err());
        assert!(args_of(&["-t", "neon"], false).is_err());
        assert!(args_of(&["-d"], false).is_err());
    }

    #[test]
    fn help_and_no_color_env() {
        let it = ["-h".to_string()].into_iter();
        assert!(matches!(parse_from(it, false), Ok(ParseOutcome::Help)));
        let a = args_of(&[], true).unwrap();
        assert!(a.no_color);
        let b = args_of(&["--no-color"], false).unwrap();
        assert!(b.no_color);
    }
}
