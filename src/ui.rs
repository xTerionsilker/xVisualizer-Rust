use crossterm::event::{KeyCode, KeyEvent};

use crate::color::{ColorConfig, ColorMenu, ColorMode};
use crate::layout::{LayoutMenu, LayoutMode};
use crate::source::{PlaybackStream, SourceMenu};
use crate::theme::{Theme, ThemeMenu};
use crate::util::{
    CLEAR_EOL, CURSOR_HOME, HIGHLIGHT, NEWLINE, RESET, center_x, fit_parts, parts_width,
    push_parts, push_spaces,
};

// Composition root for screen state: each menu owns its
// state in its file, `main` owns this struct.
pub struct UiState {
    pub theme: ThemeMenu,
    pub source: SourceMenu,
    pub color: ColorMenu,
    pub layout: LayoutMenu,
    pub paused: bool,
    pub in_help: bool,
    // True until the startup wizard (theme -> color -> layout -> source)
    // completes or is skipped with Esc.
    pub setup: bool,
}

// Source switching reroutes PulseAudio and restarts capture, so the handler
// only returns a request and `main` applies it.
pub enum UiEvent {
    SelectSystem,
    SelectStream(PlaybackStream),
    GainUp,
    GainDown,
    GainReset,
}

pub enum MenuOutcome {
    StillOpen,
    Confirmed,
    Cancelled,
}

/// A closed set of user-selectable options shown in the shared option list
/// (theme, bar color, layout).
///
/// Implementors only declare their variants plus the `(name, detail)` labels
/// used by the menu; index, cycling and string parsing are derived from that
/// table so the menus cannot drift apart.
pub trait Choice: Copy + Eq + 'static {
    const ALL: &'static [Self];
    /// `(name, detail)` per option, in `ALL` order.
    const TABLE: &'static [(&'static str, &'static str)];

    /// Extra accepted spellings, e.g. `freq` for `Frequency`.
    fn aliases(self) -> &'static [&'static str] {
        &[]
    }

    /// Menu hint listing the quick-select numbers, e.g. `1 / 3 - quick select`.
    ///
    /// Built from `ALL` so the numbers cannot drift out of sync with the
    /// option count; menus render rarely enough for the small allocation.
    fn quick_hint(self) -> String {
        let mut hint = String::from("1");
        for i in 2..=Self::ALL.len() {
            hint.push_str(" / ");
            hint.push_str(&i.to_string());
        }
        hint.push_str(" - quick select");
        hint
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    fn next(self) -> Self {
        let i = self.index();
        Self::ALL
            .get((i + 1) % Self::ALL.len().max(1))
            .copied()
            .unwrap_or(self)
    }

    fn name(self) -> &'static str {
        Self::TABLE[self.index()].0
    }

    /// Accepts the name, any alias, or a 1-based number as used by CLI flags
    /// and the `1 / 2 / 3` quick-select keys.
    fn from_str(s: &str) -> Option<Self> {
        let low = s.trim().to_lowercase();
        if let Some(i) = low.parse::<usize>().ok().and_then(|n| n.checked_sub(1))
            && i < Self::ALL.len()
        {
            return Some(Self::ALL[i]);
        }
        Self::ALL
            .iter()
            .copied()
            .find(|v| v.name().eq_ignore_ascii_case(&low) || v.aliases().iter().any(|a| *a == low))
    }
}

// Shared selection logic for the option-list menus. Each menu keeps its
// own state and applies confirm/cancel itself; only navigation lives here.
pub(crate) fn step_wrap(selected: &mut usize, dir: i32, len: usize) {
    if len == 0 {
        return;
    }
    *selected = (*selected as i32 + dir).rem_euclid(len as i32) as usize;
}

pub(crate) fn choice_key(key: KeyEvent, selected: &mut usize, len: usize) -> MenuOutcome {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') | KeyCode::BackTab => {
            step_wrap(selected, -1, len)
        }
        KeyCode::Left | KeyCode::Char('h') => step_wrap(selected, -1, len),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => step_wrap(selected, 1, len),
        KeyCode::Right | KeyCode::Char('l') => step_wrap(selected, 1, len),
        KeyCode::Char(c) if ('1'..='9').contains(&c) => {
            let idx = (c as usize) - ('1' as usize);
            if idx < len {
                *selected = idx;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => return MenuOutcome::Confirmed,
        KeyCode::Esc => return MenuOutcome::Cancelled,
        _ => {}
    }
    MenuOutcome::StillOpen
}

impl UiState {
    pub fn with_initial(theme: Theme, color: ColorMode, layout: LayoutMode, setup: bool) -> Self {
        // The wizard starts on the theme step; the other menus are opened by
        // `input` as the wizard advances.
        Self {
            theme: ThemeMenu::new(theme, setup),
            source: SourceMenu::new(),
            color: ColorMenu::new(color),
            layout: LayoutMenu::new(layout),
            paused: false,
            in_help: false,
            setup,
        }
    }

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub(crate) fn toggle_help(&mut self) {
        self.in_help = !self.in_help;
    }
}

/// One option row of a menu block.
pub(crate) struct OptionRow<'a> {
    pub(crate) index: usize,
    pub(crate) selected: usize,
    pub(crate) name: &'a str,
    pub(crate) detail: &'a str,
    pub(crate) current: bool,
}

/// Vertical cursor for menu screens.
///
/// Every screen renders exactly `rows` lines: rows inside the block carry the
/// menu, the rest are cleared padding. A menu taller than the terminal is
/// therefore clipped instead of scrolling the screen.
pub(crate) struct Block<'a> {
    frame: &'a mut String,
    /// First screen row occupied by the block.
    top: usize,
    lines: usize,
}

impl<'a> Block<'a> {
    /// Clears the frame and reserves `lines` rows, centered inside `rows`.
    pub(crate) fn new(frame: &'a mut String, lines: usize, rows: usize) -> Self {
        frame.clear();
        frame.push_str(CURSOR_HOME);
        Self {
            frame,
            top: rows.saturating_sub(lines) / 2,
            lines,
        }
    }

    /// Emits screen row `r` (row separator plus erase) and returns the block
    /// line it carries, or `None` for padding outside the block. Callers must
    /// run this for `0..rows` so the frame always covers the whole screen.
    pub(crate) fn row(&mut self, r: usize) -> Option<usize> {
        if r > 0 {
            self.frame.push_str(NEWLINE);
        }
        self.frame.push_str(CLEAR_EOL);
        r.checked_sub(self.top).filter(|i| *i < self.lines)
    }

    /// Centers one row from `parts`, truncating them to the terminal width.
    pub(crate) fn centered(&mut self, cols: usize, parts: &[&str], use_color: bool, color: &str) {
        let width = parts_width(parts, cols);
        push_spaces(self.frame, center_x(cols, width));
        if use_color {
            self.frame.push_str(color);
        }
        push_parts(self.frame, parts, cols);
        if use_color {
            self.frame.push_str(RESET);
        }
    }

    /// One option row: `> Name  [detail]  (current)`, highlight on selection.
    pub(crate) fn option(&mut self, cols: usize, opt: &OptionRow<'_>, use_color: bool) {
        let selected = opt.index == opt.selected;
        let marker = if selected { "> " } else { "  " };
        let current = if opt.current { "  (current)" } else { "" };
        // "  [" and "]" around the detail, only when there is one.
        let bracket = if opt.detail.is_empty() { 0 } else { 4 };
        let overhead = marker.chars().count() + bracket + current.chars().count();
        let (name, detail) = fit_parts(opt.name, opt.detail, cols.saturating_sub(overhead));
        let width = overhead + name.chars().count() + detail.chars().count();

        push_spaces(self.frame, center_x(cols, width));
        if selected && use_color {
            self.frame.push_str(HIGHLIGHT);
        }
        self.frame.push_str(marker);
        self.frame.push_str(name);
        if !detail.is_empty() {
            self.frame.push_str("  [");
            self.frame.push_str(detail);
            self.frame.push(']');
        }
        self.frame.push_str(current);
        if selected && use_color {
            self.frame.push_str(RESET);
        }
    }
}

/// Generic option-list screen backing the setup wizard steps and the
/// runtime menus that offer a fixed set of choices (theme, color, layout).
pub(crate) struct ChoiceMenu<'a> {
    pub(crate) title: &'a str,
    pub(crate) options: &'a [(&'a str, &'a str)],
    /// Index of the option currently applied.
    pub(crate) current: usize,
    pub(crate) selected: usize,
    pub(crate) quick_hint: String,
    pub(crate) setup: bool,
}

const NAV_HINT: &str = "Up/Down or Left/Right - select  |  Enter - apply";
const APPLY_TAIL: &str = "  |  Enter - apply  |  Esc - back  |  Q - quit";
const SETUP_TAIL: &str = "  |  Enter - next  |  Esc - skip setup  |  Q - quit";

pub(crate) fn render_choice(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &ChoiceMenu<'_>,
    use_color: bool,
    colors: &ColorConfig,
) {
    // title, hint, blank, one row per option, blank, bottom hint
    let lines = menu.options.len() + 5;
    let last = lines - 1;
    let mut block = Block::new(frame, lines, rows);
    for r in 0..rows {
        let Some(line) = block.row(r) else {
            continue;
        };
        match line {
            0 => block.centered(cols, &[menu.title], use_color, colors.title()),
            1 => block.centered(cols, &[NAV_HINT], use_color, colors.hint()),
            2 => {}
            i if i <= last - 2 => {
                let idx = i - 3;
                let (name, detail) = menu.options[idx];
                block.option(
                    cols,
                    &OptionRow {
                        index: idx,
                        selected: menu.selected,
                        name,
                        detail,
                        current: idx == menu.current,
                    },
                    use_color,
                );
            }
            i if i == last - 1 => {}
            _ => {
                let tail = if menu.setup { SETUP_TAIL } else { APPLY_TAIL };
                block.centered(cols, &[&menu.quick_hint, tail], use_color, colors.hint());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::strip_ansi;

    #[test]
    fn choice_table_drives_names_and_cycling() {
        // Numeric parsing, aliases, case-insensitivity and round tripping.
        assert_eq!(Choice::from_str("1"), Some(LayoutMode::Symmetric));
        assert_eq!(Choice::from_str(" 2 "), Some(LayoutMode::LeftToRight));
        assert_eq!(Choice::from_str("LTR"), Some(LayoutMode::LeftToRight));
        assert_eq!(LayoutMode::from_str("nope"), None);
        assert_eq!(LayoutMode::Symmetric.next(), LayoutMode::LeftToRight);
        assert_eq!(LayoutMode::LeftToRight.next(), LayoutMode::Symmetric);
        assert_eq!(ColorMode::from_str("rainbow"), Some(ColorMode::Frequency));
        assert_eq!(Theme::from_str("FAt"), Some(Theme::Solid));
        // The label table is the single source for names, details and parsing.
        assert_eq!(ColorMode::TABLE[ColorMode::Mono.index()].1, "uniform color");
        for v in LayoutMode::ALL {
            let i = v.index();
            assert!(i < LayoutMode::ALL.len());
            assert_eq!(LayoutMode::ALL[i], *v);
            assert_eq!(v.name(), LayoutMode::TABLE[i].0);
            assert_eq!(LayoutMode::from_str(v.name()), Some(*v));
            assert_eq!(LayoutMode::from_str(&(i + 1).to_string()), Some(*v));
        }
    }

    #[test]
    fn option_row_has_one_marker() {
        let colors = ColorConfig::default();
        for use_color in [false, true] {
            let mut frame = String::new();
            render_choice(
                &mut frame,
                80,
                24,
                &ChoiceMenu {
                    title: "Select theme",
                    options: Theme::TABLE,
                    current: 0,
                    selected: 1,
                    quick_hint: Theme::Classic.quick_hint(),
                    setup: false,
                },
                use_color,
                &colors,
            );
            let plain = strip_ansi(&frame);
            let row = |name: &str| {
                plain
                    .lines()
                    .find(|l| l.contains(name))
                    .unwrap_or_else(|| panic!("{name} row is rendered"))
                    .to_string()
            };
            let selected = row("Solid");
            assert_eq!(
                selected.matches("> ").count(),
                1,
                "marker duplicated without color: {selected:?}"
            );
            assert!(selected.contains("wide bars without gaps"));
            assert!(!selected.contains("(current)"));
            assert!(row("Classic").contains("(current)"));
        }
    }

    #[test]
    fn choice_screen_never_exceeds_the_terminal() {
        let colors = ColorConfig::default();
        for (cols, rows) in [(80, 24), (30, 8), (20, 7), (12, 3), (5, 1)] {
            let mut frame = String::new();
            render_choice(
                &mut frame,
                cols,
                rows,
                &ChoiceMenu {
                    title: "Setup 2/4 - Select bar color",
                    options: ColorMode::TABLE,
                    current: 0,
                    selected: 0,
                    quick_hint: ColorMode::Height.quick_hint(),
                    setup: true,
                },
                false,
                &colors,
            );
            let lines: Vec<String> = frame.split(NEWLINE).map(strip_ansi).collect();
            assert_eq!(
                lines.len(),
                rows,
                "{cols}x{rows} produced {} lines",
                lines.len()
            );
            for line in &lines {
                assert!(
                    line.chars().count() <= cols,
                    "{cols}x{rows}: row too wide: {line:?}"
                );
            }
        }
    }
}
