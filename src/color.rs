use crossterm::event::KeyEvent;

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
            ColorMode::Height => "green-yellow-red by height",
            ColorMode::Frequency => "rainbow by frequency",
            ColorMode::Mono => "uniform white",
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

fn row_color_height(row: usize, area_h: usize) -> &'static str {
    let ratio = row as f32 / area_h.max(1) as f32;
    if ratio >= 0.85 {
        "\x1b[31m"
    } else if ratio >= 0.6 {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    }
}

fn freq_color(idx: usize, n_bars: usize) -> &'static str {
    let ratio = idx as f32 / n_bars.max(1) as f32;
    if ratio < 0.2 {
        "\x1b[31m"
    } else if ratio < 0.4 {
        "\x1b[33m"
    } else if ratio < 0.6 {
        "\x1b[32m"
    } else if ratio < 0.8 {
        "\x1b[36m"
    } else {
        "\x1b[35m"
    }
}

pub fn bar_color(
    row: usize,
    area_h: usize,
    bar_idx: usize,
    n_bars: usize,
    mode: ColorMode,
) -> &'static str {
    match mode {
        ColorMode::Height => row_color_height(row, area_h),
        ColorMode::Frequency => freq_color(bar_idx, n_bars),
        ColorMode::Mono => "\x1b[37m",
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
    );
}
