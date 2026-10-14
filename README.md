# xvisualizer

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Linux-lightgrey.svg)]()
[![Rust](https://img.shields.io/badge/Rust-1.85%2B%20(Edition%202024)-orange.svg)]()

**xvisualizer** is a high-performance, real-time terminal spectrum visualizer for Linux audio playback. It captures system output through PulseAudio or PipeWire monitor sources, processes the audio using a low-latency Fast Fourier Transform (FFT) pipeline, and renders smooth, flicker-free spectrum bars with falling peak indicators directly in your terminal.

---

## Table of Contents

- [Overview & Highlights](#overview--highlights)
- [System Requirements](#system-requirements)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Command-Line Reference](#command-line-reference)
- [Interactive Controls](#interactive-controls)
- [Configuration](#configuration)
  - [Application Settings (`config`)](#application-settings-config)
  - [Color Schemes (`colors.json`)](#color-schemes-colorsjson)
- [Architecture & Audio Pipeline](#architecture--audio-pipeline)
  - [Signal Processing Pipeline](#signal-processing-pipeline)
  - [Solo Application Capture](#solo-application-capture)
  - [Rendering Engine](#rendering-engine)
- [Project Structure](#project-structure)
- [Development & Testing](#development--testing)
- [License](#license)

---

## Overview & Highlights

- **Zero-Configuration Audio Capture:** Automatically detects the default audio playback sink and attaches to its monitor source. Fully compatible with native PulseAudio and PipeWire (`pipewire-pulse`).
- **Targeted Application Capture (Solo Mode):** Capture audio from a single application or window without capturing background system sounds. The selected stream is dynamically routed through an internal null-sink while remaining fully audible via a low-latency loopback.
- **Accurate DSP Pipeline:** 2048-sample Hann-windowed FFT mapped over a logarithmic scale (30 Hz – 16 kHz).
- **Physical Bar Ballistics:** Asymmetric inertia with rapid attack (~12 ms), smooth release (~160 ms), and linear gravity fall on peak markers (~0.8 s full scale).
- **120 FPS High-Refresh Engine:** Single-buffer cursor repositioning (`ESC[H`) and atomic terminal flush per frame. No full-screen clearing (`clear`), eliminating terminal flicker.
- **Dynamic Resizing:** Bar count automatically adapts to terminal column width (up to 64 discrete frequency bands) with narrow-window safety padding.
- **External Color Theming:** Full RGB TrueColor (24-bit hex `#RRGGBB`), 16-color ANSI, and 256-color palettes configured via `colors.json` without modifying or recompiling the code.
- **Fault-Tolerant Reconnection:** Automatically reconnects with exponential backoff on audio server hiccups; drops cleanly back to the system mix if an application stops playing.
- **Clean Terminal Lifecycle:** Alternate screen buffer with RAII-guaranteed terminal restoration on exit, interrupt (`Ctrl+C`), or unexpected panic.
- **Standards Compliant:** Fully honors the [NO_COLOR](https://no-color.org/) standard and the XDG Base Directory specification.

---

## System Requirements

### Operating System
- **Linux** with an active **PulseAudio** or **PipeWire** sound server.
- The `pactl` utility must be present in `$PATH` (provided by standard sound packages).

### Development Dependencies (Building from Source)
Compiling `xvisualizer` requires C development headers for `libpulse` and `pkg-config`:

| Distribution | Package Installation Command |
|---|---|
| **Ubuntu / Debian / Pop!_OS** | `sudo apt install libpulse-dev pkg-config build-essential` |
| **Fedora / RHEL** | `sudo dnf install pulseaudio-libs-devel pkg-config gcc` |
| **Arch Linux / Manjaro** | `sudo pacman -S libpulse pkgconf base-devel` |
| **openSUSE** | `sudo zypper install libpulse-devel pkg-config` |
| **Alpine Linux** | `apk add pulseaudio-dev pkgconf build-base` |

### Compiler Toolchain
- **Rust 1.85+** (Rust Edition 2024 support is required). Ensure your toolchain is up to date via `rustup update`.

### Terminal Compatibility
- Any terminal supporting UTF-8 and standard ANSI escape sequences.
- Required Unicode block characters: `█`, `▁`, `▂`, `▃`, `▄`, `▅`, `▆`, `▇`, and horizontal bar `─`.

---

## Installation

### From Source

Clone the repository and compile using Cargo:

```bash
git clone https://github.com/xTerionsilker/xvisualizer-rust.git
cd xvisualizer-rust
cargo build --release
```

The optimized binary will be placed at `target/release/xvisualizer`.

### Install to System

To install the binary directly into your Cargo binary path (`~/.cargo/bin`):

```bash
cargo install --path .
```

---

## Quick Start

Launch `xvisualizer`:

```bash
cargo run --release
```

On your first run, an interactive **4-step Setup Wizard** will guide you through:
1. **Theme Selection:** `Classic` (thin bars with spacing) or `Solid` (wide joined bars).
2. **Color Mode:** Gradient by `Height`, gradient by `Frequency`, or `Mono`.
3. **Spectrum Layout:** `Symmetric` (bass centered, treble mirrored) or `Left-Right` (bass on left).
4. **Capture Source:** Entire system mix or an isolated running audio application.

Your selections are saved automatically to `~/.config/xvisualizer/config` and reused on future runs. Press `Q` or `Ctrl+C` at any time to exit.

---

## Command-Line Reference

```text
Usage: xvisualizer [OPTIONS]
```

### Options

| Flag | Parameter | Description |
|---|---|---|
| `-d`, `--device` | `<NAME>` | Explicit PulseAudio capture source (overrides auto-detection). |
| `-t`, `--theme` | `<NAME>` | Initial bar theme: `classic` (`1`) or `solid` (`2`). |
| `--color` | `<NAME>` | Initial bar color mode: `height`, `frequency`, or `mono`. |
| `--layout` | `<NAME>` | Initial spectrum layout: `symmetric` or `left-right`. |
| `-g`, `--gain` | `<X>` | Sensitivity multiplier from `0.2` to `8.0` (default: `1.0`). |
| `--source` | `<NAME>` | Capture source: `system` or case-insensitive app name fragment (e.g. `spotify`). |
| `--colors` | `<PATH>` | Path to a custom JSON color configuration file (default: `./colors.json`). |
| `--no-setup` | *(None)* | Skip the startup setup wizard and go straight to the visualizer. |
| `--no-color` | *(None)* | Disable all ANSI color sequences (also enabled via `NO_COLOR=1`). |
| `-h`, `--help` | *(None)* | Display command help and exit. |

*Note: All options support both `--flag value` and `--flag=value` formats.*

### Examples

```bash
# Capture system mix with Solid theme in Left-to-Right layout
xvisualizer --theme solid --layout left-right

# Isolate audio from Spotify with increased sensitivity
xvisualizer --source spotify -g 1.5

# Use a custom color configuration file
xvisualizer --colors /path/to/my-palette.json

# Capture a specific monitor sink explicitly
xvisualizer --device alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
```

---

## Interactive Controls

All controls are accessible via keyboard shortcuts during visualization:

### Global Visualizer Keys

| Shortcut | Description |
|---|---|
| `Space` / `P` | **Pause / Resume** spectrum analysis and peak markers. |
| `+` / `=` | **Increase sensitivity** (multiplies by 1.25x, up to 8.0x). |
| `-` / `_` | **Decrease sensitivity** (divides by 1.25x, down to 0.2x). |
| `G` | **Reset sensitivity** to baseline (1.0x). |
| `L` | **Toggle layout** between `Symmetric` and `Left-Right`. |
| `C` | **Cycle color mode** (`Height` → `Frequency` → `Mono`). |
| `1` / `2` | **Quick-switch theme** (`1`: Classic, `2`: Solid). |
| `T` / `Tab` / `M` / `F2` | **Open Theme Menu** (arrow keys also trigger). |
| `S` | **Open Capture Source Menu** (switch between System and running apps). |
| `H` / `?` / `F1` | **Open Help Overlay**. |
| `Q` / `Ctrl+C` | **Exit cleanly** (restoring terminal state and audio routing). |

### Menu Navigation

When any menu is open (Theme, Color, Layout, or Source):

| Shortcut | Description |
|---|---|
| `Up` / `Down` / `k` / `j` | Move selection up or down. |
| `Left` / `Right` / `h` / `l` | Move selection (alternate navigation). |
| `1` – `9` | Select item directly by index. |
| `Enter` / `Space` | Confirm and apply selection. |
| `Esc` | Cancel / dismiss menu (or skip remainder of startup wizard). |
| `R` | *(Source menu only)* Refresh the list of running playback streams. |

---

## Configuration

### Application Settings (`config`)

`xvisualizer` persists user preferences across sessions in compliance with the XDG specification:

- Path: `$XDG_CONFIG_HOME/xvisualizer/config` (typically `~/.config/xvisualizer/config`).
- Created automatically after completing the initial setup wizard or on normal exit.

Example configuration file:

```ini
theme=classic
color=height
layout=symmetric
gain=1.00
# device=alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
```

Command-line flags always take precedence over saved configuration values.

---

### Color Schemes (`colors.json`)

All visual styling, bar coloring, peak markers, and interface highlights are governed by an external `colors.json` file.

#### File Resolution Hierarchy
The program searches for color definitions in the following order:
1. Path provided explicitly via `--colors <PATH>`.
2. `./colors.json` in the current working directory.
3. `$XDG_CONFIG_HOME/xvisualizer/colors.json` (`~/.config/xvisualizer/colors.json`).
4. Hardcoded internal fallback palette if no configuration file is located or if JSON is invalid.

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

#### Supported Color Specifications

Color strings are parsed flexibly:
- **24-bit TrueColor Hex:** `"#RRGGBB"` (e.g. `"#7b2cbf"`, `"#3a0ca3"`, `"#4cc9f0"`) or 3-digit shorthand `"#RGB"` (`"#f0f"`).
- **Standard ANSI Names:** `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `purple`, `cyan`, `white`.
- **Bright ANSI Names:** `bright_black` (or `gray`), `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`, `bright_magenta` (or `pink`), `bright_cyan`, `bright_white`.
- **ANSI 256 Indices:** Decimal numbers `0` through `255`, or with prefix `"ansi:208"`.
- **Bold Modifier:** Prepend `bold ` to any color name or hex code (e.g. `"bold #00ffcc"`, `"bold cyan"`).
- **Simple Gradient Arrays:** In `height` and `frequency` modes, you can provide a plain array of colors without manual thresholds; thresholds will automatically distribute evenly between `0.0` and `1.0`:
  ```json
  "height": ["#03045e", "#0077b6", "#00b4d8", "#90e0ef", "#caf0f8"]
  ```

---

## Architecture & Audio Pipeline

```
  [ PulseAudio / PipeWire ]
              │
              ▼ (Stereo S16LE, 48 kHz, fragsize ~10ms)
    [ audio::CaptureWorker ]
              │
              ▼ (Decoded & averaged to Mono f32 in [-1, 1])
     [ Arc<Mutex<Queue>> ] (Capacity: 8192 samples)
              │
              ▼ (Newest 2048-sample window at 120 Hz)
     [ dsp::Analyzer ]
        ├── Hann Window Weighting
        ├── Forward FFT (rustfft)
        ├── Magnitude Spectrum Calculation
        ├── Logarithmic Band Pooling (30 Hz – 16 kHz)
        ├── Gain & Gamma Non-Linearity (γ = 0.6)
        └── Attack/Release Inertia + Falling Peak Markers
              │
              ▼
   [ visualizer::render ]
        ├── Dynamic Terminal Width Allocation
        ├── Symmetric or L-R Mapping
        ├── Single-Buffer Frame String Construction
        └── Atomic Stdout Write + Flush (120 Hz)
```

### Signal Processing Pipeline

| Stage | Specification | Purpose |
|---|---|---|
| **Audio Format** | Stereo S16LE, 48,000 Hz | High-fidelity standardized capture input. |
| **Capture Latency** | 512 frames per chunk (~10.7 ms) | Low-latency buffering via PulseAudio `BufferAttr`. |
| **Analysis Window** | 2048 samples (~42.7 ms) | Provides ~23.4 Hz bin resolution for bass detail. |
| **Window Function** | Hann Window | Suppresses spectral leakage across FFT bins. |
| **FFT Engine** | `rustfft 6` | Optimized radix-4 / AVX-accelerated forward FFT. |
| **Frequency Grid** | Logarithmic, 30 Hz to 16,000 Hz | Accurately models human auditory frequency perception. |
| **Magnitude Curve** | Reference 0.03, Gamma 0.6 | Balances dynamic range; keeps quiet passages visible. |
| **Ballistics** | Attack $\tau \approx 12$ ms, Release $\tau \approx 160$ ms | Punchy instantaneous response with smooth, natural decay. |
| **Peak Tracking** | Fall rate 1.2 units/s (~0.8 s fall) | Distinct falling indicator lines above each bar. |

Total end-to-end latency is typically between **60 ms and 100 ms**, providing tight synchronization with audible sound.

### Solo Application Capture

When an isolated application is selected from the `S` source menu:
1. `xvisualizer` creates a private null sink via `module-null-sink` (`sink_name=xviz_cap_<pid>_<time>_<seq>`).
2. The application's playback stream (`sink-input`) is moved into this private null sink.
3. The recording worker captures the monitor of the private null sink.
4. A loopback module (`module-loopback`, latency 50 ms) is attached to route the private null sink to your actual physical speakers, preserving complete audibility.
5. On stream switch, program exit, or application termination, the stream is seamlessly restored to its original sink and the temporary modules are unloaded.

### Rendering Engine

- **Frame Rate:** Hard-timed 120 Hz rendering loop with delta-time (`dt`) clamped between 1 ms and 50 ms to guarantee smoothing stability during terminal window resize events.
- **Buffer Optimization:** A reusable heap buffer (`String::reserve`) formats the entire terminal frame into memory before issuing a single `stdout.write_all()` and `stdout.flush()`.
- **Character Glyphs:** Full blocks (`█`), 1/8-step sub-block characters (`▁▂▃▄▅▆▇`), and horizontal box-drawing lines (`─`) render high-resolution bar tops and peak markers.

---

## Project Structure

The project is structured into domain-specific modules adhering to clean single-responsibility boundaries:

```text
src/
├── main.rs         # Application entry point, event orchestration, 120 Hz render loop
├── args.rs         # Command-line argument parsing and help presentation
├── audio.rs        # PulseAudio capture worker thread and monitor auto-detection
├── color.rs        # ColorMode definitions, colors.json parser, bar color resolution
├── config.rs       # Persistent configuration loading and saving (~/.config/xvisualizer/config)
├── dsp.rs          # FFT analysis, Hann windowing, log pooling, and bar ballistics
├── help.rs         # Full-screen keybind help overlay rendering and interaction
├── input.rs        # Non-blocking terminal keyboard polling and action dispatching
├── layout.rs       # Symmetric and Left-to-Right bar mappings and layout menu
├── solo.rs         # Per-application null sink rerouting and loopback management
├── source.rs       # Stream discovery via pactl and capture source menu
├── terminal.rs     # Terminal raw mode, alternate screen setup, and RAII restore
├── theme.rs        # Classic and Solid bar geometry definitions and theme menu
├── ui.rs           # Screen state root, menu event dispatching, shared option lists
├── util.rs         # Sensitivity gain tracker, string truncation, text centering
└── visualizer.rs   # Core spectrum frame assembler and footer status renderer
```

---

## Development & Testing

### Verification Suite

All commits adhere to zero-warning standards:

```bash
# Verify code formatting
cargo fmt --check

# Enforce zero warnings across all targets
cargo clippy --all-targets

# Execute test suite
cargo test
```

### Headless vs. Hardware Testing
Pure unit tests (covering CLI arguments, config serialization, color parsers, layout mirror math, text truncation, and `pactl` output parser fixtures) run without audio hardware via `cargo test`. Live audio capture and solo rerouting require an active Linux sound stack.

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

Copyright (c) 2026 xTerionsilker
