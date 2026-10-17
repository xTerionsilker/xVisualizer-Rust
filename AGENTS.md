# AGENTS.md

Single-crate Rust TUI app (`edition 2024`): terminal ASCII spectrum visualizer capturing system audio via PulseAudio monitor. No workspace, no CI, no existing lint/format config.

## Commands

- Build: `cargo build` — `cargo run --release` for real use (debug build works but frame timing/noise differs).
- Verify: `cargo clippy --all-targets` (repo standard is zero warnings), then `cargo test`.
- Format: `cargo fmt --check` (default rustfmt, no custom config).
- Run with args after `--`: `cargo run --release -- --device <monitor> --theme solid --color frequency --layout symmetric --gain 1.5 --source system --no-color --no-setup`.
- Only tests are pure `pactl` output parsers in `src/source.rs`, CLI parsing in `src/args.rs`, config round-trip in `src/config.rs`, the reconnect policy in `src/app.rs`, and pure helpers (`ui`, `util`, `layout`, `theme`, `visualizer`, `source`): `cargo test source` runs the audio-independent subset without hardware.

## Code rules

- Keep code modular by domain: one file = one cohesive domain. Current split: `args` / `audio` / `config` (settings persistence) / `dsp` / `color` (modes + menu) / `theme` (themes + menu) / `layout` (modes + mapping + menu) / `source` (streams + solo + menu) / `ui` (screen state, events, shared option-list screen, `Choice` trait) / `util` (gain, text helpers, frame-row emission) / `input` / `visualizer` / `help` / `terminal` / `app` (runtime session: capture worker, solo rerouting, notices, reconnect policy).
- `main` is the composition root and frame loop only: resolve args/config, enter the terminal, then poll keys → `app.tick()` → analyze → dispatch a screen → one write+flush. Session state (capture, gain, notices, retry streak) belongs to `app::App`; screen state belongs to `ui::UiState`. Feature state lives in the feature's own file; `main` and `app` are the only owners of cross-feature wiring and pass plain values down.
- Small closed option sets (`Theme`, `ColorMode`, `LayoutMode`) implement `ui::Choice`: declare `ALL`, the `(name, detail)` `TABLE` and optional `aliases`, and get `index`, `next`, `name`, `from_str`, `quick_hint` for free. Never re-add per-enum `all()`/`index()`/`name()` boilerplate or hand-written quick-select hints — they drift out of sync.
- Menu screens are drawn through `ui::Block`: reserve the block's natural height, then emit exactly `rows` rows (`Block::row(r)` + one of `centered`/`option`). A menu taller than the terminal is clipped by the block, so never write rows outside that loop — a wrapped or over-long row scrolls the screen at 120 Hz.
- Every text row goes through `util::push_parts`/`Block::centered`/`Block::option` so it is truncated to `cols`; escape sequences are wrapped *around* the text, never inside a truncation budget.
- Comments: minimal but practical — only where the logic is non-obvious (timing constants, PulseAudio quirks, render escape sequences, policy reasoning). No comment per line, no restating what the code says.
- English only: all code, identifiers, comments, CLI output, and docs strictly in English.

## Gotchas

- `src/audio.rs` and solo-capture in `src/source.rs` shell out to `pactl`/`pactl`-managed Pulse/PipeWire and need a live Linux audio stack. Do not try to verify capture, solo rerouting (`SoloSession` null-sink + loopback), or rendering headlessly — there is no mock/fixture for them.
- On Pulse failure `capture_loop` reports the error through the shared slot and exits the worker; `app::tick` retries the same device with backoff and falls back from a dead solo source to the system mix. Never restore the old fatal `running=false` behavior.
- `CaptureSession::switch()` joins the old worker thread and clears the queue before spawning a new one. Keep that order; reusing the queue without clearing it plays stale audio.
- The reconnect streak (`app::Retry`) survives a retry respawn and is only cleared by `note_health` when the worker stays alive for `HEALTHY_AFTER`, or by a full `reset` on a user-driven source switch. A device that keeps dying has to stay written off; a device that recovers must not be.
- `SoloSession` must stay stored in `app::App` for as long as the solo capture is active (including the `--source` startup path): dropping it moves the stream back and unloads the null sink under the running capture.
- Frame loop in `src/main.rs`: single buffered `write` + `flush` per frame at ~120 Hz, `dt` clamped to `0.001–0.05` s. Preserve both when touching timing/smoothing. Reused buffers (`frame`, `status`, `source_label`, key events) keep the hot path allocation-free — do not add per-frame `format!`/`String` in the render path.
- DSP constants: `RATE`/`WINDOW` are defined in `src/audio.rs` and consumed by `src/dsp.rs` (`WINDOW=2048`, `RATE=48000`, `READ_FRAMES=512`, `F_MIN/F_MAX 30Hz–16kHz`, `GAMMA 0.6`); audio chunk sizes in `src/audio.rs` (`fragsize = READ_FRAMES*4`, `QUEUE_CAP=8192`). These are coupled — changing one without the other breaks latency math documented in `README.md`.
- Terminal state is RAII via `terminal::TerminalGuard` + `terminal::enter()` (alternate screen + raw mode). Any early return must go through it or the terminal is left broken.
- Key handling is split: `input.rs` (transport, global visualizer keys, dispatch incl. setup-wizard chaining), per-domain keys/state/render in `theme.rs` / `source.rs` / `color.rs` / `layout.rs`, shared option-list screen in `ui.rs` (`ChoiceMenu` / `Block` / `OptionRow` / `render_choice`), app events and screen-state composition in `ui.rs` (`UiEvent` / `MenuOutcome` / `UiState`), session requests applied in `app.rs`, overlay keys+render in `help.rs`, frames in `visualizer.rs`, bar colors in `color.rs` (`bar_color`), theme geometry in `theme.rs` (`dims`, `fit_bar_count`), shared helpers in `util.rs` (`Gain`, `center_x`, `truncate_chars`, `fit_parts`, `push_parts`, ANSI row constants). Add a theme by extending `theme.rs` + `visualizer.rs` (`PARTS`, footer) together; add a color mode via `color.rs` (`ColorMode` + its `Choice` impl, render, quick-hint comes from `ALL.len()`).
