use crossterm::event::KeyEvent;

use crate::ui::{Choice, ChoiceMenu, MenuOutcome, choice_key, render_choice};

/// Spectrum layout: mirrored around the center (bass in the middle)
/// or plain left-to-right (bass on the left, treble on the right).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum LayoutMode {
    #[default]
    Symmetric,
    LeftToRight,
}

impl Choice for LayoutMode {
    const ALL: &'static [LayoutMode] = &[LayoutMode::Symmetric, LayoutMode::LeftToRight];
    const TABLE: &'static [(&'static str, &'static str)] = &[
        ("Symmetric", "mirrored, bass in the center"),
        ("Left-Right", "bass left, treble right"),
    ];

    fn aliases(self) -> &'static [&'static str] {
        match self {
            LayoutMode::LeftToRight => &["left", "leftright", "ltr"],
            LayoutMode::Symmetric => &["mirror"],
        }
    }
}

/// Center-symmetric spectrum mapping: bass in the middle, treble at the edges.
/// The analyzer produces `unique` bands (low -> high); the visualizer mirrors
/// them into `unique * 2 - 1` display bars so left and right stay symmetric.
pub fn unique_count(display_fit: usize) -> usize {
    display_fit.div_ceil(2).max(1)
}

pub fn display_count(unique: usize) -> usize {
    unique.saturating_mul(2).saturating_sub(1).max(1)
}

/// Display position `d` (0 = left edge) to spectrum index (0 = bass).
pub fn spectrum_index(display_idx: usize, unique: usize) -> usize {
    if unique <= 1 {
        return 0;
    }
    let center = unique - 1;
    (display_idx as isize - center as isize).unsigned_abs()
}

pub struct LayoutMenu {
    pub current: LayoutMode,
    pub selected: usize,
    pub open: bool,
}

impl LayoutMenu {
    pub fn new(current: LayoutMode) -> Self {
        Self {
            current,
            selected: current.index(),
            open: false,
        }
    }

    pub fn open(&mut self) {
        self.selected = self.current.index();
        self.open = true;
    }

    pub fn toggle(&mut self) {
        self.current = self.current.next();
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> MenuOutcome {
        match choice_key(key, &mut self.selected, LayoutMode::ALL.len()) {
            MenuOutcome::Confirmed => {
                self.current = LayoutMode::ALL[self.selected];
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
    menu: &LayoutMenu,
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
                "Setup 3/4 - Select layout"
            } else {
                "Select layout"
            },
            options: LayoutMode::TABLE,
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
    fn mirror_counts_round_trip() {
        assert_eq!(unique_count(7), 4);
        assert_eq!(display_count(4), 7);
        assert_eq!(display_count(1), 1);
    }

    #[test]
    fn bass_sits_in_the_center() {
        // unique=3 -> display [2,1,0,1,2], center is bass.
        assert_eq!(spectrum_index(0, 3), 2);
        assert_eq!(spectrum_index(2, 3), 0);
        assert_eq!(spectrum_index(4, 3), 2);
    }

    #[test]
    fn single_band_maps_to_zero() {
        assert_eq!(spectrum_index(5, 1), 0);
        assert_eq!(spectrum_index(0, 0), 0);
    }

    #[test]
    fn mirror_mapping_stays_inside_the_spectrum() {
        for unique in 1..=32usize {
            for d in 0..display_count(unique) {
                assert!(spectrum_index(d, unique) < unique);
            }
        }
    }
}
