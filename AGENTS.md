# AGENTS.md

Single-crate Rust TUI app (`edition 2024`): terminal ASCII spectrum visualizer capturing system audio via PulseAudio monitor. No workspace, no CI, no existing lint/format config.

## Commands

- Build: `cargo build` — `cargo run --release` for real use (debug build works but frame timing/noise differs).
- Verify: `cargo clippy --all-targets` (repo standard is zero warnings), then `cargo test`.
- Format: `cargo fmt --check` (default rustfmt, no custom config).
- Run with args after `--`: `cargo run --release -- --device <monitor> --theme solid --color frequency --layout symmetric --gain 1.5 --source system --no-color --no-setup`.
- Only tests are pure `pactl` output parsers in `src/source.rs`, CLI parsing in `src/args.rs`, config round-trip in `src/config.rs`, and pure helpers (`util`, `layout`, `theme`): `cargo test source` runs the audio-independent subset without hardware.

## Code rules

- Keep code modular by domain: one file = one cohesive domain (state + keys + render live together). Current split: `args` / `audio` / `config` (settings persistence) / `dsp` / `color` (modes + menu) / `theme` (themes + menu) / `layout` (modes + mapping + menu) / `source` (streams + solo + menu) / `ui` (screen state, events, shared option-list screen) / `util` (gain, text helpers) / `input` / `visualizer` / `help` / `terminal`. Feature state lives in the feature's own file (`main` owns it and passes plain values down); other modules only call that file's public API.
- Comments: minimal but practical — only where the logic is non-obvious (timing constants, PulseAudio quirks, render escape sequences). No comment per line, no restating what the code says.
- English only: all code, identifiers, comments, CLI output, and docs strictly in English.

## Gotchas

- `src/audio.rs` and solo-capture in `src/source.rs` shell out to `pactl`/`pactl`-managed Pulse/PipeWire and need a live Linux audio stack. Do not try to verify capture, solo rerouting (`SoloSession` null-sink + loopback), or rendering headlessly — there is no mock/fixture for them.
- On Pulse failure `capture_loop` reports the error through the shared slot and exits the worker; `main` retries the same device with backoff and falls back from a dead solo source to the system mix. Never restore the old fatal `running=false` behavior.
- `CaptureSession::switch()` joins the old worker thread and clears the queue before spawning a new one. Keep that order; reusing the queue without clearing plays stale audio.
- Frame loop in `src/main.rs`: single buffered `write` + `flush` per frame at ~120 Hz, `dt` clamped to `0.001–0.05` s. Preserve both when touching timing/smoothing.
- DSP constants: `RATE`/`WINDOW` are defined in `src/audio.rs` and consumed by `src/dsp.rs` (`WINDOW=2048`, `RATE=48000`, `READ_FRAMES=512`, `F_MIN/F_MAX 30Hz–16kHz`, `GAMMA 0.6`); audio chunk sizes in `src/audio.rs` (`fragsize = READ_FRAMES*4`, `QUEUE_CAP=8192`). These are coupled — changing one without the other breaks latency math documented in `README.md`.
- Terminal state is RAII via `terminal::TerminalGuard` + `terminal::enter()` (alternate screen + raw mode). Any early return must go through it or the terminal is left broken.
- Key handling is split: `input.rs` (transport, global visualizer keys, dispatch incl. setup-wizard chaining), per-domain keys/state/render in `theme.rs` / `source.rs` / `color.rs` / `layout.rs`, shared option-list screen in `ui.rs` (`ChoiceMenu` / `render_choice`), app events and screen-state composition in `ui.rs` (`UiEvent` / `MenuOutcome` / `UiState`), overlay keys+render in `help.rs`, frames in `visualizer.rs`, bar colors in `color.rs` (`bar_color`), theme geometry in `theme.rs` (`dims`), shared helpers in `util.rs` (`Gain`, `center_x`, `truncate_chars`). Add a theme by extending `theme.rs` + `visualizer.rs` (`PARTS`, footer) together; add a color mode via `color.rs` (`ColorMode`, `bar_color`, render, quick-hint).
