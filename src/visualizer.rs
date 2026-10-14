use std::fmt::Write as _;

use crate::color::{ColorConfig, ColorMode};
use crate::layout::{LayoutMode, display_count, spectrum_index};
use crate::util::{push_spaces, truncate_chars};

const FOOTER_HEIGHT: usize = 2;

pub struct Visualizer<'a> {
    pub bars: &'a [f32],
    pub peaks: &'a [f32],
    pub cols: usize,
    pub rows: usize,
    pub bar_width: usize,
    pub gap_width: usize,
    pub use_color: bool,
    pub source: &'a str,
    pub theme_name: &'a str,
    pub notice: Option<&'a str>,
    pub color_mode: ColorMode,
    pub layout: LayoutMode,
    pub paused: bool,
    pub gain: f32,
    pub colors: &'a ColorConfig,
}

const PARTS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn render_visualizer(frame: &mut String, v: &Visualizer) {
    if v.rows == 0 || v.cols == 0 {
        frame.clear();
        return;
    }

    let symmetric = v.layout == LayoutMode::Symmetric;
    let cell = v.bar_width + v.gap_width;
    let max_fit = (v.cols + v.gap_width).checked_div(cell).unwrap_or(0);
    let initial_unique = v.bars.len().min(crate::dsp::MAX_BARS);
    let (n_bars, n_unique) = if symmetric {
        let fit = if max_fit.is_multiple_of(2) {
            max_fit.saturating_sub(1)
        } else {
            max_fit
        };
        let bars = display_count(initial_unique)
            .min(crate::dsp::MAX_BARS)
            .min(fit);
        let unique = bars.div_ceil(2);
        (bars, unique)
    } else {
        let bars = initial_unique.min(max_fit);
        (bars, bars)
    };

    let area_h = v.rows.saturating_sub(FOOTER_HEIGHT);

    // Stack buffers: no per-frame heap allocation for heights.
    let mut bar_h = [0.0f32; crate::dsp::MAX_BARS];
    let mut cap_rows = [0usize; crate::dsp::MAX_BARS];
    // Spectrum index for a display bar: mirrored around the center
    // in Symmetric mode, identity in LeftToRight.
    let spec_idx = |d: usize| {
        if symmetric {
            spectrum_index(d, n_unique).min(n_unique.saturating_sub(1))
        } else {
            d
        }
    };
    for d in 0..n_bars {
        let s = spec_idx(d);
        if let Some(b) = v.bars.get(s) {
            bar_h[d] = (b * area_h as f32).clamp(0.0, area_h as f32);
        }
        if let Some(c) = v.peaks.get(s) {
            cap_rows[d] = ((c * area_h as f32).round() as usize).min(area_h);
        }
    }

    let width = n_bars * v.bar_width + n_bars.saturating_sub(1) * v.gap_width;
    let pad_x = v.cols.saturating_sub(width) / 2;
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    for row in (0..area_h).rev() {
        for _ in 0..pad_x {
            frame.push(' ');
        }
        // Per-bar colors need a reset between bars; Height and Mono modes are uniform
        // per row so a single prefix suffices, Frequency varies per bar.
        let uniform_row = matches!(v.color_mode, ColorMode::Height | ColorMode::Mono);
        if v.use_color && uniform_row {
            frame.push_str(v.colors.bar_color(row, area_h, 0, n_bars, v.color_mode));
        }
        for i in 0..n_bars {
            // Frequency colors follow the spectrum (bass = center in
            // Symmetric mode), so mirrored bars share the same color.
            let s = spec_idx(i);
            let (color_idx, color_n) = if symmetric {
                (s, n_unique)
            } else {
                (i, n_bars)
            };
            if v.use_color && !uniform_row {
                frame.push_str(
                    v.colors
                        .bar_color(row, area_h, color_idx, color_n, v.color_mode),
                );
            }
            // Integer part is full blocks, the fraction is 1/8-blocks.
            let fh = bar_h[i];
            let full = fh.floor() as usize;
            let frac = fh - full as f32;
            let has_peak = cap_rows[i] == row + 1 && cap_rows[i] <= area_h && row >= full;
            if row < full {
                for _ in 0..v.bar_width {
                    frame.push('█');
                }
            } else if row == full {
                let idx = ((frac * 8.0).round() as usize).min(8);
                let ch = PARTS[idx];
                if ch == ' ' {
                    if has_peak {
                        push_peak(frame, v.bar_width, v.use_color, v.colors.peak());
                        if v.use_color {
                            frame.push_str(v.colors.bar_color(
                                row,
                                area_h,
                                color_idx,
                                color_n,
                                v.color_mode,
                            ));
                        }
                    } else {
                        for _ in 0..v.bar_width {
                            frame.push(' ');
                        }
                    }
                } else {
                    for _ in 0..v.bar_width {
                        frame.push(ch);
                    }
                }
            } else if has_peak {
                push_peak(frame, v.bar_width, v.use_color, v.colors.peak());
                if v.use_color {
                    // Peak uses its own color; restore the bar color after it.
                    frame.push_str(v.colors.bar_color(
                        row,
                        area_h,
                        color_idx,
                        color_n,
                        v.color_mode,
                    ));
                }
            } else {
                for _ in 0..v.bar_width {
                    frame.push(' ');
                }
            }
            if v.gap_width > 0 && i + 1 < n_bars {
                if v.use_color {
                    frame.push_str("\x1b[0m");
                    push_spaces(frame, v.gap_width);
                    if uniform_row {
                        frame.push_str(v.colors.bar_color(row, area_h, 0, n_bars, v.color_mode));
                    }
                } else {
                    push_spaces(frame, v.gap_width);
                }
            } else if v.use_color && !uniform_row {
                frame.push_str("\x1b[0m");
            }
        }
        if v.use_color {
            frame.push_str("\x1b[0m");
        }
        // Clear leftovers when shrinking to a narrower frame.
        frame.push_str("\x1b[K\r\n");
    }
    // Footer without per-frame String allocation: the only formatting is
    // the tiny gain number; status width is counted from parts.
    let gain_text = format!("{:.1}", v.gain);
    let max_text = v.cols.saturating_sub(4);
    let status_w = match v.notice {
        Some(msg) => truncate_chars(msg, max_text).chars().count(),
        None if v.paused => (27
            + v.layout.name().chars().count()
            + 5
            + gain_text.chars().count()
            + 5
            + v.color_mode.name().chars().count())
        .min(v.cols),
        None => (truncate_chars(v.source, 24).chars().count()
            + 5
            + v.theme_name.chars().count()
            + 5
            + v.layout.name().chars().count()
            + 5
            + gain_text.chars().count()
            + 5
            + v.color_mode.name().chars().count())
        .min(v.cols),
    };
    let keys = "T-themes S-source Space-pause L-layout +/-gain G-reset C-color ?-help Q-quit";
    // Bar rows each end with `\r\n`, so the cursor is already on the first
    // footer line: write status directly, newline only before the keys line.
    push_spaces(frame, v.cols.saturating_sub(status_w) / 2);
    match v.notice {
        Some(msg) => {
            if v.use_color {
                frame.push_str(v.colors.title());
            }
            frame.push_str(truncate_chars(msg, max_text));
            if v.use_color {
                frame.push_str("\x1b[0m");
            }
        }
        None if v.paused => {
            if v.use_color {
                frame.push_str(v.colors.title());
            }
            frame.push_str("Paused");
            if v.use_color {
                frame.push_str("\x1b[0m");
            }
            frame.push_str(" - Space to resume  |  ");
            frame.push_str(v.layout.name());
            frame.push_str("  |  ");
            frame.push_str(&gain_text);
            frame.push_str("x  |  ");
            frame.push_str(v.color_mode.name());
        }
        None => {
            frame.push_str(truncate_chars(v.source, 24));
            frame.push_str("  |  ");
            frame.push_str(v.theme_name);
            frame.push_str("  |  ");
            frame.push_str(v.layout.name());
            frame.push_str("  |  ");
            frame.push_str(&gain_text);
            frame.push_str("x  |  ");
            frame.push_str(v.color_mode.name());
        }
    }
    if v.rows >= 2 {
        frame.push_str("\x1b[K\r\n");
        let kw = keys.chars().count().min(v.cols);
        push_spaces(frame, v.cols.saturating_sub(kw) / 2);
        if v.use_color {
            frame.push_str(v.colors.hint());
        }
        frame.push_str(truncate_chars(keys, v.cols));
        if v.use_color {
            frame.push_str("\x1b[0m");
        }
        frame.push_str("\x1b[K");
    } else {
        frame.push_str("\x1b[K");
    }
}

fn push_peak(frame: &mut String, bar_width: usize, use_color: bool, peak_color: &str) {
    if use_color {
        frame.push_str("\x1b[0m");
        frame.push_str(peak_color);
    }
    for _ in 0..bar_width {
        frame.push('─');
    }
    if use_color {
        frame.push_str("\x1b[0m");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_never_exceeds_terminal_rows() {
        let colors = ColorConfig::default();
        let bars = vec![0.5f32; 38];
        let peaks = vec![0.8f32; 38];
        for (cols, rows) in [(80, 24), (40, 6), (30, 2), (20, 1)] {
            let mut frame = String::new();
            render_visualizer(
                &mut frame,
                &Visualizer {
                    bars: &bars,
                    peaks: &peaks,
                    cols,
                    rows,
                    bar_width: 1,
                    gap_width: 1,
                    use_color: false,
                    source: "test",
                    theme_name: "Classic",
                    notice: None,
                    color_mode: ColorMode::Height,
                    layout: LayoutMode::Symmetric,
                    paused: false,
                    gain: 1.0,
                    colors: &colors,
                },
            );
            let line_count = frame.split("\r\n").count();
            assert!(
                line_count <= rows,
                "Rendered {line_count} lines for terminal rows {rows}"
            );
        }
    }
}
