//! What the pill shows, from the daemon's watch lines. The clock comes in as
//! an argument, so tests drive time.

use crate::ipc::protocol::{DaemonResponse, DaemonState, ModelState, WatchEvent, WatchLine};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Waveform bars, the newest on the right.
pub const BARS: usize = 9;

/// How long an error stays on the pill.
pub const ERROR_SHOWN: Duration = Duration::from_secs(4);

/// The time between frames while something moves.
pub const FRAME: Duration = Duration::from_millis(33);

#[derive(Debug, Clone, PartialEq)]
pub enum Scene {
    Recording {
        elapsed: Duration,
        /// From 0 to 1, the oldest first
        levels: [f32; BARS],
        locked: bool,
        command: bool,
        /// Set once the recording cap is near
        remaining: Option<Duration>,
    },
    /// The take is transcribed or typed; `loading` while the model loads
    Working {
        loading: bool,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Default)]
pub struct View {
    state: Option<DaemonState>,
    model: Option<ModelState>,
    /// The first audio frame of the take, by this process's clock
    since: Option<Instant>,
    stops_at: Option<Instant>,
    levels: VecDeque<f32>,
    smoothed: f32,
    error: Option<(String, Instant)>,
}

impl View {
    pub fn apply(&mut self, line: WatchLine, now: Instant) {
        match line {
            WatchLine::State(response) => self.apply_state(&response, now),
            WatchLine::Level(level) => self.apply_level(level.level),
            WatchLine::Event(WatchEvent::Error { message }) => {
                self.error = Some((message, now + ERROR_SHOWN));
            },
        }
    }

    fn apply_state(&mut self, response: &DaemonResponse, now: Instant) {
        if !response.ok {
            return;
        }
        self.model = response.model_state;
        match response.state {
            Some(DaemonState::Recording {
                elapsed_ms,
                stops_in_ms,
                ..
            }) => {
                if !matches!(self.state, Some(DaemonState::Recording { .. })) {
                    self.since = now.checked_sub(Duration::from_millis(elapsed_ms));
                    self.levels.clear();
                    self.smoothed = 0.0;
                    // A new take replaces an old error.
                    self.error = None;
                }
                self.stops_at = stops_in_ms.map(|ms| now + Duration::from_millis(ms));
            },
            _ => {
                self.since = None;
                self.stops_at = None;
                self.levels.clear();
            },
        }
        self.state = response.state;
    }

    /// Fast attack and slow decay, so speech moves the bars and pauses let
    /// them fall gently.
    fn apply_level(&mut self, level: f32) {
        if !matches!(self.state, Some(DaemonState::Recording { .. })) {
            return;
        }
        let level = level.clamp(0.0, 1.0);
        let weight = if level > self.smoothed { 0.4 } else { 0.15 };
        self.smoothed += (level - self.smoothed) * weight;
        if self.levels.len() == BARS {
            self.levels.pop_front();
        }
        self.levels.push_back(self.smoothed);
    }

    fn bars(&self) -> [f32; BARS] {
        let mut bars = [0.0; BARS];
        let start = BARS - self.levels.len();
        for (bar, level) in bars[start..].iter_mut().zip(&self.levels) {
            *bar = *level;
        }
        bars
    }

    fn error(&self, now: Instant) -> Option<&str> {
        self.error
            .as_ref()
            .filter(|(_, until)| now < *until)
            .map(|(message, _)| message.as_str())
    }

    /// `None` hides the pill. A recording shows first, then an error, then
    /// the work after a take.
    pub fn scene(&self, now: Instant) -> Option<Scene> {
        if let Some(DaemonState::Recording {
            locked, command, ..
        }) = self.state
        {
            return Some(Scene::Recording {
                elapsed: self
                    .since
                    .map_or(Duration::ZERO, |since| now.saturating_duration_since(since)),
                levels: self.bars(),
                locked,
                command,
                remaining: self.stops_at.map(|at| at.saturating_duration_since(now)),
            });
        }
        if let Some(message) = self.error(now) {
            return Some(Scene::Error {
                message: message.to_string(),
            });
        }
        match self.state {
            Some(DaemonState::Transcribing | DaemonState::Inserting | DaemonState::Stopping) => {
                Some(Scene::Working {
                    loading: matches!(self.model, Some(ModelState::Loading | ModelState::Unloaded)),
                })
            },
            _ => None,
        }
    }

    /// How long the loop may sleep before the pill must change on its own:
    /// a frame while something moves, the end of an error, else no limit.
    pub fn next_change(&self, now: Instant) -> Option<Duration> {
        match self.scene(now)? {
            Scene::Recording { .. } | Scene::Working { .. } => Some(FRAME),
            Scene::Error { .. } => self
                .error
                .as_ref()
                .map(|(_, until)| until.saturating_duration_since(now)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::protocol::Level;

    fn state(state: DaemonState) -> WatchLine {
        WatchLine::State(Box::new(DaemonResponse::ok(state)))
    }

    #[test]
    fn a_take_shows_from_its_first_frame_with_its_elapsed_time() {
        let mut view = View::default();
        let t0 = Instant::now();
        view.apply(state(DaemonState::Starting), t0);
        assert_eq!(view.scene(t0), None, "no pill before the first audio");
        view.apply(state(DaemonState::recording(500)), t0);
        let Some(Scene::Recording { elapsed, .. }) = view.scene(t0 + Duration::from_secs(2)) else {
            panic!("a recording shows");
        };
        assert_eq!(elapsed, Duration::from_millis(2500));
        view.apply(state(DaemonState::Idle), t0 + Duration::from_secs(3));
        assert_eq!(view.scene(t0 + Duration::from_secs(3)), None);
        assert_eq!(view.next_change(t0 + Duration::from_secs(3)), None);
    }

    #[test]
    fn the_bars_follow_the_level_and_shift_left() {
        let mut view = View::default();
        let now = Instant::now();
        view.apply(WatchLine::Level(Level { level: 1.0 }), now);
        view.apply(state(DaemonState::recording(0)), now);
        let Some(Scene::Recording { levels, .. }) = view.scene(now) else {
            panic!("a recording shows");
        };
        assert_eq!(levels, [0.0; BARS], "a level before the take is ignored");
        for _ in 0..BARS + 3 {
            view.apply(WatchLine::Level(Level { level: 1.0 }), now);
        }
        let Some(Scene::Recording { levels, .. }) = view.scene(now) else {
            panic!("a recording shows");
        };
        assert!(
            levels.windows(2).all(|pair| pair[0] <= pair[1]),
            "{:?}",
            levels
        );
        assert!(levels[BARS - 1] > 0.9);
        view.apply(WatchLine::Level(Level { level: 0.0 }), now);
        let Some(Scene::Recording { levels: after, .. }) = view.scene(now) else {
            panic!("a recording shows");
        };
        assert!(after[BARS - 1] > 0.5, "the decay is slow");
    }

    #[test]
    fn badges_and_the_cap_come_from_the_state() {
        let mut view = View::default();
        let now = Instant::now();
        view.apply(
            state(DaemonState::Recording {
                elapsed_ms: 0,
                locked: true,
                command: true,
                stops_in_ms: Some(60_000),
            }),
            now,
        );
        let Some(Scene::Recording {
            locked,
            command,
            remaining,
            ..
        }) = view.scene(now + Duration::from_secs(15))
        else {
            panic!("a recording shows");
        };
        assert!(locked && command);
        assert_eq!(remaining, Some(Duration::from_secs(45)));
    }

    #[test]
    fn an_error_shows_for_a_while_and_a_new_take_replaces_it() {
        let mut view = View::default();
        let now = Instant::now();
        view.apply(
            WatchLine::Event(WatchEvent::Error {
                message: "The microphone did not start".into(),
            }),
            now,
        );
        assert_eq!(
            view.scene(now + Duration::from_secs(1)),
            Some(Scene::Error {
                message: "The microphone did not start".into()
            })
        );
        assert_eq!(
            view.next_change(now + Duration::from_secs(1)),
            Some(Duration::from_secs(3))
        );
        assert_eq!(view.scene(now + ERROR_SHOWN), None);

        view.apply(
            WatchLine::Event(WatchEvent::Error {
                message: "Undo failed".into(),
            }),
            now,
        );
        view.apply(state(DaemonState::recording(0)), now);
        view.apply(state(DaemonState::Idle), now);
        assert_eq!(view.scene(now), None, "the take replaced the error");
    }

    #[test]
    fn the_work_after_a_take_names_a_model_that_loads() {
        let mut view = View::default();
        let now = Instant::now();
        view.apply(state(DaemonState::Transcribing), now);
        assert_eq!(view.scene(now), Some(Scene::Working { loading: false }));
        let loading = DaemonResponse::ok(DaemonState::Transcribing)
            .with_model_state(Some(ModelState::Loading));
        view.apply(WatchLine::State(Box::new(loading)), now);
        assert_eq!(view.scene(now), Some(Scene::Working { loading: true }));
        assert_eq!(view.next_change(now), Some(FRAME));
    }

    #[test]
    fn a_failed_reply_changes_nothing() {
        let mut view = View::default();
        let now = Instant::now();
        view.apply(state(DaemonState::recording(0)), now);
        view.apply(
            WatchLine::State(Box::new(DaemonResponse::error("nope"))),
            now,
        );
        assert!(matches!(view.scene(now), Some(Scene::Recording { .. })));
    }
}
