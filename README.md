# xvisualizer — Rust Terminal Spectrum Visualizer

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-000000?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/Edition-2024-orange?style=for-the-badge&logo=rust&logoColor=white)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![Platform](https://img.shields.io/badge/Platform-Linux-41CD52?style=for-the-badge&logo=tux&logoColor=white)](https://www.kernel.org/)
[![Terminal](https://img.shields.io/badge/Terminal-TUI-41CD52?style=for-the-badge&logo=gnome-terminals&logoColor=white)](https://gnome-terminal.org/)
[![License](https://img.shields.io/badge/License-MIT-b91c1c?style=for-the-badge&logo=open-source-initiative&logoColor=white)](LICENSE)
[![Audio](https://img.shields.io/badge/Audio-PulseAudio%20%7C%20PipeWire-4CAF50?style=for-the-badge&logo=pipewire&logoColor=white)](https://pipewire.org/)

**xvisualizer** is a high-performance, real-time terminal spectrum visualizer for Linux audio playback. It captures the system mix (or a single application) through PulseAudio monitor sources, analyzes it with a low-latency Fast Fourier Transform pipeline, and renders flicker-free spectrum bars with falling peak indicators directly in the terminal.

Current version is **0.1.0**. The whole visual language is built from Unicode block glyphs (`█ ▁▂▃▄▅▆▇`) and box-drawing rules (`─`), so it renders identically on any terminal that speaks UTF-8 and ANSI escapes — no GPU, no image protocol, no external binary.

---

## Table of Contents

- [Features](#features)
- [Preview](#preview)
- [Architecture & Codebase Tour](#architecture--codebase-tour)
  - [Audio Pipeline](#audio-pipeline)
  - [Solo Application Capture](#solo-application-capture)
  - [Rendering Pipeline](#rendering-pipeline)
- [Graceful Degradation](#graceful-degradation)
- [Requirements](#requirements)
  - [Build Dependencies](#build-dependencies)
  - [Runtime Dependencies](#runtime-dependencies)
- [Installation & Build](#installation--build)
  - [1. Build from Source](#1-build-from-source)
  - [2. Install to Cargo Bin Path](#2-install-to-cargo-bin-path)
- [Configuration](#configuration)
  - [Command-Line Options](#command-line-options)
  - [Startup Setup Wizard](#startup-setup-wizard)
  - [Persisted Settings (`config`)](#persisted-settings-config)
  - [External Palettes (`colors.json`)](#external-palettes-colorsjson)
  - [Exit Codes](#exit-codes)
- [Interactive Live Controls](#interactive-live-controls)
- [Capture, Theme and Color Catalog](#capture-theme-and-color-catalog)
  - [Themes Reference](#themes-reference)
  - [Layouts Reference](#layouts-reference)
  - [Color Modes Reference](#color-modes-reference)
- [Verification & Diagnostics](#verification--diagnostics)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [Uninstallation](#uninstallation)
- [License](#license)

---

## Features

↑ [Table of Contents](#table-of-contents)

- **Zero-Configuration Audio Capture:** Automatically detects the default playback sink through `pactl get-default-sink` and attaches to its `.monitor` source. Works with native PulseAudio and PipeWire (`pipewire-pulse`).
- **Targeted Application Capture (Solo Mode):** Visualize a single application without background system noise. The chosen `sink-input` is moved into a private null sink and stays fully audible through a 50 ms `module-loopback`; everything is restored when you switch away or quit.
- **Accurate DSP Pipeline:** A 2048-sample Hann-windowed FFT pooled into logarithmically spaced bands from 30 Hz to 16 kHz, with a γ = 0.6 magnitude curve that keeps quiet passages visible.
- **Physical Bar Ballistics:** Asymmetric smoothing — ~12 ms attack, ~160 ms release — plus peak markers that fall at a constant 1.2 units/s (full scale in ~0.8 s).
- **120 Hz Hard-Timed Engine:** One `write_all` + `flush` per frame, cursor repositioning with `ESC[H`, and `ESC[K` erase-to-end-of-line instead of a screen clear. Delta time is clamped to 1–50 ms so resizes never destabilize the smoothing.
- **Dynamic Resizing:** The band count is derived from the live terminal width (up to 64 bands) and re-derived on every resize; a shrink-until-it-fits loop keeps even a 3-column window valid.
- **Two Bar Themes and Two Layouts:** `Classic` (thin bars with gaps) or `Solid` (joined double-width bars); `Symmetric` (bass in the center) or `Left-Right` (bass on the left).
- **External Color Theming:** Full 24-bit TrueColor (`#RRGGBB`, `#RGB`), 16-color ANSI names, 256-color indices, and a `bold ` modifier — all driven by `colors.json`, with no recompilation.
- **Fault-Tolerant Capture:** The capture worker is restartable. Read errors are reported through a shared slot instead of killing the app; `main` retries the same device with linear backoff and falls back from a dead solo source to the system mix.
- **Clean Terminal Lifecycle:** Alternate screen, raw mode, and hidden cursor are owned by a `TerminalGuard`, restored on normal exit, on I/O error, and from a panic hook.
- **Live Keyboard Control:** Pause, sensitivity, theme, layout, color mode, capture source, and the help overlay are all reachable without a restart, and the current settings are printed in the footer.
- **Standards Compliant:** Honors [NO_COLOR](https://no-color.org/) and the XDG Base Directory specification. Leaves no PulseAudio modules behind.

---

## Preview

↑ [Table of Contents](#table-of-contents)

`Classic` theme, `Symmetric` layout, `Height` color mode, 80×20 terminal, system mix:

```text
                                 ▆     ▂     ▆
                               ▃ █ ▇ ▇ █ ▇ ▇ █ ▃
                             ▃ █ █ █ █ █ █ █ █ █ ▃
               ▂ ▄         ▅ █ █ █ █ █ █ █ █ █ █ ▅         ▄ ▂
 ▄             █ █       ▅ █ █ █ █ █ █ █ █ █ █ █ ▅       █ █             ▄
 █             █ █     ▅ █ █ █ █ █ █ █ █ █ █ █ █ ▅     █ █             █
 █             █ █   ▂ █ █ █ █ █ █ █ █ █ █ █ █ █ █ ▂   █ █             █
 █             █ █ ▂ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ ▂ █ █             █
 █ █           █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █           █ █
 █ █         ▁ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ ▁         █ █
 █ █ ▂     ▁ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ ▁     ▂ █ █
 alsa_output.pci-0000_00_1f.3  |  Classic  |  Symmetric  |  1.0x  |  Height
 T-themes S-source Space-pause L-layout +/-gain G-reset C-color ?-help Q-quit
```

The footer always shows `source | theme | layout | gain | color mode`; transient status (capture switches, audio loss, sensitivity changes) replaces it for four seconds. The line below it is the live key reminder, truncated to the window width when necessary.

---

## Architecture & Codebase Tour

↑ [Table of Contents](#table-of-contents)

`xvisualizer` is a single crate with one dependency direction and no framework — only `crossterm` touches the terminal, only `libpulse-binding` touches the sound server:

```text
xvisualizer/
├── Cargo.toml             # Crate manifest: edition 2024, seven dependencies, no build scripts
├── Cargo.lock             # Committed dependency graph
├── colors.json            # Default palette, overridable at runtime
├── AGENTS.md              # Conventions and gotchas for contributors
└── src/
    ├── main.rs         # Entrypoint: argument/config resolution, terminal setup, frame loop
    ├── app.rs          # Runtime session: capture worker, solo rerouting, notices, reconnect policy
    ├── args.rs         # Zero-dependency CLI parser, help text, exit codes
    ├── audio.rs        # PulseAudio capture worker, monitor auto-detection, RATE/WINDOW/READ_FRAMES
    ├── source.rs       # pactl stream discovery, source menu, SoloSession null-sink rerouting
    ├── dsp.rs          # FFT analysis, Hann window, log band pooling, bar and peak ballistics
    ├── color.rs        # ColorMode, colors.json parser, color-string parser, color menu
    ├── config.rs       # Settings persistence at $XDG_CONFIG_HOME/xvisualizer/config
    ├── theme.rs        # Classic/Solid geometry, bar-count fitting, theme menu
    ├── layout.rs       # Symmetric/Left-Right mapping math, layout menu
    ├── ui.rs           # UiState composition root, UiEvent/MenuOutcome, Choice trait, shared menu screen
    ├── input.rs        # Non-blocking key polling, global keys, setup-wizard chaining
    ├── visualizer.rs   # Spectrum frame assembler, peak rows, footer and key reminder
    ├── help.rs         # Full-screen keybind overlay
    ├── terminal.rs     # TerminalGuard RAII: alternate screen, raw mode, cursor, size
    └── util.rs         # Gain tracker, char-safe truncation, centering, ANSI row helpers
```

Five invariants carry most of the weight:

- **One domain per file, state included.** A feature's state, its key handling, and its rendering live together (`theme.rs`, `layout.rs`, `color.rs`, `source.rs`). No other module reaches into a sibling's fields.
- **`main` is a loop, `app` is a session.** `main.rs` resolves flags, enters the terminal and drives poll → `app.tick()` → analyze → render → write. Everything that touches PulseAudio at runtime (capture worker, solo rerouting, gain, notices, reconnect policy) lives in `app.rs` behind `App::select` / `App::tick` / `App::view`. The policy itself (`app::Retry`) is pure and driven by an explicit `Instant`, so it is unit-tested without a sound server.
- **One table per option set.** `Theme`, `ColorMode` and `LayoutMode` declare `ALL` plus a `(name, detail)` `TABLE` and implement `ui::Choice`, which derives `index`, `next`, `name`, `from_str` and the `1 / N - quick select` hint. Labels, CLI parsing and quick-select keys cannot drift apart.
- **A frame is exactly `rows` rows.** Screens draw through `ui::Block`: it reserves the block height and then emits one row per terminal line, clipping whatever does not fit. Menus that are taller than the window scroll around the cursor instead of pushing themselves off-screen, and every text row is truncated to `cols`, so a wrapped row (which would scroll the screen at 120 Hz) is impossible.
- **Menus return requests, not side effects.** `handle_key` returns a `MenuOutcome` and an optional `UiEvent`; only `app.rs` mutates capture state or restarts the worker. Key handling stays testable and PulseAudio stays out of `ui.rs`.
- **Capture is restartable, not restart-prone.** `CaptureSession::switch()` joins the old worker, *then* clears the queue, *then* spawns the replacement — in that order, because reusing a queue without clearing it plays stale audio. Errors travel through a shared `Option<String>` slot, so a dead device degrades instead of exiting.

`AGENTS.md` documents these rules and the coupled constants (frame timing, DSP parameters, chunk sizes) for future contributors.

### Audio Pipeline

↑ [Table of Contents](#table-of-contents)

```text
  [ PulseAudio / PipeWire ]        pactl: default sink -> <sink>.monitor
              │
              ▼  Stereo S16LE, 48 kHz, fragsize 2048 B (~10.7 ms chunks)
    [ audio::CaptureWorker ]        read -> decode to mono f32 in [-1, 1]
              │
              ▼  Arc<Mutex<VecDeque<f32>>>, capacity 8192 samples (~170 ms)
     [ dsp::Analyzer ]
        ├── newest 2048-sample window, zero-padded at the head
        ├── Hann window weighting
        ├── forward FFT (rustfft 6)
        ├── magnitude / WINDOW, DC bin zeroed
        ├── logarithmic band pooling (30 Hz - 16 kHz, 64 bands resampled to fit)
        ├── gain and gamma (ref 0.03, γ = 0.6)
        └── attack/release inertia + falling peak markers
              │
              ▼
   [ visualizer::render ]           single-buffer frame string, one write + flush per frame
```

| Stage | Specification | Purpose |
|---|---|---|
| **Audio format** | Stereo S16LE, 48,000 Hz | Monitors are usually stereo; mono would capture the left channel only. |
| **Capture chunk** | 512 frames = 2048 bytes (~10.7 ms) | PulseAudio defaults to ~2 s fragments; `fragsize` is set explicitly to keep latency at ~10 ms. |
| **Sample queue** | 8192 samples (~170 ms) | Decouples the blocking reader thread from the render loop; older samples are drained. |
| **Analysis window** | 2048 samples (~42.7 ms) | ~23.4 Hz bin resolution, enough detail for the bass fundamental. |
| **Window function** | Hann | Suppresses spectral leakage between bins. |
| **FFT engine** | `rustfft 6` forward, planned once | Radix-optimized, SIMD-accelerated; the plan and window are computed in `Analyzer::new()`. |
| **Frequency grid** | Logarithmic, 30 Hz – 16,000 Hz | Matches auditory resolution: narrow at the bottom, wide at the top. |
| **Band pooling** | Maximum magnitude per band | Peak-picking keeps transient bass from being averaged away. |
| **Magnitude curve** | `ref 0.03`, γ = 0.6 | Typical full-scale music peaks near 0.01–0.1 here; gamma keeps quiet bands above the floor. |
| **Ballistics** | Attack τ ≈ 12 ms, release τ ≈ 160 ms | Instant response, natural decay — both derived per frame from `dt`. |
| **Peak tracking** | Fall 1.2 units/s (≈ 0.8 s full scale) | Distinct falling rule above each bar. |

The queue depth plus the analysis window bounds end-to-end latency at roughly **60–100 ms**, tight enough to read as synchronized with the sound.

### Solo Application Capture

↑ [Table of Contents](#table-of-contents)

When an application is selected from the `S` source menu or via `--source <name>`, `SoloSession` reroutes it without silencing it:

1. A private null sink is created with `module-null-sink`, named `xviz_cap_<pid>_<nanos>_<seq>` so parallel instances and rapid switches never collide.
2. The application's `sink-input` is moved into that sink. If this fails, the module is unloaded immediately and the session returns an error.
3. A `module-loopback` (50 ms) routes `xviz_cap_*.monitor` back to the application's original sink, so playback stays audible. This step is best effort: if it fails, capture still works and the footer says the app is muted.
4. The capture worker is switched to the private monitor through `CaptureSession::switch()`.
5. On switch, quit, or panic, `Drop for SoloSession` moves the stream back to its original sink and unloads both modules. The session is owned by `app::App` for as long as the capture is active, so it is never dropped early while its monitor is being recorded.

If the source dies later, `app::tick` retries the same device with backoff and, after more than five consecutive failures, drops the session and returns to the system mix. A streak is only forgotten once the new worker has stayed alive for two seconds, so a device that comes back is not written off after a few unlucky attempts.

### Rendering Pipeline

↑ [Table of Contents](#table-of-contents)

1. **Key drain**: `input::poll_keys` reads every pending key with a zero-timeout `poll` into a buffer reused across frames, so a fast burst is applied in full and the render loop is never blocked. `Release` and `Repeat` events are ignored.
2. **Health check and reconnect**: `app::tick` takes a pending worker error from the shared slot, schedules the backoff (500 ms × up to 5), resets the streak once a worker proves itself, and decides the solo-to-system fallback.
3. **Timing and sizing**: `dt` is measured and clamped to `0.001–0.05 s`; the terminal size is read and the frame buffer is grown ahead of very wide windows so a resize does not trigger a reallocation mid-frame.
4. **Analysis**: while unpaused, the bar count is solved from the theme and layout, and `Analyzer::update` refreshes bars and peaks. Pausing keeps the loop and `dt` alive, so resuming is seamless.
5. **Screen dispatch**: the first open screen wins — source, theme, color, layout, help — otherwise the visualizer frame is assembled. Bar heights and peak rows are computed on fixed-size stack arrays, and the status line is built into a reused buffer, so no per-frame heap allocation happens in the hot path.
6. **Atomic write**: the whole frame, including `ESC[H` and one `ESC[K` per row, is written with a single `write_all` + `flush`. The loop then sleeps out the remainder of the 1/120 s budget.

---

## Graceful Degradation

↑ [Table of Contents](#table-of-contents)

`xvisualizer` never draws a broken frame and never dies because audio did. Layout, rendering, and capture each degrade in defined stages:

| Stage | Condition | Result |
| :--- | :--- | :--- |
| 1. Bar-count fit | The window is narrower than the theme's cell width. | `Theme::fit_bar_count` returns fewer bars, reserving a 4-column margin. |
| 2. Extreme-width clamp | Even the computed count does not fit (very narrow panes). | A shrink loop reduces the count one step at a time down to a single bar. |
| 3. Symmetric parity | The fitted width is even. | One bar is dropped so the mirror stays centered on an exact half. |
| 4. Footer removal | The terminal is only one row tall. | The key-reminder line is dropped; the status line remains. |
| 5. Text truncation | The status line, notice, menu row, or key reminder exceeds the width. | `truncate_chars` cuts on char boundaries, so UTF-8 is never split mid-codepoint. The status line is built in a reused buffer, so no row is ever longer than the terminal. |
| 6. Menu clipping | A menu block is taller than the terminal. | `ui::Block` clips the block and pads the rest, so the frame stays exactly `rows` rows. The source menu first shrinks its app list around the highlighted entry, then drops the hint rows; a one-row source terminal gives its last row to the app list. |
| 7. Size-query failure | The terminal size cannot be read. | Rendering falls back to 80×24. |
| 8. Capture retry | The device stops delivering samples. | The same device is retried with backoff while the app keeps running. |
| 9. Solo fallback | A solo source fails repeatedly (app closed, device gone). | The session is dropped, the stream is restored, and the system mix takes over. |

That is why `xvisualizer` on a stock 80×24 terminal renders a full-width spectrum instead of wrapping or scrolling: the bar count is a function of the window, not a constant, and every text row is truncated rather than allowed to overflow.

The fit budgets are computed, not guessed:

| Theme | Bar width | Gap | Cell | Bars at 80 columns (Symmetric / Left-Right) | Drawn width |
| :--- | ---: | ---: | ---: | :---: | ---: |
| `classic` | 1 | 1 | 2 | 37 / 38 | 73 / 75 |
| `solid` | 2 | 0 | 2 | 37 / 38 | 74 / 76 |

In `Symmetric` mode the analyzer is asked for `ceil(fit / 2)` bands and the renderer mirrors them into `2·n − 1` bars, so the spectrum is identical on both sides of the center. Band count is capped at 64 regardless of window size.

---

## Requirements

↑ [Table of Contents](#table-of-contents)

### Build Dependencies

↑ [Table of Contents](#table-of-contents)

- **Rust Toolchain**: Rust `1.85+` with Cargo (edition `2024`). Keep it current with `rustup update`.
- **A C toolchain and PulseAudio headers** — `libpulse-binding` links against `libpulse`, so headers and `pkg-config` are needed to build from source:

| Distribution | Package Installation Command |
| :--- | :--- |
| **Ubuntu / Debian / Pop!_OS** | `sudo apt install libpulse-dev pkg-config build-essential` |
| **Fedora / RHEL** | `sudo dnf install pulseaudio-libs-devel pkg-config gcc` |
| **Arch Linux / Manjaro** | `sudo pacman -S libpulse pkgconf base-devel` |
| **openSUSE** | `sudo zypper install libpulse-devel pkg-config gcc` |
| **Alpine Linux** | `apk add pulseaudio-dev pkgconf build-base` |

### Runtime Dependencies

↑ [Table of Contents](#table-of-contents)

- **Linux with an active sound server**: PulseAudio or PipeWire with the PulseAudio shim (`pipewire-pulse`), which speaks the same protocol and the same `pactl` CLI.
- **`pactl` in `$PATH`**: used for monitor auto-detection, stream discovery, and the solo reroute. It ships with `pulseaudio-utils` / `pipewire-utils`.
- **A terminal** supporting UTF-8 and ANSI escapes. Glyphs used: `█`, `▁ ▂ ▃ ▄ ▅ ▆ ▇`, and `─`. Colors work at every depth: 16-color, 256-color, and 24-bit TrueColor.
- **No other runtime files.** Settings are optional; without them the app starts with built-in defaults.

---

## Installation & Build

↑ [Table of Contents](#table-of-contents)

### 1. Build from Source

↑ [Table of Contents](#table-of-contents)

Clone the repository and build the optimized binary:

```bash
git clone https://github.com/xTerionsilker/xvisualizer-rust.git
cd xvisualizer-rust
cargo build --release
```

The compiled binary is located at `./target/release/xvisualizer`. Build a debug binary with plain `cargo build`, but note that frame timing and I/O noise differ.

### 2. Install to Cargo Bin Path

↑ [Table of Contents](#table-of-contents)

```bash
cargo install --path .
```

This installs `xvisualizer` into `~/.cargo/bin`. Ensure that directory is present in your `$PATH`:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
```

---

## Configuration

↑ [Table of Contents](#table-of-contents)

`xvisualizer` is configured through command-line flags, an optional `config` file, and an optional `colors.json`. Every option except the capture device can also be changed live through the [live controls](#interactive-live-controls); the result is written to `config` on exit.

### Command-Line Options

↑ [Table of Contents](#table-of-contents)

| Option | Short | Arguments | Default | Description |
|---|---|---|---|---|
| `--device` | `-d` | `<NAME>` | *auto-detected* | Explicit PulseAudio capture source (a `<sink>.monitor` name). Overrides auto-detection. |
| `--theme` | `-t` | `<name>` | `classic` | Bar geometry: `classic` or `solid`. |
| `--color` | | `<name>` | `height` | Bar color mode: `height`, `frequency`, or `mono`. |
| `--layout` | | `<name>` | `symmetric` | Spectrum layout: `symmetric` or `left-right`. |
| `--gain` | `-g` | `<X>` | `1.0` | Sensitivity multiplier, `0.2`–`8.0`. |
| `--source` | | `<NAME>` | `system` | Capture the system mix, or one application by case-insensitive name/label fragment. |
| `--colors` | | `<PATH>` | *searched* | Custom color configuration file (alias: `--colors-file`). |
| `--no-setup` | | *none* | | Skip the startup setup wizard. |
| `--no-color` | | *none* | *disabled* | Disable all ANSI color sequences (also enabled by `NO_COLOR` in the environment). |
| `--version` | `-v` | *none* | | Print the version and exit. |
| `--help` | `-h` | *none* | | Print usage and exit. |

> 💡 **Note on CLI Syntax**:
> - Both separated and equals syntax work for the flags that take a value: `-t solid`, `--theme solid`, `--theme=solid`, `-t=solid`.
> - Theme, color, and layout values are case-insensitive and accept aliases (see [Themes](#themes-reference), [Layouts](#layouts-reference), and [Color Modes](#color-modes-reference)).
> - CLI flags override saved configuration values; `--device` is the only setting that is never persisted implicitly — it is saved only when you pass it explicitly.

### Usage Examples

↑ [Table of Contents](#table-of-contents)

```bash
# Launch with defaults (classic bars, height colors, symmetric, gain 1.0, system mix)
xvisualizer

# Solid bars, bass on the left, capture only Spotify, extra sensitivity
xvisualizer --theme solid --layout left-right --source spotify -g 1.5

# Use a custom palette and skip the wizard
xvisualizer --colors /path/to/my-palette.json --no-setup

# Capture a specific monitor sink explicitly
xvisualizer --device alsa_output.pci-0000_00_1f.3.analog-stereo.monitor

# Plain monochrome output for a screen recording, no colors at all
xvisualizer --color mono --no-color
```

### Startup Setup Wizard

↑ [Table of Contents](#table-of-contents)

On the first run — no `config` file **and** no startup flag given — a four-step wizard appears before the spectrum:

| Step | Screen | Options |
| :---: | :--- | :--- |
| 1/4 | Select theme | `Classic` (blocks with gaps), `Solid` (joined blocks) |
| 2/4 | Select bar color | `Height` (gradient by bar height), `Frequency` (gradient by frequency), `Mono` (uniform color) |
| 3/4 | Select layout | `Symmetric` (mirrored, bass in the center), `Left-Right` (bass left, treble right) |
| 4/4 | Select capture source | `System` (everything you hear) or any running playback stream |

`Enter` confirms and advances, `Esc` skips the remainder of the wizard, and `Q` quits. Pressing any startup flag (`--theme`, `--no-setup`, `--gain`, …) also skips it, so scripted launches never block on a menu.

### Persisted Settings (`config`)

↑ [Table of Contents](#table-of-contents)

Settings are written on a clean exit to `$XDG_CONFIG_HOME/xvisualizer/config` (typically `~/.config/xvisualizer/config`), following the XDG Base Directory specification. Lines that are unknown, malformed, or out of range are ignored rather than rejected.

```ini
theme=classic
color=height
layout=symmetric
gain=1.00
# device=alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
```

| Key | Values | Notes |
|---|---|---|
| `theme` | `classic`, `solid` | Bar geometry. |
| `color` | `height`, `frequency`, `mono` | Bar color mode. |
| `layout` | `symmetric`, `leftright` | Written without the dash; both spellings are read back. |
| `gain` | `0.2`–`8.0` | Clamped on load. |
| `device` | `<sink>.monitor` | Optional; only present when `--device` was passed explicitly. |

The existence of a non-empty config file also suppresses the setup wizard on subsequent runs.

### External Palettes (`colors.json`)

↑ [Table of Contents](#table-of-contents)

Bar colors, peak markers, and interface highlights come from an external JSON file, so a new palette never requires a recompile.

**File resolution order**:

1. Path passed explicitly via `--colors <PATH>`.
2. `./colors.json` in the current working directory.
3. `$XDG_CONFIG_HOME/xvisualizer/colors.json`.
4. `~/.config/xvisualizer/colors.json`.
5. The hardcoded built-in palette (also used when an explicitly requested file is missing or invalid — you then see a short notice in the footer).

#### Default `colors.json`

```json
{
  "modes": {
    "height": [
      { "threshold": 0.0, "color": "blue" },
      { "threshold": 0.6, "color": "magenta" },
      { "threshold": 0.85, "color": "bright_magenta" }
    ],
    "frequency": [
      { "threshold": 0.0, "color": "blue" },
      { "threshold": 0.2, "color": "magenta" },
      { "threshold": 0.4, "color": "bright_magenta" },
      { "threshold": 0.6, "color": "bold magenta" },
      { "threshold": 0.8, "color": "bold bright_magenta" }
    ],
    "mono": "bright_magenta"
  },
  "peak": "bright_magenta",
  "ui": {
    "title": "bold magenta",
    "hint": "bright_black"
  }
}
```

`height` stops are matched against `row / area_height`, `frequency` stops against `bar_index / bar_count`, so both gradients follow the live geometry. Any section you omit keeps its built-in value.

#### Supported Color Specifications

Color strings are parsed flexibly:

- **24-bit TrueColor hex:** `"#RRGGBB"` (e.g. `"#7b2cbf"`, `"#3a0ca3"`, `"#4cc9f0"`) or 3-digit shorthand `"#RGB"` (`"#f0f"`).
- **Standard ANSI names:** `black`, `red`, `green`, `yellow`, `blue`, `magenta`/`purple`, `cyan`, `white`. Dashes and spaces are accepted in place of underscores.
- **Bright ANSI names:** `bright_black`/`gray`/`grey`, `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta`/`pink`, `bright_cyan`, `bright_white`.
- **ANSI 256 indices:** a decimal number `0`–`255`, or with the prefix `"ansi:208"`.
- **Bold modifier:** prepend `bold ` to any color name or hex code (`"bold #00ffcc"`, `"bold cyan"`).
- **Shorthand gradients:** in `height` and `frequency`, provide a plain array of colors without thresholds; they are distributed evenly across `0.0`–`1.0`:
  ```json
  "height": ["#03045e", "#0077b6", "#00b4d8", "#90e0ef", "#caf0f8"]
  ```

### Exit Codes

↑ [Table of Contents](#table-of-contents)

| Code | Meaning | Description |
| :---: | :--- | :--- |
| `0` | **Success** | Clean exit by the user (`Q` or `Ctrl+C`), or a `--help` / `--version` request. |
| `1` | **Runtime Error** | Terminal setup failed (alternate screen or raw mode unavailable), or stdout I/O failed. |
| `2` | **CLI Error** | Unknown flag, missing option value, empty device name, out-of-range gain, or an unknown theme/color/layout value. |

Capture failures never change the exit code: the app degrades and keeps running.

---

## Interactive Live Controls

↑ [Table of Contents](#table-of-contents)

While `xvisualizer` runs in raw mode, settings change without a restart. The active keys are printed in the footer, so no external cheat sheet is needed.

### Global Visualizer Keys

| Key | Action | Details |
| :---: | :--- | :--- |
| `Space` / `P` | **Pause / Resume** | Freezes bars and peaks; `dt` keeps flowing so resuming is smooth. |
| `+` / `=` | **Increase sensitivity** | Multiplies by 1.25×, up to 8.0×. |
| `-` / `_` | **Decrease sensitivity** | Divides by 1.25×, down to 0.2×. |
| `G` | **Reset sensitivity** | Back to 1.0×. |
| `L` | **Toggle layout** | Switches between `Symmetric` and `Left-Right`. |
| `C` | **Cycle color mode** | `Height` → `Frequency` → `Mono`. |
| `1` / `2` | **Quick theme switch** | `1`: Classic, `2`: Solid. |
| `T` / `Tab` / `M` / `F2` | **Open theme menu** | Arrow keys open it too and move the highlight in one press. |
| `S` | **Open capture source menu** | System mix or any running playback stream; `R` refreshes the list. |
| `H` / `?` / `F1` | **Toggle help overlay** | Any of `Esc`, `Enter`, `Space`, `?`, `H`, `F1` closes it. |
| `Q` / `Ctrl+C` | **Quit** | Restores the terminal and returns any rerouted audio. |

### Menu Navigation

The same navigation applies to the theme, color, layout, and source menus:

| Key | Action |
| :---: | :--- |
| `Up` / `Down` / `k` / `j` / `Tab` / `BackTab` | Move the selection up or down (wraps around). |
| `Left` / `Right` / `h` / `l` | Move the selection as an alternative axis. |
| `1` – `9` | Jump to an item by index (moves the highlight; `Enter` applies it). |
| `Enter` / `Space` | Confirm and apply the selection. |
| `Esc` | Cancel, close the menu, or skip the remainder of the startup wizard. |
| `R` | *(source menu only)* Re-run `pactl list sink-inputs`. |

While the setup wizard is active, confirming one step chains into the next, and any menu can be opened later with the same keys.

---

## Capture, Theme and Color Catalog

↑ [Table of Contents](#table-of-contents)

### Themes Reference

↑ [Table of Contents](#table-of-contents)

| Theme | Aliases | Bar width | Gap | Description |
| :--- | :--- | ---: | ---: | :--- |
| `classic` | `1` | 1 | 1 | Thin bars separated by a space — the default, best band separation on any width. |
| `solid` | `bold`, `fat`, `2` | 2 | 0 | Joined double-width bars — a continuous, denser wall of spectrum. |

Both themes resolve to the same number of bars at a given width (the cell is 2 columns either way), so switching themes changes density, not resolution.

### Layouts Reference

↑ [Table of Contents](#table-of-contents)

| Layout | Aliases | Mapping | Description |
| :--- | :--- | :--- | :--- |
| `symmetric` | `mirror`, `1` | `2·n − 1` bars mirrored around the center | Bass in the middle, treble at both edges — the default. |
| `left-right` | `leftright`, `left`, `ltr`, `2` | `n` bars, identity mapping | Bass on the left, treble on the right; uses the full analyzer bandwidth. |

`Frequency` coloring follows the spectrum, not the screen: in `Symmetric` mode mirrored bars share a color, because both sides are the same band.

### Color Modes Reference

↑ [Table of Contents](#table-of-contents)

| Mode | Aliases | Gradient driven by | Section in `colors.json` |
| :--- | :--- | :--- | :--- |
| `height` | `1` | Vertical position of the cell in the spectrum area | `modes.height` |
| `frequency` | `freq`, `rainbow`, `2` | Position of the band along the spectrum | `modes.frequency` |
| `mono` | `white`, `3` | Nothing — one color for all bars | `modes.mono` |

The peak marker color (`peak`) and the UI colors (`ui.title`, `ui.hint`) are shared by every mode.

---

## Verification & Diagnostics

↑ [Table of Contents](#table-of-contents)

Verify that your build and environment behave as expected:

```bash
# 1. Confirm the built binary and its version
cargo run --release -- --version

# 2. Review the full flag surface
xvisualizer --help

# 3. Confirm a sound server and the pactl CLI are present
pactl info | head -3
pactl get-default-sink
pactl list short sources | grep monitor

# 4. Confirm truecolor support (a blank value is intentionally rejected)
echo "${COLORTERM:-unset}"   # expect: truecolor | 24bit

# 5. Smoke-test rendering, then quit with Q
xvisualizer --theme classic --color mono

# 6. Check degradation: the same binary in a narrow, short pane
xvisualizer --theme solid --layout left-right --no-color
```

`xvisualizer` reacts to live resizing, so the narrowest case is easiest to verify by dragging the window smaller and watching the bar count drop. Watch the status line as well: capture switches, audio loss, and sensitivity changes are reported there for four seconds.

---

## Troubleshooting

↑ [Table of Contents](#table-of-contents)

| Symptom | Cause | Solution |
|---|---|---|
| `error: unknown flag '…'` | The flag does not exist. | Run `xvisualizer --help`; values must be separated by a space or `=`. |
| `error: unknown theme/color/layout '…'` | The value is not a documented id or alias. | Use the ids from the [catalog](#capture-theme-and-color-catalog); values are case-insensitive. |
| `error: bad gain '…' (need 0.2-8.0)` | Gain is non-numeric or out of range. | Pass a float inside the range, e.g. `-g 1.5`. |
| Flat bars, no spectrum | Nothing is playing, or the capture device is wrong. | Check `pactl list short sink-inputs` for active streams; try `--device <your-sink>.monitor`. |
| Audio notice: `Audio lost (…) - retrying...` | The worker hit repeated read errors. | The same device is retried automatically; verify the monitor name still exists. |
| Audio notice: `App source gone (…), back to system` | The solo stream disappeared — usually the app was closed. | Expected behavior: the system mix takes over. Pick a new app with `S`. |
| `Capturing <app> (muted: no loopback)` | The loopback module could not be loaded, so the app is not routed to your speakers. | Check that `module-loopback` is available to PulseAudio/PipeWire. |
| Bars are tiny or invisible | The gain is too low, or the material is very quiet. | Press `+` (up to 8.0×) or `G` to reset. |
| Bars are pinned at full height | The gain is too high. | Press `-` (down to 0.2×) or `G` to reset. |
| `Colors file '…' not found, using defaults` | The path passed to `--colors` does not exist. | Check the path, or drop the flag to use `./colors.json` or the XDG location. |
| `Colors JSON '…' invalid: …` | The file is not valid JSON for the expected schema. | Fix the syntax; the built-in palette is used meanwhile. |
| Colors look wrong or washed out | The terminal does not advertise 24-bit TrueColor. | Check `COLORTERM=truecolor`, or use ANSI names and the `mono` mode. |
| Everything is monochrome | `NO_COLOR` is set in the environment. | `unset NO_COLOR`, or remove `--no-color`. |
| Garbled bars or rules | The terminal is not UTF-8 capable. | Switch to a UTF-8 terminal such as Alacritty, Kitty, or WezTerm. |
| Frame text is cut off | The window is too narrow for the footer or key reminder. | Text is truncated by design; widen the window to read it in full. |
| The setup wizard reappears every run | No non-empty `config` was written — the process was killed instead of exiting with `Q`. | Exit with `Q`; check that `$XDG_CONFIG_HOME` (or `~/.config/xvisualizer`) is writable. |
| Audio is stuck on the wrong device after quitting | The process was killed (`SIGKILL`, power loss) before `SoloSession::drop` ran. | Move the stream back manually with `pactl move-sink-input <id> <sink>`; unload the leftover `xviz_cap_*` module. |
| Terminal left in raw mode after a crash | Only `SIGKILL`/`SIGSTOP`-class signals bypass the `TerminalGuard`. | Run `reset` or `stty sane` in the affected shell. |
| Frame rate drops under load | `dt` is clamped to 50 ms, so a stalled frame is stretched rather than smoothed wrongly. | Close other heavy processes; the spectrum recovers on the next frames. |

---

## Development

↑ [Table of Contents](#table-of-contents)

### Building and Checking

↑ [Table of Contents](#table-of-contents)

```bash
# Build the debug binary
cargo build

# Check formatting (rustfmt)
cargo fmt --check

# Run the clippy linter (repo standard is zero warnings)
cargo clippy --all-targets

# Execute the test suite
cargo test

# Build the optimized release binary
cargo build --release
```

Locally, the full gate is:

```bash
cargo fmt --check \
  && cargo clippy --all-targets --locked -- -D warnings \
  && cargo test --all-targets --locked
```

### Testing Strategy

↑ [Table of Contents](#table-of-contents)

`cargo test` runs without audio hardware and covers:

- **CLI parsing** (`args.rs`): long, short, and equals syntax; numeric theme aliases; `NO_COLOR`; and every rejection path (unknown flag, unknown value, missing value, out-of-range gain).
- **Settings round-trip** (`config.rs`): save → load equality, missing files, and silently ignored malformed lines.
- **Color parsing** (`color.rs`): hex, 3-digit shorthand, named and bright ANSI names, dashed spellings, `ansi:N` indices, `bold` prefixes, full JSON schema, thresholdless gradients, color resolution by ratio, and malformed (including non-ASCII) color strings degrading to the default instead of panicking.
- **Option tables** (`ui.rs`): `Choice::from_str` round trips for every variant, alias and numeric spelling; the quick-select hint is derived from the option count; one option row carries exactly one `>` marker with and without color.
- **Reconnect policy** (`app.rs`): the backoff grows 0.5 s → 2.5 s and plateaus, a retry fires only after its deadline (and only once), a solo source gives up after five consecutive failures while the system mix is retried forever, and a worker that stays alive forgets the streak while one that keeps dying does not.
- **Layout math** (`layout.rs`): mirror counts, center-is-bass mapping, single-band clamping, and the mirror never indexing outside the band range.
- **Geometry and bounds** (`theme.rs`, `ui.rs`, `source.rs`, `visualizer.rs`, `help.rs`): bar counts and drawn widths over 1–200 columns; every rendered frame and every menu screen emits exactly `rows` rows and no row wider than `cols`; the highlighted source stays visible in a windowed list; the help overlay's options share one left edge.
- **Pure helpers** (`util.rs`, `input.rs`): gain clamping, char-boundary truncation, centering overflow, width budgets shared between centering and printing.

`cargo test source` runs the audio-independent subset as a fast smoke test. There is no mock for PulseAudio: live capture, solo rerouting, and terminal rendering must be verified on a machine with a running sound stack — see the [gotchas in `AGENTS.md`](AGENTS.md) before touching timing constants.

### Dependencies

↑ [Table of Contents](#table-of-contents)

| Crate | Purpose |
| :--- | :--- |
| [`crossterm 0.29`](https://crates.io/crates/crossterm) | Terminal raw mode, alternate screen, key events, and size queries. |
| [`ctrlc 3`](https://crates.io/crates/ctrlc) | `SIGINT` handling so the terminal is restored even when the interrupt arrives as a signal. |
| [`rustfft 6`](https://crates.io/crates/rustfft) | SIMD-accelerated forward FFT for the spectrum analysis. |
| [`libpulse-binding 2`](https://crates.io/crates/libpulse-binding) | PulseAudio protocol types (`BufferAttr`, sample specs). |
| [`libpulse-simple-binding 2`](https://crates.io/crates/libpulse-simple-binding) | The blocking simple capture API used by the worker thread. |
| [`serde 1`](https://crates.io/crates/serde) | Derive-based deserialization of `colors.json`. |
| [`serde_json 1`](https://crates.io/crates/serde_json) | JSON parsing for the palette file. |

`Cargo.lock` is committed. There is no build script and no code generation step.

---

## Uninstallation

↑ [Table of Contents](#table-of-contents)

If `xvisualizer` was installed through Cargo, remove it with:

```bash
cargo uninstall xvisualizer
```

If it was built from source, simply remove the working copy:

```bash
rm -rf /path/to/xvisualizer-rust
```

Delete the optional user files by hand if you want a clean slate:

```bash
rm -f ~/.config/xvisualizer/config
rm -f ~/.config/xvisualizer/colors.json
rm -f ./colors.json
```

`xvisualizer` creates no PulseAudio modules at rest and no cache directories, so no further cleanup is required.

---

## License

↑ [Table of Contents](#table-of-contents)

Distributed under the MIT License. See [`LICENSE`](LICENSE) for more information.

Copyright (c) 2026 xTerionsilker