//! Small shared helpers: gain state, char-safe text slicing, frame row helpers.

const MIN: f32 = 0.2;
const MAX: f32 = 8.0;
const STEP: f32 = 1.25;

/// Erase from the cursor to the end of the line.
pub const CLEAR_EOL: &str = "\x1b[K";
/// Move the cursor to the top-left corner of the screen.
pub const CURSOR_HOME: &str = "\x1b[H";
/// Reset all attributes (colors, reverse video).
pub const RESET: &str = "\x1b[0m";
/// Reverse video, used to highlight the selected menu option.
pub const HIGHLIGHT: &str = "\x1b[7m";
/// Line terminator in raw mode: no implicit CR translation anymore.
pub const NEWLINE: &str = "\r\n";

pub struct Gain {
    value: f32,
}

impl Gain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(value: f32) -> Self {
        let mut g = Self::new();
        g.set(value);
        g
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn set(&mut self, value: f32) {
        self.value = value.clamp(MIN, MAX);
    }

    pub fn up(&mut self) {
        self.value = (self.value * STEP).min(MAX);
    }

    pub fn down(&mut self) {
        self.value = (self.value / STEP).max(MIN);
    }

    pub fn reset(&mut self) {
        self.value = 1.0;
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

pub fn truncate_chars(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    let idx = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..idx]
}

pub fn center_x(cols: usize, width: usize) -> usize {
    cols.saturating_sub(width) / 2
}

pub fn push_spaces(frame: &mut String, n: usize) {
    for _ in 0..n {
        frame.push(' ');
    }
}

/// Width of `parts` after they were truncated to `cols` in order, so the
/// centering padding matches what is actually printed.
pub fn parts_width(parts: &[&str], cols: usize) -> usize {
    let mut budget = cols;
    let mut width = 0;
    for p in parts {
        let n = truncate_chars(p, budget).chars().count();
        budget -= n;
        width += n;
    }
    width
}

/// Writes `parts` into `frame`, each one truncated so the row never grows
/// past `cols` columns. Earlier parts win when the terminal is narrow.
pub fn push_parts(frame: &mut String, parts: &[&str], cols: usize) {
    let mut budget = cols;
    for p in parts {
        let text = truncate_chars(p, budget);
        budget -= text.chars().count();
        frame.push_str(text);
    }
}

/// Splits `avail` columns between a label and its detail. The label keeps at
/// most a third (but at least 10 columns) so long details stay readable.
pub fn fit_parts<'a>(label: &'a str, detail: &'a str, avail: usize) -> (&'a str, &'a str) {
    let l = label.chars().count();
    let d = detail.chars().count();
    if l + d <= avail {
        return (label, detail);
    }
    let label_len = (avail / 3).max(10).min(l).min(avail);
    (
        truncate_chars(label, label_len),
        truncate_chars(detail, avail - label_len),
    )
}

/// Drops ANSI escape sequences; render tests measure text with it.
#[cfg(test)]
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // A CSI sequence ends at its first ASCII letter.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Columns `s` occupies on screen, ignoring escape sequences.
#[cfg(test)]
pub fn visible_width(s: &str) -> usize {
    strip_ansi(s).chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_clamp_at_bounds() {
        let mut g = Gain::new();
        for _ in 0..20 {
            g.up();
        }
        assert_eq!(g.value(), MAX);
        for _ in 0..40 {
            g.down();
        }
        assert_eq!(g.value(), MIN);
        g.reset();
        assert_eq!(g.value(), 1.0);
    }

    #[test]
    fn short_strings_pass_through() {
        assert_eq!(truncate_chars("abc", 60), "abc");
    }

    #[test]
    fn long_strings_cut_on_char_boundary() {
        assert_eq!(truncate_chars("abcdef", 4), "abcd");
        // Combining mark is its own char; cut keeps whole chars only.
        assert_eq!(truncate_chars("a\u{301}bc", 1), "a");
        assert_eq!(truncate_chars("a\u{301}bc", 2), "a\u{301}");
    }

    #[test]
    fn centering_clamps_on_overflow() {
        assert_eq!(center_x(80, 10), 35);
        assert_eq!(center_x(5, 10), 0);
    }

    #[test]
    fn parts_share_the_width_budget() {
        let parts = ["Padding ", "tail"];
        assert_eq!(parts_width(&parts, 80), 12);
        // 8 chars fit, the tail is dropped instead of wrapping the row.
        assert_eq!(parts_width(&parts, 8), 8);
        let mut frame = String::new();
        push_parts(&mut frame, &parts, 8);
        assert_eq!(frame, "Padding ");
        assert_eq!(frame.chars().count(), 8);
    }

    #[test]
    fn fit_parts_never_exceeds_the_budget() {
        for avail in [0, 1, 4, 10, 17, 40] {
            let (l, d) = fit_parts("a-very-long-app-name", "stream title here", avail);
            assert!(
                l.chars().count() + d.chars().count() <= avail,
                "{l:?}+{d:?} overflows {avail}"
            );
        }
        // Room to spare: nothing is cut.
        assert_eq!(fit_parts("app", "song", 20), ("app", "song"));
    }
}
