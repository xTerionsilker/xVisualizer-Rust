use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyEvent};

use crate::color::{ColorMenu, ColorMode};
use crate::layout::{LayoutMenu, LayoutMode};
use crate::source::PlaybackStream;
use crate::source::SourceMenu;
use crate::theme::Theme;
use crate::theme::ThemeMenu;
use crate::util::{center_x, push_spaces};

// Composition root for screen state: each menu owns its own
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
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => step_wrap(selected, -1, len),
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
        let mut theme_menu = ThemeMenu::new(theme);
        let mut color_menu = ColorMenu::with(color);
        let mut layout_menu = LayoutMenu::with(layout);
        if !setup {
            theme_menu.open = false;
            color_menu.open = false;
            layout_menu.open = false;
        } else {
            color_menu.open = false;
            layout_menu.open = false;
        }
        Self {
            theme: theme_menu,
            source: SourceMenu::new(),
            color: color_menu,
            layout: layout_menu,
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

// Generic option-list screen backing the setup wizard steps
// that offer a fixed set of choices (color, layout).
pub(crate) struct ChoiceMenu<'a> {
    pub(crate) title: &'a str,
    pub(crate) options: &'a [(String, bool)],
    pub(crate) selected: usize,
    pub(crate) quick_hint: &'a str,
}

pub(crate) fn render_choice(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &ChoiceMenu,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let lines = menu.options.len() + 6;
    let mut top = rows.saturating_sub(lines) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    let tx = center_x(cols, menu.title.chars().count());
    push_spaces(frame, tx);
    if use_color {
        frame.push_str("\x1b[1;35m");
    }
    frame.push_str(menu.title);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n");

    let hint_top = "Up/Down or Left/Right - select  |  Enter - apply";
    let hx = center_x(cols, hint_top.chars().count());
    push_spaces(frame, hx);
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(hint_top);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K\r\n\r\n\x1b[K\r\n");

    for (idx, (label, is_cur)) in menu.options.iter().enumerate() {
        let is_sel = idx == menu.selected;
        let marker = if is_sel { "> " } else { "  " };
        let cur_tag = if *is_cur { "  (current)" } else { "" };
        let w = marker.chars().count() + label.chars().count() + cur_tag.chars().count();
        let x = center_x(cols, w);
        push_spaces(frame, x);
        if is_sel {
            if use_color {
                frame.push_str("\x1b[7m");
            } else {
                frame.push_str("> ");
            }
        }
        frame.push_str(marker);
        frame.push_str(label);
        frame.push_str(cur_tag);
        if is_sel && use_color {
            frame.push_str("\x1b[0m");
        }
        frame.push_str("\x1b[K\r\n");
    }

    frame.push_str("\x1b[K\r\n");
    let mid = "  |  Enter - next  |  Esc - skip setup  |  Q - quit";
    let bx = center_x(cols, menu.quick_hint.chars().count() + mid.chars().count());
    push_spaces(frame, bx);
    if use_color {
        frame.push_str("\x1b[90m");
    }
    frame.push_str(menu.quick_hint);
    frame.push_str(mid);
    if use_color {
        frame.push_str("\x1b[0m");
    }
    frame.push_str("\x1b[K");
    // Clear any visualizer leftovers below the menu.
    top += lines;
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}
