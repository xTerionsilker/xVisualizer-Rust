use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::{Block, MenuOutcome, OptionRow, UiEvent, choice_key};

#[derive(Clone, Debug)]
pub struct PlaybackStream {
    pub input: u32,
    pub sink: u32,
    pub app: String,
    pub stream: String,
}

impl PlaybackStream {
    pub fn label(&self) -> String {
        if self.stream.is_empty() || self.stream == self.app {
            self.app.clone()
        } else {
            format!("{} - {}", self.app, self.stream)
        }
    }

    /// Appends the label to `buf` (reused across frames) to keep the render
    /// path free of per-frame allocations.
    fn label_into(&self, buf: &mut String) {
        buf.push_str(&self.app);
        if !self.stream.is_empty() && self.stream != self.app {
            buf.push_str(" - ");
            buf.push_str(&self.stream);
        }
    }
}

#[derive(Clone, Debug)]
pub enum CaptureTarget {
    System,
    Stream(PlaybackStream),
}

impl CaptureTarget {
    // Row 0 is always System, rows 1.. mirror `apps`.
    pub fn menu_index(&self, apps: &[PlaybackStream]) -> usize {
        match self {
            CaptureTarget::System => 0,
            CaptureTarget::Stream(s) => apps
                .iter()
                .position(|a| a.input == s.input)
                .map(|i| i + 1)
                .unwrap_or(0),
        }
    }

    /// Writes the footer label into `buf` (reused across frames).
    pub fn label_into(&self, system_label: &str, buf: &mut String) {
        match self {
            CaptureTarget::System => {
                buf.clear();
                buf.push_str(system_label);
            }
            CaptureTarget::Stream(s) => {
                buf.clear();
                buf.push_str("app: ");
                s.label_into(buf);
            }
        }
    }
}

pub fn list_playback_streams() -> Vec<PlaybackStream> {
    pactl(&["list", "sink-inputs"])
        .map(|out| parse_sink_inputs(&out))
        .unwrap_or_default()
}

fn parse_sink_inputs(out: &str) -> Vec<PlaybackStream> {
    #[derive(Default)]
    struct Raw {
        input: Option<u32>,
        sink: Option<u32>,
        app: Option<String>,
        media: Option<String>,
        binary: Option<String>,
    }
    let mut raws: Vec<Raw> = Vec::new();
    let mut cur: Option<Raw> = None;
    let mut in_props = false;

    for line in out.lines() {
        // `pactl` indents with tabs or spaces depending on version.
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("Sink Input #") {
            if let Some(prev) = cur.take() {
                raws.push(prev);
            }
            cur = Some(Raw {
                input: rest.trim().parse().ok(),
                ..Raw::default()
            });
            in_props = false;
        } else if let Some(r) = cur.as_mut() {
            if t == "Properties:" {
                in_props = true;
            } else if let Some(rest) = t.strip_prefix("Sink:") {
                // Parsed regardless of Properties position; field order
                // differs across pactl/PipeWire versions.
                r.sink = rest.trim().parse().ok();
            } else if in_props && let Some((k, v)) = t.split_once('=') {
                let v = v.trim().trim_matches('"').to_string();
                match k.trim() {
                    "application.name" => r.app = Some(v),
                    "media.name" => r.media = Some(v),
                    "application.process.binary" => r.binary = Some(v),
                    _ => {}
                }
            }
        }
    }
    if let Some(prev) = cur.take() {
        raws.push(prev);
    }

    raws.into_iter()
        .filter_map(|r| {
            let app = r
                .app
                .or(r.binary)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown app".to_string());
            Some(PlaybackStream {
                input: r.input?,
                sink: r.sink?,
                stream: r.media.unwrap_or_default(),
                app,
            })
        })
        .collect()
}

fn pactl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("pactl")
        .args(args)
        .output()
        .map_err(|e| format!("pactl failed to run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "pactl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub struct SourceMenu {
    pub target: CaptureTarget,
    pub apps: Vec<PlaybackStream>,
    pub selected: usize,
    pub open: bool,
}

impl SourceMenu {
    pub fn new() -> Self {
        Self {
            target: CaptureTarget::System,
            apps: Vec::new(),
            selected: 0,
            open: false,
        }
    }

    pub fn open(&mut self) {
        self.apps = list_playback_streams();
        self.selected = self.target.menu_index(&self.apps);
        self.open = true;
    }

    fn refresh(&mut self) {
        self.apps = list_playback_streams();
        let max = self.apps.len();
        self.selected = self.selected.min(max);
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> (MenuOutcome, Option<UiEvent>) {
        if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            self.refresh();
            return (MenuOutcome::StillOpen, None);
        }
        match choice_key(key, &mut self.selected, self.apps.len() + 1) {
            MenuOutcome::Confirmed => {
                self.open = false;
                if self.selected == 0 {
                    self.target = CaptureTarget::System;
                    (MenuOutcome::Confirmed, Some(UiEvent::SelectSystem))
                } else if let Some(app) = self.apps.get(self.selected - 1).cloned() {
                    self.target = CaptureTarget::Stream(app.clone());
                    (MenuOutcome::Confirmed, Some(UiEvent::SelectStream(app)))
                } else {
                    (MenuOutcome::Confirmed, None)
                }
            }
            MenuOutcome::Cancelled => {
                self.selected = self.target.menu_index(&self.apps);
                self.open = false;
                (MenuOutcome::Cancelled, None)
            }
            MenuOutcome::StillOpen => (MenuOutcome::StillOpen, None),
        }
    }
}

/// Rows the full menu spends on chrome: title, hint, blank, `System`, blank,
/// bottom hint (plus one more when the app list is empty).
const CHROME_ROWS: usize = 6;
/// Chrome kept when the terminal is too short for hints: title, `System`, bottom.
const COMPACT_CHROME_ROWS: usize = 3;
const HINT: &str = "Up/Down - select  |  Enter - capture  |  R - refresh";
const NO_APPS: &str = "(no apps playing audio right now)";

/// Visible slice of `apps` for `budget` rows, kept centered on the highlighted
/// entry so a long list scrolls around the cursor instead of pushing the menu
/// off the screen.
fn visible_range(apps: usize, selected: usize, budget: usize) -> std::ops::Range<usize> {
    let budget = budget.min(apps);
    if apps <= budget {
        return 0..apps;
    }
    let cursor = selected.saturating_sub(1).min(apps - 1);
    let first = cursor.saturating_sub(budget / 2).min(apps - budget);
    first..first + budget
}

pub fn render(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &SourceMenu,
    setup: bool,
    use_color: bool,
    colors: &crate::color::ColorConfig,
) {
    let current_input = match &menu.target {
        CaptureTarget::Stream(s) => Some(s.input),
        CaptureTarget::System => None,
    };
    let title: &[&str] = if setup {
        &["Setup 4/4 - Select capture source"]
    } else {
        &["Select capture source"]
    };
    let bottom: &[&str] = if setup {
        &["Esc - skip setup  |  Q - quit"]
    } else {
        &["Esc - back  |  Q - quit"]
    };
    let empty_note = usize::from(menu.apps.is_empty());
    // A terminal too short for the hints keeps only title, options and the
    // bottom line, so the highlighted app stays reachable. In the extreme
    // case the bottom hint gives up its row to the app list.
    let compact = rows < CHROME_ROWS + empty_note + 1;
    let chrome = if compact {
        // Never more than the title plus one option row.
        COMPACT_CHROME_ROWS.min(rows.saturating_sub(1))
    } else {
        CHROME_ROWS + empty_note
    };
    let window = visible_range(menu.apps.len(), menu.selected, rows.saturating_sub(chrome));
    // title, [hint, blank], System, one row per visible app, [note], [blank], bottom
    let lines = window.len() + chrome;
    let last = lines - 1;
    let system_row = if compact { 1 } else { 3 };
    let app_row = system_row + 1;

    let mut block = Block::new(frame, lines, rows);
    for r in 0..rows {
        let Some(line) = block.row(r) else {
            continue;
        };
        match line {
            0 => block.centered(cols, title, use_color, colors.title()),
            i if i == system_row => block.option(
                cols,
                &OptionRow {
                    index: 0,
                    selected: menu.selected,
                    name: "System",
                    detail: "everything you hear",
                    current: current_input.is_none(),
                },
                use_color,
            ),
            1 if !compact => block.centered(cols, &[HINT], use_color, colors.hint()),
            2 if !compact => {}
            // App rows keep their absolute index, so the highlight follows the
            // selection even when the list is windowed.
            i if i >= app_row && i < app_row + window.len() => {
                let idx = window.start + (i - app_row);
                let app = &menu.apps[idx];
                block.option(
                    cols,
                    &OptionRow {
                        index: idx + 1,
                        selected: menu.selected,
                        name: &app.app,
                        detail: &app.stream,
                        current: current_input == Some(app.input),
                    },
                    use_color,
                );
            }
            i if empty_note == 1 && i == app_row => {
                block.centered(cols, &[NO_APPS], use_color, colors.hint());
            }
            i if i == last => block.centered(cols, bottom, use_color, colors.hint()),
            _ => {}
        }
    }
}

static SINK_SEQ: AtomicU32 = AtomicU32::new(0);

/// Active solo capture; restores everything on drop, including on panic.
pub struct SoloSession {
    null_module: u32,
    loop_module: Option<u32>,
    null_sink: String,
    input: u32,
    home_sink: u32,
    done: bool,
}

impl SoloSession {
    pub fn start(stream: &PlaybackStream) -> Result<Self, String> {
        // pid + time + sequence: unique across parallel app instances
        // and rapid successive solo switches.
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let seq = SINK_SEQ.fetch_add(1, Ordering::Relaxed);
        let null_sink = format!("xviz_cap_{}_{}_{}", std::process::id(), uniq, seq);
        let null_module: u32 = pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={null_sink}"),
            "sink_properties=device.description=XVisualizerAppCapture",
        ])
        .and_then(|out| {
            out.trim()
                .parse()
                .map_err(|_| "unexpected load-module output".to_string())
        })
        .map_err(|e| format!("cannot create capture sink: {e}"))?;

        let mut session = Self {
            null_module,
            loop_module: None,
            null_sink: null_sink.clone(),
            input: stream.input,
            home_sink: stream.sink,
            done: false,
        };

        if let Err(e) = pactl(&["move-sink-input", &stream.input.to_string(), &null_sink]) {
            session.shutdown();
            return Err(format!(
                "cannot move app stream (is it still playing?): {e}"
            ));
        }

        // Best effort: without the loopback the app goes mute.
        session.loop_module = pactl(&[
            "load-module",
            "module-loopback",
            &format!("source={null_sink}.monitor"),
            &format!("sink={}", stream.sink),
            "latency_msec=50",
        ])
        .ok()
        .and_then(|out| out.trim().parse().ok());

        Ok(session)
    }

    pub fn monitor_source(&self) -> String {
        format!("{}.monitor", self.null_sink)
    }

    pub fn loopback_active(&self) -> bool {
        self.loop_module.is_some()
    }

    fn shutdown(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        // The stream may already be gone; ignore errors here.
        let _ = pactl(&[
            "move-sink-input",
            &self.input.to_string(),
            &self.home_sink.to_string(),
        ]);
        if let Some(m) = self.loop_module.take() {
            let _ = pactl(&["unload-module", &m.to_string()]);
        }
        let _ = pactl(&["unload-module", &self.null_module.to_string()]);
    }
}

impl Drop for SoloSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Sink Input #471\n\
\tDriver: PipeWire\n\
\tClient: 83\n\
\tSink: 443\n\
\tCorked: no\n\
\tProperties:\n\
\t\tapplication.name = \"WEBRTC VoiceEngine\"\n\
\t\tapplication.process.binary = \"Discord\"\n\
\t\tmedia.name = \"playStream\"\n\
\n\
Sink Input #550\n\
\tSink: 0\n\
\tProperties:\n\
\t\tapplication.process.binary = \"firefox\"\n\
";

    #[test]
    fn parses_sink_inputs() {
        let apps = parse_sink_inputs(SAMPLE);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].input, 471);
        assert_eq!(apps[0].sink, 443);
        assert_eq!(apps[0].app, "WEBRTC VoiceEngine");
        assert_eq!(apps[0].label(), "WEBRTC VoiceEngine - playStream");
        assert_eq!(apps[1].app, "firefox");
        assert_eq!(apps[1].label(), "firefox");
    }

    #[test]
    fn empty_output_gives_no_streams() {
        assert!(parse_sink_inputs("").is_empty());
    }

    #[test]
    fn parses_space_indented_output() {
        let sample = "Sink Input #7\n\
         Driver: PipeWire\n\
         Sink: 1\n\
         Properties:\n\
                 application.name = \"player\"\n\
                 media.name = \"music = loud\"\n";
        let apps = parse_sink_inputs(sample);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].input, 7);
        assert_eq!(apps[0].sink, 1);
        assert_eq!(apps[0].label(), "player - music = loud");
    }

    #[test]
    fn skips_entries_without_sink_or_input() {
        let sample = "Sink Input #x\n\
\tSink: 1\n\
\tProperties:\n\
\t\tapplication.name = \"broken\"\n";
        assert!(parse_sink_inputs(sample).is_empty());
    }

    #[test]
    fn parses_sink_after_properties() {
        let sample = "Sink Input #9\n\
\tProperties:\n\
\t\tapplication.name = \"late\"\n\
\tSink: 3\n";
        let apps = parse_sink_inputs(sample);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].input, 9);
        assert_eq!(apps[0].sink, 3);
        assert_eq!(apps[0].app, "late");
    }

    fn stream(input: u32, app: &str, title: &str) -> PlaybackStream {
        PlaybackStream {
            input,
            sink: 0,
            app: app.to_string(),
            stream: title.to_string(),
        }
    }

    #[test]
    fn source_menu_fits_any_terminal_and_keeps_the_selection_visible() {
        let colors = crate::color::ColorConfig::default();
        let apps: Vec<PlaybackStream> = (0..12)
            .map(|i| {
                stream(
                    i,
                    &format!("app-{i}"),
                    &format!("a fairly long track name {i}"),
                )
            })
            .collect();
        for (cols, rows) in [(80, 24), (60, 12), (30, 10), (20, 7), (20, 4), (14, 3)] {
            for selected in [0, 5, 12] {
                let menu = SourceMenu {
                    target: CaptureTarget::System,
                    apps: apps.clone(),
                    selected,
                    open: true,
                };
                let mut frame = String::new();
                render(&mut frame, cols, rows, &menu, false, false, &colors);
                let lines: Vec<String> = frame.split("\r\n").map(crate::util::strip_ansi).collect();
                assert_eq!(
                    lines.len(),
                    rows,
                    "{cols}x{rows} selected {selected}: {} lines",
                    lines.len()
                );
                for line in &lines {
                    assert!(
                        line.chars().count() <= cols,
                        "{cols}x{rows} row too wide: {line:?}"
                    );
                }
                // Exactly one highlighted row, and it is the selected entry.
                let marks = lines
                    .iter()
                    .filter(|l| l.trim_start().starts_with('>'))
                    .count();
                assert_eq!(marks, 1, "{cols}x{rows} selected {selected}");
                if selected > 0 {
                    let name = apps[selected - 1].app.clone();
                    assert!(
                        lines
                            .iter()
                            .any(|l| { l.trim_start().starts_with('>') && l.contains(&name) }),
                        "selection {name} scrolled out of view at {cols}x{rows}"
                    );
                }
            }
        }
    }

    #[test]
    fn empty_source_list_explains_itself() {
        let colors = crate::color::ColorConfig::default();
        let menu = SourceMenu::new();
        let mut frame = String::new();
        render(&mut frame, 80, 24, &menu, false, false, &colors);
        assert!(frame.contains("no apps playing audio right now"));
        assert_eq!(frame.split("\r\n").count(), 24);
    }

    #[test]
    fn window_stays_inside_the_list() {
        for apps in 0..20usize {
            for selected in 0..=apps {
                for budget in 0..20usize {
                    let w = visible_range(apps, selected, budget);
                    assert!(
                        w.start <= w.end && w.end <= apps,
                        "{w:?} for {apps}/{selected}"
                    );
                    if !w.is_empty() {
                        let cursor = selected.saturating_sub(1);
                        assert!(
                            (w.start..w.end).contains(&cursor),
                            "cursor {cursor} outside {w:?} ({apps} apps, {budget} rows)"
                        );
                    }
                }
            }
        }
    }
}
