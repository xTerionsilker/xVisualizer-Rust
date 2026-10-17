//! Entrypoint: argument/config resolution, terminal setup and the frame loop.
//! Runtime session state (capture worker, solo rerouting, reconnect policy)
//! lives in `app`; every screen renders itself from plain values passed down.

mod app;
mod args;
mod audio;
mod color;
mod config;
mod dsp;
mod help;
mod input;
mod layout;
mod source;
mod terminal;
mod theme;
mod ui;
mod util;
mod visualizer;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::App;
use crate::layout::LayoutMode;
use crate::source::CaptureTarget;
use crate::ui::{Choice, UiEvent, UiState};

/// Frame budget of the render loop (~120 Hz).
const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 120);

fn main() -> std::io::Result<()> {
    let parsed = args::parse_args();

    let running = Arc::new(AtomicBool::new(true));
    let stop = running.clone();
    let _ = ctrlc::set_handler(move || {
        stop.store(false, Ordering::SeqCst);
    });

    let (colors, color_err) = color::ColorConfig::load(parsed.colors_file.as_deref());
    let cfg = config::load();
    let theme = parsed.theme.or(cfg.theme).unwrap_or(theme::Theme::Classic);
    let color = parsed.color.or(cfg.color).unwrap_or_default();
    let layout_mode = parsed.layout.or(cfg.layout).unwrap_or_default();
    let gain = parsed.gain.or(cfg.gain).unwrap_or(1.0);
    // Only an explicit device is persisted; the auto-detected monitor is not.
    let device = parsed.device.clone().or(cfg.device);
    // Any explicit preference means the user already chose: no wizard.
    let skip_setup = parsed.no_setup
        || cfg.found
        || parsed.theme.is_some()
        || parsed.color.is_some()
        || parsed.layout.is_some()
        || parsed.source.is_some()
        || parsed.device.is_some()
        || parsed.colors_file.is_some()
        || parsed.gain.is_some();

    let mut app = App::new(device.clone(), gain, running.clone());
    if let Some(msg) = color_err {
        app.notify(msg);
    }
    let mut ui = UiState::with_initial(theme, color, layout_mode, !skip_setup);

    // `--source` preselects one app by sink-input id or name fragment.
    if let Some(want) = parsed.source.as_deref() {
        ui.source.target = app.select_named(want);
    }

    let use_color = !parsed.no_color;
    let mut stdout = std::io::stdout();
    let _guard = terminal::enter(&mut stdout)?;
    // Panics would otherwise leave raw mode / alternate screen on.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        terminal::restore();
        default_hook(info);
    }));

    let mut events: Vec<UiEvent> = Vec::new();
    // Buffers reused every frame: the hot path allocates nothing.
    let mut frame = String::with_capacity(80 * 30);
    let mut status = String::with_capacity(96);
    let mut source_label = String::new();
    let mut prev = Instant::now();

    while running.load(Ordering::SeqCst) {
        for event in input::poll_keys(&mut ui, &running, &mut events) {
            match event {
                UiEvent::SelectSystem => {
                    ui.source.target = app.select(&CaptureTarget::System);
                }
                UiEvent::SelectStream(stream) => {
                    ui.source.target = app.select(&CaptureTarget::Stream(stream.clone()));
                }
                UiEvent::GainUp => app.gain_up(),
                UiEvent::GainDown => app.gain_down(),
                UiEvent::GainReset => app.gain_reset(),
            }
        }
        if !running.load(Ordering::SeqCst) {
            break;
        }

        // Health check and reconnect come first: a dead worker has to be
        // replaced before this frame reads from an empty queue.
        app.tick();

        let frame_start = Instant::now();
        // Clamp dt against resize/lag spikes so smoothing stays stable.
        let dt = frame_start
            .duration_since(prev)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        prev = frame_start;

        let (cols, rows) = terminal::size();
        reserve_frame(&mut frame, cols, rows);

        // Paused freezes bars and peaks; dt keeps updating so resume is smooth.
        if !ui.paused {
            let display_fit = ui.theme.current.fit_bar_count(cols, dsp::MAX_BARS);
            // Symmetric layout mirrors half the bands around the center,
            // left-to-right uses the full width directly.
            let bar_count = match ui.layout.current {
                LayoutMode::Symmetric => layout::unique_count(display_fit),
                LayoutMode::LeftToRight => display_fit.max(1),
            };
            app.analyze(bar_count, dt);
        }

        let view = app.view();
        ui.source
            .target
            .label_into(view.system_label, &mut source_label);
        if ui.source.open {
            source::render(
                &mut frame, cols, rows, &ui.source, ui.setup, use_color, &colors,
            );
        } else if ui.theme.open {
            theme::render(
                &mut frame, cols, rows, &ui.theme, ui.setup, use_color, &colors,
            );
        } else if ui.color.open {
            color::render(
                &mut frame, cols, rows, &ui.color, ui.setup, use_color, &colors,
            );
        } else if ui.layout.open {
            layout::render(
                &mut frame, cols, rows, &ui.layout, ui.setup, use_color, &colors,
            );
        } else if ui.in_help {
            help::render_help(&mut frame, cols, rows, use_color, &colors);
        } else {
            let (bar_width, gap_width) = ui.theme.current.dims();
            visualizer::render_visualizer(
                &mut frame,
                &mut status,
                &visualizer::Visualizer {
                    bars: view.bars,
                    peaks: view.peaks,
                    cols,
                    rows,
                    bar_width,
                    gap_width,
                    use_color,
                    source: &source_label,
                    theme_name: ui.theme.current.name(),
                    notice: view.notice,
                    color_mode: ui.color.current,
                    layout: ui.layout.current,
                    paused: ui.paused,
                    gain: view.gain,
                    colors: &colors,
                },
            );
        }

        stdout.write_all(frame.as_bytes())?;
        stdout.flush()?;

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_TIME {
            thread::sleep(FRAME_TIME - elapsed);
        }
    }

    // Drop the session before saving: it moves the stream back on the way out.
    let final_gain = app.gain();
    drop(app);
    config::save(
        ui.theme.current,
        ui.color.current,
        ui.layout.current,
        final_gain,
        &device,
    );

    Ok(())
}

/// Grows the frame buffer ahead of very wide terminals so a resize does not
/// reallocate in the middle of a frame.
fn reserve_frame(frame: &mut String, cols: usize, rows: usize) {
    let want = cols.saturating_mul(rows).saturating_mul(8).max(80 * 30);
    if frame.capacity() < want {
        frame.reserve_exact(want);
    }
}
