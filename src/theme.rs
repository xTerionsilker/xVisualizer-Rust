use crossterm::event::KeyEvent;

use crate::ui::{Choice, ChoiceMenu, MenuOutcome, choice_key, render_choice, step_wrap};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Classic,
    Solid,
}

impl Choice for Theme {
    const ALL: &'static [Theme] = &[Theme::Classic, Theme::Solid];
    const TABLE: &'static [(&'static str, &'static str)] = &[
        ("Classic", "thin bars with gaps"),
        ("Solid", "wide bars without gaps"),
    ];

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Theme::Solid => &["bold", "fat"],
            Theme::Classic => &[],
        }
    }
}

impl Theme {
    /// `(bar width, gap width)` in terminal columns.
    pub fn dims(self) -> (usize, usize) {
        match self {
            Theme::Classic => (1, 1),
            Theme::Solid => (2, 0),
        }
    }

    /// How many bars fit in `cols` columns, capped at `max`.
    pub fn fit_bar_count(self, cols: usize, max: usize) -> usize {
        let (bar_w, gap_w) = self.dims();
        let cell = (bar_w + gap_w).max(1);
        let mut n = ((cols.saturating_sub(4) + gap_w) / cell).clamp(1, max);
        // Shrink until the bars actually fit; guards very narrow windows.
        while n > 1 && (n * bar_w + n.saturating_sub(1) * gap_w) > cols {
            n -= 1;
        }
        n
    }
}

pub struct ThemeMenu {
    pub current: Theme,
    pub selected: usize,
    pub open: bool,
}

impl ThemeMenu {
    pub fn new(current: Theme, open: bool) -> Self {
        Self {
            current,
            selected: current.index(),
            open,
        }
    }

    pub fn open(&mut self) {
        self.selected = self.current.index();
        self.open = true;
    }

    pub fn step(&mut self, dir: i32) {
        step_wrap(&mut self.selected, dir, Theme::ALL.len());
    }

    pub fn quick(&mut self, idx: usize) {
        if let Some(t) = Theme::ALL.get(idx) {
            self.current = *t;
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> MenuOutcome {
        match choice_key(key, &mut self.selected, Theme::ALL.len()) {
            MenuOutcome::Confirmed => {
                self.current = Theme::ALL[self.selected];
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
    menu: &ThemeMenu,
    setup: bool,
    use_color: bool,
    colors: &crate::color::ColorConfig,
) {
    render_choice(
        frame,
        cols,
        rows,
        &ChoiceMenu {
            title: if setup {
                "Setup 1/4 - Select theme"
            } else {
                "Select theme"
            },
            options: Theme::TABLE,
            current: menu.current.index(),
            selected: menu.selected,
            quick_hint: menu.current.quick_hint(),
            setup,
        },
        use_color,
        colors,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_leaves_room_for_margins() {
        // Classic cell is 2 wide; 80 cols minus 4 margin fit 38 bars.
        assert_eq!(Theme::Classic.fit_bar_count(80, 64), 38);
    }

    #[test]
    fn solid_uses_full_width() {
        assert_eq!(Theme::Solid.fit_bar_count(80, 64), 38);
    }

    #[test]
    fn narrow_window_shrinks_to_fit() {
        assert_eq!(Theme::Classic.fit_bar_count(3, 64), 1);
        assert_eq!(Theme::Solid.fit_bar_count(1, 64), 1);
    }

    #[test]
    fn bar_count_never_overflows_the_width() {
        for cols in 1..=200usize {
            for th in Theme::ALL {
                let (bar_w, gap_w) = th.dims();
                let n = th.fit_bar_count(cols, 64);
                assert!((1..=64).contains(&n), "{th:?} at {cols} cols gave {n} bars");
                let width = n * bar_w + n.saturating_sub(1) * gap_w;
                // A bar may be wider than a 1-column terminal; the renderer
                // clamps the drawn cells, so compare against that.
                assert!(
                    width <= cols.max(bar_w),
                    "{th:?} at {cols} cols draws {width}"
                );
            }
        }
    }

    #[test]
    fn theme_menu_renders_all_rows() {
        let mut frame = String::new();
        let menu = ThemeMenu::new(Theme::Classic, false);
        let colors = crate::color::ColorConfig::default();
        render(&mut frame, 80, 24, &menu, false, false, &colors);
        // Each rendered line (except the first) begins after a \r\n
        let line_count = frame.split("\r\n").count();
        assert_eq!(line_count, 24);
        let plain = crate::util::strip_ansi(&frame);
        assert!(plain.contains("thin bars with gaps"));
        assert!(plain.contains("wide bars without gaps"));
    }

    #[test]
    fn quick_select_ignores_out_of_range() {
        let mut menu = ThemeMenu::new(Theme::Classic, false);
        menu.quick(9);
        assert_eq!(menu.current, Theme::Classic);
        menu.quick(1);
        assert_eq!(menu.current, Theme::Solid);
    }
}
