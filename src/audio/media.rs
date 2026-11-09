//! Pause the media players while a take records, and resume them after.
//!
//! Players speak MPRIS on the session bus. When a take goes live, Hush pauses
//! each player that plays and keeps its bus name. When the take ends, Hush
//! resumes only those players, and only when they are still paused. A player
//! that the user paused before the take, or stopped during it, stays as it is.
//!
//! The D-Bus calls run on one worker thread, each with a short timeout, so a
//! player that does not answer cannot delay the hotkey or a stop.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::future::Future;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc as async_mpsc;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};
use zbus::zvariant::{DynamicType, OwnedValue};
use zbus::Connection;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
// playerctld sends each call on to the player that played last, so through
// it Hush would pause and resume that player twice.
const PROXY: &str = "org.mpris.MediaPlayer2.playerctld";
const OBJECT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// How long one D-Bus call can take.
pub const CALL_TIMEOUT: Duration = Duration::from_millis(500);
/// How long a shutdown waits for the worker to resume the players.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    Playing,
    Paused,
    Stopped,
}

impl Playback {
    fn parse(status: &str) -> Result<Self> {
        match status {
            "Playing" => Ok(Self::Playing),
            "Paused" => Ok(Self::Paused),
            "Stopped" => Ok(Self::Stopped),
            other => Err(anyhow!("unknown playback status '{}'", other)),
        }
    }
}

/// The media players that Hush can pause.
#[async_trait]
pub trait Players: Send + Sync {
    /// The bus names of the players.
    async fn names(&self) -> Result<Vec<String>>;
    async fn status(&self, name: &str) -> Result<Playback>;
    async fn pause(&self, name: &str) -> Result<()>;
    async fn play(&self, name: &str) -> Result<()>;
}

/// The bus name belongs to a player that Hush controls.
pub fn is_player(name: &str) -> bool {
    name.starts_with(PREFIX) && name != PROXY
}

/// Pause each player that plays. Returns the names of the players paused.
pub async fn pause_playing<P: Players + ?Sized>(players: &P) -> Vec<String> {
    let names = match players.names().await {
        Ok(names) => names,
        Err(e) => {
            warn!("Could not list the media players: {:#}", e);
            return Vec::new();
        },
    };
    let mut paused = Vec::new();
    for name in names {
        match players.status(&name).await {
            Ok(Playback::Playing) => match players.pause(&name).await {
                Ok(()) => {
                    debug!(player = %name, "Paused the media player");
                    paused.push(name);
                },
                Err(e) => debug!(player = %name, "Could not pause the media player: {:#}", e),
            },
            Ok(_) => {},
            Err(e) => debug!(player = %name, "Could not read the media player: {:#}", e),
        }
    }
    paused
}

/// Resume the players in `paused` that are still paused. A player that is
/// gone, or that plays or stopped, stays as it is.
pub async fn resume_paused<P: Players + ?Sized>(players: &P, paused: Vec<String>) {
    if paused.is_empty() {
        return;
    }
    let present = match players.names().await {
        Ok(names) => names,
        Err(e) => {
            warn!("Could not list the media players to resume them: {:#}", e);
            return;
        },
    };
    for name in paused {
        if !present.contains(&name) {
            debug!(player = %name, "The media player is gone");
            continue;
        }
        match players.status(&name).await {
            Ok(Playback::Paused) => match players.play(&name).await {
                Ok(()) => debug!(player = %name, "Resumed the media player"),
                Err(e) => debug!(player = %name, "Could not resume the media player: {:#}", e),
            },
            Ok(status) => debug!(player = %name, ?status, "The media player is not paused now"),
            Err(e) => debug!(player = %name, "Could not read the media player: {:#}", e),
        }
    }
}

/// The players on the session bus. The connection opens at the first call,
/// and a failed connection tries again at the next call.
#[derive(Default)]
pub struct SessionPlayers {
    connection: OnceCell<Connection>,
}

impl SessionPlayers {
    async fn connection(&self) -> Result<&Connection> {
        self.connection
            .get_or_try_init(|| async { within(Connection::session()).await })
            .await
    }

    async fn call<B>(
        &self,
        name: &str,
        interface: &str,
        method: &str,
        body: &B,
    ) -> Result<zbus::Message>
    where
        B: serde::Serialize + DynamicType + Sync,
    {
        let connection = self.connection().await?;
        within(connection.call_method(Some(name), OBJECT, Some(interface), method, body)).await
    }
}

#[async_trait]
impl Players for SessionPlayers {
    async fn names(&self) -> Result<Vec<String>> {
        let connection = self.connection().await?;
        let reply = within(connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        ))
        .await?;
        let names: Vec<String> = reply.body().deserialize()?;
        Ok(names.into_iter().filter(|name| is_player(name)).collect())
    }

    async fn status(&self, name: &str) -> Result<Playback> {
        let reply = self
            .call(name, PROPERTIES, "Get", &(PLAYER, "PlaybackStatus"))
            .await?;
        let value: OwnedValue = reply.body().deserialize()?;
        Playback::parse(&String::try_from(value)?)
    }

    async fn pause(&self, name: &str) -> Result<()> {
        self.call(name, PLAYER, "Pause", &()).await.map(drop)
    }

    async fn play(&self, name: &str) -> Result<()> {
        self.call(name, PLAYER, "Play", &()).await.map(drop)
    }
}

async fn within<T>(call: impl Future<Output = zbus::Result<T>>) -> Result<T> {
    tokio::time::timeout(CALL_TIMEOUT, call)
        .await
        .map_err(|_| anyhow!("no answer in {} ms", CALL_TIMEOUT.as_millis()))?
        // A zbus error already shows its source, which `{:#}` would repeat.
        .map_err(|e| anyhow!("{}", e))
}

/// Connect to the session bus and count the media players on it.
pub async fn probe() -> Result<usize> {
    Ok(SessionPlayers::default().names().await?.len())
}

/// When a take pauses the players, and whether its end resumes them.
#[derive(Debug, Default)]
pub struct PauseTiming {
    not_before: Option<Instant>,
    due: Option<Instant>,
    paused: bool,
}

impl PauseTiming {
    /// A take started at `at`. With `tap_window`, the press can still turn
    /// out to be a tap, which discards the take, so the pause waits until
    /// the window closes.
    pub fn started(&mut self, at: Instant, tap_window: Option<Duration>) {
        self.not_before = Some(at + tap_window.unwrap_or_default());
        self.due = None;
    }

    /// The first audio frame of the take arrived at `at`.
    pub fn live(&mut self, at: Instant) {
        self.due = Some(self.not_before.map_or(at, |not_before| at.max(not_before)));
    }

    /// True once per take, when the players must pause.
    pub fn poll(&mut self, now: Instant) -> bool {
        match self.due {
            Some(due) if now >= due => {
                self.due = None;
                self.paused = true;
                true
            },
            _ => false,
        }
    }

    /// The take ended for any reason. True when the players must resume.
    pub fn ended(&mut self) -> bool {
        let paused = self.paused;
        *self = Self::default();
        paused
    }
}

enum Request {
    Pause,
    Resume,
}

struct Worker {
    requests: async_mpsc::UnboundedSender<Request>,
    /// Disconnects when the worker thread ends.
    done: mpsc::Receiver<()>,
}

/// Pauses the media players during each take. Without `audio.pause_media`,
/// it does nothing.
pub struct MediaPause {
    timing: PauseTiming,
    worker: Option<Worker>,
}

impl MediaPause {
    pub fn new(enabled: bool) -> Self {
        if !enabled {
            return Self::off();
        }
        match Self::with_players(SessionPlayers::default()) {
            Ok(media) => {
                info!("Media players pause while Hush records");
                media
            },
            Err(e) => {
                warn!("Media players will not pause: {:#}", e);
                Self::off()
            },
        }
    }

    fn off() -> Self {
        Self {
            timing: PauseTiming::default(),
            worker: None,
        }
    }

    pub fn with_players<P: Players + 'static>(players: P) -> Result<Self> {
        let (requests, request_rx) = async_mpsc::unbounded_channel();
        let (done_tx, done) = mpsc::channel::<()>();
        thread::Builder::new()
            .name("hush-media".into())
            .spawn(move || {
                let _done = done_tx;
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(serve(players, request_rx)),
                    Err(e) => warn!("Media players will not pause: {}", e),
                }
            })
            .context("Failed to start the media player thread")?;
        Ok(Self {
            timing: PauseTiming::default(),
            worker: Some(Worker { requests, done }),
        })
    }

    /// See [`PauseTiming::started`].
    pub fn started(&mut self, at: Instant, tap_window: Option<Duration>) {
        self.timing.started(at, tap_window);
    }

    pub fn live(&mut self, at: Instant) {
        self.timing.live(at);
    }

    pub fn poll(&mut self, now: Instant) {
        if self.timing.poll(now) {
            self.send(Request::Pause);
        }
    }

    /// Call it as soon as the capture stops, before transcription.
    pub fn ended(&mut self) {
        if self.timing.ended() {
            self.send(Request::Resume);
        }
    }

    fn send(&self, request: Request) {
        let Some(worker) = &self.worker else {
            return;
        };
        if worker.requests.send(request).is_err() {
            debug!("The media player thread has stopped");
        }
    }
}

impl Drop for MediaPause {
    fn drop(&mut self) {
        self.ended();
        if let Some(Worker { requests, done }) = self.worker.take() {
            drop(requests);
            let _ = done.recv_timeout(SHUTDOWN_WAIT);
        }
    }
}

async fn serve<P: Players>(players: P, mut requests: async_mpsc::UnboundedReceiver<Request>) {
    let mut paused: Vec<String> = Vec::new();
    while let Some(request) = requests.recv().await {
        match request {
            Request::Pause => {
                for name in pause_playing(&players).await {
                    if !paused.contains(&name) {
                        paused.push(name);
                    }
                }
            },
            Request::Resume => resume_paused(&players, std::mem::take(&mut paused)).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    const SPOTIFY: &str = "org.mpris.MediaPlayer2.spotify";
    const FIREFOX: &str = "org.mpris.MediaPlayer2.firefox.instance_1_42";
    const VLC: &str = "org.mpris.MediaPlayer2.vlc";
    const TAP: Duration = Duration::from_millis(300);

    #[derive(Default)]
    struct State {
        players: BTreeMap<String, Playback>,
        silent: BTreeSet<String>,
        calls: Vec<String>,
    }

    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<State>>);

    impl Fake {
        fn with(players: &[(&str, Playback)]) -> Self {
            let fake = Self::default();
            for (name, status) in players {
                fake.set(name, *status);
            }
            fake
        }

        fn set(&self, name: &str, status: Playback) {
            self.0.lock().players.insert(name.to_string(), status);
        }

        fn remove(&self, name: &str) {
            self.0.lock().players.remove(name);
        }

        fn silence(&self, name: &str) {
            self.0.lock().silent.insert(name.to_string());
        }

        fn status_of(&self, name: &str) -> Option<Playback> {
            self.0.lock().players.get(name).copied()
        }

        fn calls(&self) -> Vec<String> {
            self.0.lock().calls.clone()
        }

        fn command(&self, verb: &str, name: &str, next: Playback) -> Result<()> {
            let mut state = self.0.lock();
            state.calls.push(format!("{} {}", verb, name));
            if state.silent.contains(name) {
                return Err(anyhow!("no answer"));
            }
            match state.players.get_mut(name) {
                Some(status) => {
                    *status = next;
                    Ok(())
                },
                None => Err(anyhow!("no such name")),
            }
        }
    }

    #[async_trait]
    impl Players for Fake {
        async fn names(&self) -> Result<Vec<String>> {
            Ok(self.0.lock().players.keys().cloned().collect())
        }

        async fn status(&self, name: &str) -> Result<Playback> {
            let state = self.0.lock();
            if state.silent.contains(name) {
                return Err(anyhow!("no answer"));
            }
            state
                .players
                .get(name)
                .copied()
                .ok_or_else(|| anyhow!("no such name"))
        }

        async fn pause(&self, name: &str) -> Result<()> {
            self.command("pause", name, Playback::Paused)
        }

        async fn play(&self, name: &str) -> Result<()> {
            self.command("play", name, Playback::Playing)
        }
    }

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[tokio::test]
    async fn only_players_that_play_pause() {
        let fake = Fake::with(&[
            (SPOTIFY, Playback::Playing),
            (FIREFOX, Playback::Paused),
            (VLC, Playback::Stopped),
        ]);
        assert_eq!(pause_playing(&fake).await, vec![SPOTIFY.to_string()]);
        assert_eq!(fake.calls(), vec![format!("pause {}", SPOTIFY)]);
        assert_eq!(fake.status_of(SPOTIFY), Some(Playback::Paused));
        assert_eq!(fake.status_of(VLC), Some(Playback::Stopped));
    }

    #[tokio::test]
    async fn only_players_that_are_still_paused_resume() {
        let fake = Fake::with(&[(SPOTIFY, Playback::Playing), (VLC, Playback::Playing)]);
        let paused = pause_playing(&fake).await;
        assert_eq!(paused.len(), 2);
        fake.set(VLC, Playback::Stopped);

        resume_paused(&fake, paused).await;
        assert_eq!(fake.status_of(SPOTIFY), Some(Playback::Playing));
        assert_eq!(fake.status_of(VLC), Some(Playback::Stopped));
        assert!(!fake.calls().contains(&format!("play {}", VLC)));
    }

    #[tokio::test]
    async fn a_player_paused_before_the_take_stays_paused() {
        let fake = Fake::with(&[(SPOTIFY, Playback::Playing), (FIREFOX, Playback::Paused)]);
        let paused = pause_playing(&fake).await;
        resume_paused(&fake, paused).await;
        assert_eq!(fake.status_of(FIREFOX), Some(Playback::Paused));
        assert_eq!(
            fake.calls(),
            vec![format!("pause {}", SPOTIFY), format!("play {}", SPOTIFY)]
        );
    }

    #[tokio::test]
    async fn a_player_that_is_gone_is_skipped() {
        let fake = Fake::with(&[(FIREFOX, Playback::Playing), (VLC, Playback::Playing)]);
        let paused = pause_playing(&fake).await;
        fake.remove(FIREFOX);

        resume_paused(&fake, paused).await;
        assert!(!fake.calls().contains(&format!("play {}", FIREFOX)));
        assert_eq!(fake.status_of(VLC), Some(Playback::Playing));
    }

    #[tokio::test]
    async fn a_player_that_does_not_answer_does_not_stop_the_others() {
        let fake = Fake::with(&[(SPOTIFY, Playback::Playing), (VLC, Playback::Playing)]);
        fake.silence(SPOTIFY);
        let paused = pause_playing(&fake).await;
        assert_eq!(paused, vec![VLC.to_string()]);

        resume_paused(&fake, paused).await;
        assert_eq!(fake.status_of(VLC), Some(Playback::Playing));
    }

    #[test]
    fn a_tap_pauses_nothing() {
        let fake = Fake::with(&[(SPOTIFY, Playback::Playing)]);
        let base = Instant::now();
        let mut media = MediaPause::with_players(fake.clone()).unwrap();
        media.started(base, Some(TAP));
        media.live(at(base, 5));
        media.poll(at(base, 16));
        media.poll(at(base, 120));
        media.ended();
        drop(media);

        assert!(fake.calls().is_empty());
        assert_eq!(fake.status_of(SPOTIFY), Some(Playback::Playing));
    }

    #[test]
    fn a_hold_pauses_when_the_tap_window_closes_and_resumes_at_the_end() {
        let fake = Fake::with(&[(SPOTIFY, Playback::Playing)]);
        let base = Instant::now();
        let mut media = MediaPause::with_players(fake.clone()).unwrap();
        media.started(base, Some(TAP));
        media.live(at(base, 5));
        media.poll(at(base, 299));
        media.poll(at(base, 300));
        media.poll(at(base, 316));
        media.ended();
        drop(media);

        assert_eq!(
            fake.calls(),
            vec![format!("pause {}", SPOTIFY), format!("play {}", SPOTIFY)]
        );
        assert_eq!(fake.status_of(SPOTIFY), Some(Playback::Playing));
    }

    #[test]
    fn a_shutdown_during_a_take_resumes_the_players() {
        let fake = Fake::with(&[(SPOTIFY, Playback::Playing)]);
        let base = Instant::now();
        let mut media = MediaPause::with_players(fake.clone()).unwrap();
        media.started(base, None);
        media.live(at(base, 5));
        media.poll(at(base, 16));
        drop(media);

        assert_eq!(fake.status_of(SPOTIFY), Some(Playback::Playing));
        assert_eq!(fake.calls().len(), 2);
    }

    #[test]
    fn a_take_without_a_tap_window_pauses_at_its_first_frame() {
        let base = Instant::now();
        let mut timing = PauseTiming::default();
        timing.started(base, None);
        assert!(
            !timing.poll(at(base, 600)),
            "no pause before the first frame"
        );
        timing.live(at(base, 650));
        assert!(timing.poll(at(base, 650)));
        assert!(!timing.poll(at(base, 700)), "the pause goes out once");
        assert!(timing.ended());
        assert!(!timing.ended(), "the resume goes out once");
    }

    #[test]
    fn a_slow_first_frame_pauses_when_it_arrives() {
        let base = Instant::now();
        let mut timing = PauseTiming::default();
        timing.started(base, Some(TAP));
        timing.live(at(base, 900));
        assert!(timing.poll(at(base, 900)));
    }

    #[test]
    fn a_take_that_never_goes_live_resumes_nothing() {
        let base = Instant::now();
        let mut timing = PauseTiming::default();
        timing.started(base, None);
        assert!(!timing.poll(at(base, 5000)));
        assert!(!timing.ended());
    }

    #[test]
    fn only_mpris_players_count() {
        assert!(is_player(SPOTIFY));
        assert!(is_player(FIREFOX));
        assert!(!is_player("org.mpris.MediaPlayer2.playerctld"));
        assert!(!is_player("org.freedesktop.Notifications"));
        assert!(!is_player(":1.42"));
    }

    #[test]
    fn playback_status_parses() {
        assert_eq!(Playback::parse("Playing").unwrap(), Playback::Playing);
        assert_eq!(Playback::parse("Paused").unwrap(), Playback::Paused);
        assert_eq!(Playback::parse("Stopped").unwrap(), Playback::Stopped);
        assert!(Playback::parse("Buffering").is_err());
    }
}
