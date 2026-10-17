//! Runtime session: the capture worker, solo rerouting, notices and the
//! reconnect policy.
//!
//! `main` owns the frame loop and feeds this module; everything that touches
//! PulseAudio or `pactl` at runtime lives here, so the policy decisions stay
//! separate from the presentation code.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::audio::{self, CaptureSession};
use crate::dsp::Analyzer;
use crate::source::{CaptureTarget, SoloSession, list_playback_streams};
use crate::util::Gain;

const NOTICE_TIME: Duration = Duration::from_secs(4);
/// First retry delay; later attempts add one step up to the cap.
const RECONNECT_BASE: Duration = Duration::from_millis(500);
const RECONNECT_MAX_STEPS: u32 = 4;
/// A solo source that dies this many times in a row is considered gone.
const SOLO_MAX_FAILURES: u32 = 5;
/// How long a worker must stay alive before a failure streak is forgotten.
const HEALTHY_AFTER: Duration = Duration::from_secs(2);
/// Footer label when no monitor could be detected at all.
pub const DEFAULT_DEVICE_LABEL: &str = "mic/default";

/// What the frame loop has to do after the capture worker reported an error.
#[derive(Debug, PartialEq, Eq)]
pub enum Recovery {
    /// Respawn the worker on the same device once the backoff has elapsed.
    Retry,
    /// The solo source is unrecoverable: drop it and use the system mix.
    FallbackToSystem,
}

/// Reconnect policy for a capture worker that keeps dying.
///
/// Pure bookkeeping driven by an explicit `now`, so the policy is unit
/// testable without a sound server: a failure streak, a backoff deadline and
/// the moment the current worker was spawned.
#[derive(Debug)]
pub struct Retry {
    failures: u32,
    deadline: Option<Instant>,
    alive_since: Instant,
}

impl Retry {
    pub fn new(now: Instant) -> Self {
        Self {
            failures: 0,
            deadline: None,
            alive_since: now,
        }
    }

    /// Full reset: a user-driven source switch starts a new streak.
    pub fn reset(&mut self, now: Instant) {
        self.failures = 0;
        self.deadline = None;
        self.alive_since = now;
    }

    /// The retry policy respawned the worker. The streak deliberately
    /// survives: a device that keeps dying has to stay written off.
    pub fn on_spawn(&mut self, now: Instant) {
        self.deadline = None;
        self.alive_since = now;
    }

    /// The worker reported an error and exited.
    pub fn on_error(&mut self, now: Instant, solo_active: bool) -> Recovery {
        self.failures = self.failures.saturating_add(1);
        if solo_active && self.failures > SOLO_MAX_FAILURES {
            self.reset(now);
            return Recovery::FallbackToSystem;
        }
        self.deadline = Some(now + self.backoff());
        Recovery::Retry
    }

    /// Forgets the failure streak once the current worker proved itself.
    ///
    /// Liveness is the honest signal: a worker that exits on a dead device
    /// never gets this far, so a recovering device clears the streak instead
    /// of being written off after a few unlucky attempts.
    pub fn note_health(&mut self, now: Instant, worker_alive: bool) {
        if self.failures > 0
            && worker_alive
            && now.duration_since(self.alive_since) >= HEALTHY_AFTER
        {
            self.failures = 0;
        }
    }

    /// True once the backoff elapsed; consumes the deadline.
    pub fn take_due(&mut self, now: Instant) -> bool {
        match self.deadline {
            Some(at) if now >= at => {
                self.deadline = None;
                true
            }
            _ => false,
        }
    }

    fn backoff(&self) -> Duration {
        // 0.5 s, 1 s, 1.5 s, 2 s, then 2.5 s for every further attempt.
        RECONNECT_BASE * self.failures.min(RECONNECT_MAX_STEPS + 1)
    }
}

/// Everything the renderer needs from the session, borrowed from a single
/// `&mut` call so the frame loop never needs two overlapping borrows.
pub struct View<'a> {
    pub notice: Option<&'a str>,
    pub bars: &'a [f32],
    pub peaks: &'a [f32],
    pub gain: f32,
    pub system_label: &'a str,
}

pub struct App {
    capture: CaptureSession,
    analyzer: Analyzer,
    gain: Gain,
    /// Solo rerouting, dropped (and restored) as a unit.
    solo: Option<SoloSession>,
    /// Device the system mix is captured from; never changed at runtime.
    system_monitor: Option<String>,
    /// Device currently captured; equals `system_monitor` unless solo.
    monitor: Option<String>,
    notice: Option<(String, Instant)>,
    retry: Retry,
    running: Arc<AtomicBool>,
}

impl App {
    /// Starts capturing `device`, or the auto-detected monitor when `None`.
    pub fn new(device: Option<String>, gain: f32, running: Arc<AtomicBool>) -> Self {
        let system_monitor = device.or_else(audio::detect_monitor);
        Self {
            capture: CaptureSession::start(system_monitor.clone(), running.clone()),
            analyzer: Analyzer::new(),
            gain: Gain::with(gain),
            solo: None,
            monitor: system_monitor.clone(),
            system_monitor,
            notice: None,
            retry: Retry::new(Instant::now()),
            running,
        }
    }

    /// Points capture at `target`, keeping PulseAudio state and the failure
    /// streak consistent. Returns the target that is really active: a failed
    /// solo switch falls back to the system mix.
    pub fn select(&mut self, target: &CaptureTarget) -> CaptureTarget {
        // Dropping the session moves the stream back and unloads the modules.
        drop(self.solo.take());
        match target {
            CaptureTarget::System => {
                self.switch_source(None, Instant::now());
                self.notify("Capturing system mix");
                CaptureTarget::System
            }
            CaptureTarget::Stream(app) => match SoloSession::start(app) {
                Ok(session) => {
                    let audible = session.loopback_active();
                    let monitor = Some(session.monitor_source());
                    self.solo = Some(session);
                    self.switch_source(monitor, Instant::now());
                    self.notify(if audible {
                        format!("Capturing {} (still audible)", app.label())
                    } else {
                        format!("Capturing {} (muted: no loopback)", app.label())
                    });
                    target.clone()
                }
                Err(e) => {
                    self.switch_source(None, Instant::now());
                    self.notify(format!("Source switch failed, back to system: {e}"));
                    CaptureTarget::System
                }
            },
        }
    }

    /// Resolves `--source NAME` by sink-input id, app name or stream title and
    /// captures it. Returns the active target; a miss keeps the system mix.
    pub fn select_named(&mut self, want: &str) -> CaptureTarget {
        if want.trim().is_empty() || want.eq_ignore_ascii_case("system") {
            return CaptureTarget::System;
        }
        let needle = want.to_lowercase();
        let apps = list_playback_streams();
        let hit = apps
            .iter()
            .find(|a| {
                a.input.to_string() == want
                    || a.app.to_lowercase().contains(&needle)
                    || a.label().to_lowercase().contains(&needle)
            })
            .cloned();
        match hit {
            Some(app) => self.select(&CaptureTarget::Stream(app)),
            None => {
                self.notify(format!("Source '{want}' not playing, using system"));
                CaptureTarget::System
            }
        }
    }

    /// Advances the reconnect policy: reports a dead worker, retries it with
    /// backoff, and falls back to the system mix when solo capture is gone.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(err) = self.capture.take_error() {
            match self.retry.on_error(now, self.solo.is_some()) {
                Recovery::Retry => self.notify(format!("Audio lost ({err}) - retrying...")),
                Recovery::FallbackToSystem => {
                    // `select` notifies as well, so the reason goes after it.
                    self.select(&CaptureTarget::System);
                    self.notify(format!("App source gone ({err}), back to system"));
                }
            }
        }
        self.retry.note_health(now, self.capture.worker_alive());
        if self.retry.take_due(now) {
            self.restart_capture();
        }
    }

    /// Feeds the analyzer with the newest samples; `bar_count` is the number of
    /// spectrum bands the layout currently needs.
    pub fn analyze(&mut self, bar_count: usize, dt: f32) {
        self.analyzer
            .update(self.capture.queue(), bar_count, dt, self.gain.value());
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), Instant::now()));
    }

    pub fn gain(&self) -> f32 {
        self.gain.value()
    }

    pub fn gain_up(&mut self) {
        self.gain.up();
        self.notify(format!("Sensitivity {:.2}x", self.gain.value()));
    }

    pub fn gain_down(&mut self) {
        self.gain.down();
        self.notify(format!("Sensitivity {:.2}x", self.gain.value()));
    }

    pub fn gain_reset(&mut self) {
        self.gain.reset();
        self.notify("Sensitivity 1.00x");
    }

    /// Snapshot for the renderer; expires the notice once it timed out.
    pub fn view(&mut self) -> View<'_> {
        let now = Instant::now();
        if let Some((_, at)) = &self.notice
            && now.duration_since(*at) >= NOTICE_TIME
        {
            self.notice = None;
        }
        View {
            notice: self.notice.as_ref().map(|(msg, _)| msg.as_str()),
            bars: self.analyzer.bars(),
            peaks: self.analyzer.peaks(),
            gain: self.gain.value(),
            system_label: self
                .system_monitor
                .as_deref()
                .unwrap_or(DEFAULT_DEVICE_LABEL),
        }
    }

    /// Replaces the capture worker for the device that is currently selected.
    fn restart_capture(&mut self) {
        let now = Instant::now();
        self.capture.switch(self.monitor.clone(), &self.running);
        self.retry.on_spawn(now);
    }

    /// User-driven switch to `monitor`: unlike a retry this also starts a new
    /// failure streak.
    fn switch_source(&mut self, monitor: Option<String>, now: Instant) {
        self.monitor = monitor;
        self.restart_capture();
        self.retry.reset(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed reference point so the policy can be tested against exact
    /// deadlines instead of sleeping.
    fn base() -> Instant {
        std::time::Instant::now()
    }

    fn at(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    fn after(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    /// Waits out the pending retry deadline on a fake clock and reports how
    /// long it took.
    fn wait_due(r: &mut Retry, base: Instant) -> Duration {
        let mut waited = Duration::ZERO;
        while !r.take_due(base + waited) {
            waited += Duration::from_millis(50);
        }
        waited
    }

    #[test]
    fn backoff_grows_then_plateaus() {
        let base = base();
        let mut r = Retry::new(base);
        let mut delays = Vec::new();
        for _ in 0..8 {
            assert_eq!(r.on_error(base, false), Recovery::Retry);
            delays.push(wait_due(&mut r, base));
        }
        assert_eq!(
            delays,
            vec![
                Duration::from_millis(500),
                Duration::from_millis(1000),
                Duration::from_millis(1500),
                Duration::from_millis(2000),
                Duration::from_millis(2500),
                Duration::from_millis(2500),
                Duration::from_millis(2500),
                Duration::from_millis(2500),
            ]
        );
    }

    #[test]
    fn a_retry_is_due_only_after_the_backoff() {
        let base = base();
        let mut r = Retry::new(base);
        assert_eq!(r.on_error(base, false), Recovery::Retry);
        assert!(!r.take_due(base));
        assert!(!r.take_due(after(base, 499)));
        assert!(r.take_due(after(base, 500)));
        // The deadline is consumed, so the loop does not respawn twice.
        assert!(!r.take_due(at(base, 10)));
    }

    #[test]
    fn solo_gives_up_after_repeated_failures() {
        let base = base();
        let mut r = Retry::new(base);
        for i in 0..SOLO_MAX_FAILURES {
            assert_eq!(r.on_error(at(base, i as u64), true), Recovery::Retry);
            // The streak grows, so each retry waits half a second longer.
            assert_eq!(
                wait_due(&mut r, at(base, i as u64)),
                Duration::from_millis(500 * (i as u64 + 1))
            );
        }
        assert_eq!(r.on_error(at(base, 9), true), Recovery::FallbackToSystem);
        // The fallback also resets the streak, so a second dead app still gets
        // its own set of attempts: the first delay applies again.
        assert_eq!(r.on_error(at(base, 10), true), Recovery::Retry);
        assert_eq!(wait_due(&mut r, at(base, 10)), Duration::from_millis(500));
    }

    #[test]
    fn system_mix_is_retried_forever() {
        let base = base();
        let mut r = Retry::new(base);
        for i in 0..50 {
            assert_eq!(r.on_error(at(base, i), false), Recovery::Retry);
        }
    }

    #[test]
    fn a_healthy_worker_clears_the_streak() {
        let base = base();
        let streak = |r: &mut Retry| {
            for i in 0..SOLO_MAX_FAILURES {
                r.on_error(at(base, i as u64), true);
            }
        };

        // Staying alive long enough resets the streak instead of falling back.
        let mut healthy = Retry::new(base);
        streak(&mut healthy);
        healthy.on_spawn(at(base, 30));
        healthy.note_health(at(base, 32), true);
        assert_eq!(healthy.on_error(at(base, 33), true), Recovery::Retry);
        assert_eq!(
            wait_due(&mut healthy, at(base, 33)),
            Duration::from_millis(500)
        );

        // A worker that keeps dying never earns the streak reset.
        let mut dead = Retry::new(base);
        streak(&mut dead);
        dead.on_spawn(at(base, 40));
        dead.note_health(at(base, 41), false);
        assert_eq!(
            dead.on_error(at(base, 42), true),
            Recovery::FallbackToSystem
        );

        // A freshly respawned worker has not proven anything yet.
        let mut fresh = Retry::new(base);
        streak(&mut fresh);
        fresh.on_spawn(at(base, 50));
        fresh.note_health(at(base, 51), true);
        assert_eq!(
            fresh.on_error(at(base, 52), true),
            Recovery::FallbackToSystem
        );
    }
}
