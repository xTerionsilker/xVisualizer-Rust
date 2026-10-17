use std::fmt::Write as _;

use crate::color::{ColorConfig, ColorMode};
use crate::layout::{LayoutMode, display_count, spectrum_index};
use crate::ui::Choice;
#[cfg(test)]
use crate::util::visible_width;
use crate::util::{
    CLEAR_EOL, CURSOR_HOME, NEWLINE, RESET, center_x, parts_width, push_parts, push_spaces,
};

const FOOTER_HEIGHT: usize = 2;
const KEY_HINT: &str =
    "T-themes S-source Space-pause L-layout +/-gain G-reset C-color ?-help Q-quit";
const SEP: &str = "  |  ";
const GAIN_TAIL: &str = "x  |  ";
const PAUSED_HEAD: &str = "Paused - Space to resume  |  ";

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

/// Draws one frame. `status` is a scratch buffer owned by the frame loop and
/// reused every frame, so the footer costs no allocation.
pub fn render_visualizer(frame: &mut String, status: &mut String, v: &Visualizer<'_>) {
    if v.rows == 0 || v.cols == 0 {
        frame.clear();
        return;
    }

    let symmetric = v.layout == LayoutMode::Symmetric;
    // A theme bar can be wider than the terminal itself (Solid on a 1-column
    // pane); clamp the drawn cells so a row can never wrap.
    let bar_width = v.bar_width.min(v.cols);
    let cell = bar_width + v.gap_width;
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

    let width = n_bars * bar_width + n_bars.saturating_sub(1) * v.gap_width;
    let pad_x = v.cols.saturating_sub(width) / 2;
    frame.clear();
    frame.push_str(CURSOR_HOME);
    // Per-bar colors need a reset between bars; Height and Mono modes are
    // uniform per row so a single prefix suffices, Frequency varies per bar.
    let uniform_row = matches!(v.color_mode, ColorMode::Height | ColorMode::Mono);
    for row in (0..area_h).rev() {
        for _ in 0..pad_x {
            frame.push(' ');
        }
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
                for _ in 0..bar_width {
                    frame.push('█');
                }
            } else if row == full {
                let idx = ((frac * 8.0).round() as usize).min(8);
                let ch = PARTS[idx];
                if ch == ' ' {
                    if has_peak {
                        push_peak(frame, bar_width, v.use_color, v.colors.peak());
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
                        for _ in 0..bar_width {
                            frame.push(' ');
                        }
                    }
                } else {
                    for _ in 0..bar_width {
                        frame.push(ch);
                    }
                }
            } else if has_peak {
                push_peak(frame, bar_width, v.use_color, v.colors.peak());
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
                for _ in 0..bar_width {
                    frame.push(' ');
                }
            }
            if v.gap_width > 0 && i + 1 < n_bars {
                if v.use_color {
                    frame.push_str(RESET);
                    push_spaces(frame, v.gap_width);
                    if uniform_row {
                        frame.push_str(v.colors.bar_color(row, area_h, 0, n_bars, v.color_mode));
                    }
                } else {
                    push_spaces(frame, v.gap_width);
                }
            } else if v.use_color && !uniform_row {
                frame.push_str(RESET);
            }
        }
        if v.use_color {
            frame.push_str(RESET);
        }
        // Clear leftovers when shrinking to a narrower frame.
        frame.push_str(CLEAR_EOL);
        frame.push_str(NEWLINE);
    }
    // Bar rows each end with a newline, so the cursor is already on the first
    // footer line: the status line needs no separator of its own.
    push_footer(frame, status, v);
}

/// Status line plus key reminder. The status text is built into the reused
/// buffer and then truncated to the terminal width, so a narrow window can
/// never wrap (a wrapped row would scroll the screen at 120 Hz).
fn push_footer(frame: &mut String, status: &mut String, v: &Visualizer<'_>) {
    status.clear();
    match v.notice {
        Some(msg) => {
            let _ = write!(status, "{msg}");
        }
        None if v.paused => {
            let _ = write!(
                status,
                "{PAUSED_HEAD}{}{SEP}{:.1}{GAIN_TAIL}{}",
                v.layout.name(),
                v.gain,
                v.color_mode.name()
            );
        }
        None => {
            let _ = write!(
                status,
                "{}{SEP}{}{SEP}{}{SEP}{:.1}{GAIN_TAIL}{}",
                v.source,
                v.theme_name,
                v.layout.name(),
                v.gain,
                v.color_mode.name()
            );
        }
    }
    let parts = [status.as_str()];
    push_spaces(frame, center_x(v.cols, parts_width(&parts, v.cols)));
    if v.use_color {
        frame.push_str(v.colors.title());
    }
    push_parts(frame, &parts, v.cols);
    if v.use_color {
        frame.push_str(RESET);
    }
    frame.push_str(CLEAR_EOL);
    // A one-row terminal keeps the status line only: a trailing newline would
    // scroll the screen.
    if v.rows < 2 {
        return;
    }
    frame.push_str(NEWLINE);
    let keys = [KEY_HINT];
    push_spaces(frame, center_x(v.cols, parts_width(&keys, v.cols)));
    if v.use_color {
        frame.push_str(v.colors.hint());
    }
    push_parts(frame, &keys, v.cols);
    if v.use_color {
        frame.push_str(RESET);
    }
    frame.push_str(CLEAR_EOL);
}

fn push_peak(frame: &mut String, bar_width: usize, use_color: bool, peak_color: &str) {
    if use_color {
        frame.push_str(RESET);
        frame.push_str(peak_color);
    }
    for _ in 0..bar_width {
        frame.push('─');
    }
    if use_color {
        frame.push_str(RESET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(v: &Visualizer<'_>) -> String {
        let mut frame = String::new();
        let mut status = String::new();
        render_visualizer(&mut frame, &mut status, v);
        frame
    }

    #[test]
    fn render_never_exceeds_terminal_rows() {
        let colors = ColorConfig::default();
        let bars = vec![0.5f32; 38];
        let peaks = vec![0.8f32; 38];
        for (cols, rows) in [(80, 24), (40, 6), (30, 2), (20, 1)] {
            let frame = draw(&Visualizer {
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
            });
            let line_count = frame.split(NEWLINE).count();
            assert!(
                line_count <= rows,
                "Rendered {line_count} lines for terminal rows {rows}"
            );
        }
    }

    #[test]
    fn no_row_wraps_on_a_narrow_terminal() {
        let colors = ColorConfig::default();
        let bars = vec![0.42f32; 24];
        let peaks = vec![0.9f32; 24];
        for use_color in [false, true] {
            for (cols, rows) in [(120, 30), (80, 24), (40, 12), (28, 8), (20, 5), (12, 3)] {
                for layout in LayoutMode::ALL {
                    for mode in ColorMode::ALL {
                        for notice in [
                            None,
                            Some("Audio lost (device read failed repeatedly) - retrying..."),
                        ] {
                            let frame = draw(&Visualizer {
                                bars: &bars,
                                peaks: &peaks,
                                cols,
                                rows,
                                bar_width: 1,
                                gap_width: 1,
                                use_color,
                                source: "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor",
                                theme_name: "Classic",
                                notice,
                                color_mode: *mode,
                                layout: *layout,
                                paused: notice.is_none(),
                                gain: 8.0,
                                colors: &colors,
                            });
                            let line_count = frame.split(NEWLINE).count();
                            assert!(
                                line_count <= rows,
                                "{cols}x{rows} {layout:?}/{mode:?} drew {line_count} lines"
                            );
                            for row in frame.split(NEWLINE) {
                                assert!(
                                    visible_width(row) <= cols,
                                    "{cols}x{rows} {layout:?}/{mode:?} row too wide: {row:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn status_line_reports_state() {
        let colors = ColorConfig::default();
        let bars = vec![0.5f32; 8];
        let peaks = vec![0.5f32; 8];
        let frame = draw(&Visualizer {
            bars: &bars,
            peaks: &peaks,
            cols: 80,
            rows: 24,
            bar_width: 1,
            gap_width: 1,
            use_color: false,
            source: "app: Spotify - track",
            theme_name: "Solid",
            notice: None,
            color_mode: ColorMode::Mono,
            layout: LayoutMode::LeftToRight,
            paused: false,
            gain: 2.5,
            colors: &colors,
        });
        assert!(frame.contains("app: Spotify - track"));
        assert!(frame.contains("Left-Right"));
        assert!(frame.contains("2.5x"));

        let frame = draw(&Visualizer {
            bars: &bars,
            peaks: &peaks,
            cols: 80,
            rows: 24,
            bar_width: 1,
            gap_width: 1,
            use_color: false,
            source: "app: Spotify - track",
            theme_name: "Solid",
            notice: None,
            color_mode: ColorMode::Mono,
            layout: LayoutMode::LeftToRight,
            paused: true,
            gain: 2.5,
            colors: &colors,
        });
        assert!(frame.contains("Paused - Space to resume"));
        assert!(frame.contains("2.5x"));
    }
}
