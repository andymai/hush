//! The local model in the daemon. [`IdleTimer`] says when the daemon has
//! been idle long enough to free it, and [`ModelSlot`] drops it then. The
//! next take loads it again in the background while the user speaks.

use crate::Result;
use futures_util::future::BoxFuture;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{info, warn};

/// Decides when an idle model is freed. The time counts from the last
/// moment the daemon was busy, or from the start.
#[derive(Debug, Clone)]
pub struct IdleTimer {
    after: Option<Duration>,
    last_busy: Instant,
}

impl IdleTimer {
    /// `minutes` of 0 never frees the model.
    pub fn new(minutes: u64, now: Instant) -> Self {
        Self {
            after: (minutes > 0).then(|| Duration::from_secs(minutes.saturating_mul(60))),
            last_busy: now,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.after.is_some()
    }

    /// `busy` is true during a take, a transcription, and while the result
    /// is processed, which includes a Command Mode request. The answer is
    /// true when the daemon was not busy for the full time.
    pub fn poll(&mut self, busy: bool, now: Instant) -> bool {
        if busy {
            self.last_busy = now;
            return false;
        }
        self.after
            .is_some_and(|after| now.saturating_duration_since(self.last_busy) >= after)
    }
}

/// Where the model is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Loaded,
    Loading,
    Unloaded,
    /// A load failed, and the take that started it has not seen the error
    Failed,
}

type Loader<T> = Box<dyn Fn() -> BoxFuture<'static, Result<T>>>;

enum Slot<T> {
    Loaded(T),
    Loading(oneshot::Receiver<Result<T>>),
    Unloaded,
    Failed(anyhow::Error),
}

/// A model that can be dropped and loaded again. A load runs as a Tokio
/// task, so call [`ModelSlot::begin_load`] and [`ModelSlot::ready`] on a
/// Tokio runtime.
pub struct ModelSlot<T> {
    slot: Slot<T>,
    loader: Loader<T>,
}

impl<T: Send + 'static> ModelSlot<T> {
    /// A slot that holds `model` now and calls `loader` for each reload.
    pub fn loaded(model: T, loader: impl Fn() -> BoxFuture<'static, Result<T>> + 'static) -> Self {
        Self {
            slot: Slot::Loaded(model),
            loader: Box::new(loader),
        }
    }

    pub fn state(&self) -> SlotState {
        match self.slot {
            Slot::Loaded(_) => SlotState::Loaded,
            Slot::Loading(_) => SlotState::Loading,
            Slot::Unloaded => SlotState::Unloaded,
            Slot::Failed(_) => SlotState::Failed,
        }
    }

    /// Drop the model, or the load in progress. True when something was
    /// dropped. A load in progress drops its model when it finishes.
    pub fn unload(&mut self) -> bool {
        matches!(
            std::mem::replace(&mut self.slot, Slot::Unloaded),
            Slot::Loaded(_) | Slot::Loading(_)
        )
    }

    /// Start a load in the background when no model is loaded or loading.
    /// True when a load started.
    pub fn begin_load(&mut self) -> bool {
        if matches!(self.slot, Slot::Loaded(_) | Slot::Loading(_)) {
            return false;
        }
        info!("Loading the model again");
        let (tx, rx) = oneshot::channel();
        let load = (self.loader)();
        tokio::spawn(async move {
            let started = Instant::now();
            let result = load.await;
            let load_ms = started.elapsed().as_millis() as u64;
            match &result {
                Ok(_) => info!(load_ms, "Loaded the model again in {} ms", load_ms),
                Err(e) => warn!(load_ms, "The model did not load again: {:#}", e),
            }
            let _ = tx.send(result);
        });
        self.slot = Slot::Loading(rx);
        true
    }

    /// Keep the result of a load that finished, without a wait. A failed
    /// load keeps its error for [`ModelSlot::ready`].
    pub fn settle(&mut self) {
        let Slot::Loading(rx) = &mut self.slot else {
            return;
        };
        self.slot = match rx.try_recv() {
            Ok(Ok(model)) => Slot::Loaded(model),
            Ok(Err(e)) => Slot::Failed(e),
            Err(oneshot::error::TryRecvError::Closed) => Slot::Failed(stopped()),
            Err(oneshot::error::TryRecvError::Empty) => return,
        };
    }

    /// The model, after a wait for the load in progress. An empty slot
    /// starts a load first. A failed load fails only this call: the slot
    /// is then empty, and the next take loads again.
    pub async fn ready(&mut self) -> Result<&T> {
        if matches!(self.slot, Slot::Unloaded) {
            self.begin_load();
        }
        if let Slot::Loading(rx) = &mut self.slot {
            self.slot = match rx.await {
                Ok(Ok(model)) => Slot::Loaded(model),
                Ok(Err(e)) => Slot::Failed(e),
                Err(_) => Slot::Failed(stopped()),
            };
        }
        if matches!(self.slot, Slot::Failed(_)) {
            if let Slot::Failed(e) = std::mem::replace(&mut self.slot, Slot::Unloaded) {
                return Err(e);
            }
        }
        match &self.slot {
            Slot::Loaded(model) => Ok(model),
            _ => Err(anyhow::anyhow!("The model is not loaded")),
        }
    }
}

fn stopped() -> anyhow::Error {
    anyhow::anyhow!("The model load stopped before it finished")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Weak};

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn the_model_is_freed_after_the_idle_minutes() {
        let start = Instant::now();
        let mut timer = IdleTimer::new(15, start);
        assert!(timer.is_enabled());
        assert!(!timer.poll(false, start + 15 * MINUTE - Duration::from_secs(1)));
        assert!(timer.poll(false, start + 15 * MINUTE));
        assert!(timer.poll(false, start + 60 * MINUTE));
    }

    #[test]
    fn zero_minutes_never_frees_the_model() {
        let start = Instant::now();
        let mut timer = IdleTimer::new(0, start);
        assert!(!timer.is_enabled());
        assert!(!timer.poll(false, start + 10_000 * MINUTE));
    }

    #[test]
    fn a_busy_daemon_never_frees_the_model() {
        let start = Instant::now();
        let mut timer = IdleTimer::new(1, start);
        assert!(!timer.poll(true, start + 2 * MINUTE));
        assert!(!timer.poll(true, start + 30 * MINUTE));
    }

    #[test]
    fn the_time_counts_again_after_each_transcription() {
        let start = Instant::now();
        let mut timer = IdleTimer::new(10, start);
        assert!(!timer.poll(false, start + 4 * MINUTE));
        assert!(!timer.poll(true, start + 5 * MINUTE), "a take records");
        assert!(!timer.poll(true, start + 6 * MINUTE), "it transcribes");
        assert!(!timer.poll(false, start + 12 * MINUTE));
        assert!(!timer.poll(false, start + 16 * MINUTE - Duration::from_secs(1)));
        assert!(timer.poll(false, start + 16 * MINUTE));
    }

    #[test]
    fn a_huge_minute_count_does_not_overflow() {
        let start = Instant::now();
        let mut timer = IdleTimer::new(u64::MAX, start);
        assert!(!timer.poll(false, start + 10_000 * MINUTE));
    }

    struct Fake {
        loads: Arc<AtomicUsize>,
        fail_first: usize,
        current: Arc<std::sync::Mutex<Weak<()>>>,
    }

    impl Fake {
        fn new(fail_first: usize) -> Self {
            Self {
                loads: Arc::new(AtomicUsize::new(0)),
                fail_first,
                current: Arc::new(std::sync::Mutex::new(Weak::new())),
            }
        }

        fn slot(&self) -> (ModelSlot<Arc<()>>, Weak<()>) {
            let model = Arc::new(());
            let first = Arc::downgrade(&model);
            let loads = Arc::clone(&self.loads);
            let current = Arc::clone(&self.current);
            let fail_first = self.fail_first;
            let slot = ModelSlot::loaded(model, move || {
                let loads = Arc::clone(&loads);
                let current = Arc::clone(&current);
                Box::pin(async move {
                    let n = loads.fetch_add(1, Ordering::SeqCst);
                    if n < fail_first {
                        return Err(anyhow::anyhow!("no model file"));
                    }
                    let model = Arc::new(());
                    *current.lock().unwrap() = Arc::downgrade(&model);
                    Ok(model)
                })
            });
            (slot, first)
        }

        fn loads(&self) -> usize {
            self.loads.load(Ordering::SeqCst)
        }

        fn last_loaded(&self) -> Weak<()> {
            self.current.lock().unwrap().clone()
        }
    }

    async fn settled(slot: &mut ModelSlot<Arc<()>>) -> SlotState {
        for _ in 0..100 {
            slot.settle();
            if slot.state() != SlotState::Loading {
                break;
            }
            tokio::task::yield_now().await;
        }
        slot.state()
    }

    #[tokio::test]
    async fn a_loaded_model_is_ready_without_a_load() {
        let fake = Fake::new(0);
        let (mut slot, _) = fake.slot();
        assert_eq!(slot.state(), SlotState::Loaded);
        assert!(!slot.begin_load(), "a loaded slot loads nothing");
        assert!(slot.ready().await.is_ok());
        assert_eq!(fake.loads(), 0);
    }

    #[tokio::test]
    async fn unloading_drops_the_model() {
        let fake = Fake::new(0);
        let (mut slot, first) = fake.slot();
        assert!(slot.unload());
        assert_eq!(slot.state(), SlotState::Unloaded);
        assert!(first.upgrade().is_none(), "the model is freed");
        assert!(!slot.unload(), "an empty slot has nothing to drop");
    }

    #[tokio::test]
    async fn a_take_loads_the_model_in_the_background() {
        let fake = Fake::new(0);
        let (mut slot, _) = fake.slot();
        slot.unload();
        assert!(slot.begin_load());
        assert_eq!(slot.state(), SlotState::Loading);
        assert!(!slot.begin_load(), "one load at a time");
        assert_eq!(settled(&mut slot).await, SlotState::Loaded);
        assert_eq!(fake.loads(), 1);
        assert!(slot.ready().await.is_ok());
        assert_eq!(fake.loads(), 1);
    }

    #[tokio::test]
    async fn a_stop_waits_for_the_load_in_progress() {
        let fake = Fake::new(0);
        let (mut slot, _) = fake.slot();
        slot.unload();
        slot.begin_load();
        assert!(slot.ready().await.is_ok());
        assert_eq!(slot.state(), SlotState::Loaded);
        assert_eq!(fake.loads(), 1);
    }

    #[tokio::test]
    async fn an_empty_slot_loads_when_the_model_is_needed() {
        let fake = Fake::new(0);
        let (mut slot, _) = fake.slot();
        slot.unload();
        assert!(slot.ready().await.is_ok());
        assert_eq!(slot.state(), SlotState::Loaded);
        assert_eq!(fake.loads(), 1);
    }

    #[tokio::test]
    async fn a_failed_load_fails_one_take_and_the_next_tries_again() {
        let fake = Fake::new(1);
        let (mut slot, _) = fake.slot();
        slot.unload();
        slot.begin_load();
        let error = slot.ready().await.err().map(|e| e.to_string());
        assert_eq!(error.as_deref(), Some("no model file"));
        assert_eq!(slot.state(), SlotState::Unloaded);

        assert!(slot.begin_load());
        assert!(slot.ready().await.is_ok());
        assert_eq!(slot.state(), SlotState::Loaded);
        assert_eq!(fake.loads(), 2);
    }

    #[tokio::test]
    async fn a_failed_background_load_fails_its_take_without_a_second_load() {
        let fake = Fake::new(1);
        let (mut slot, _) = fake.slot();
        slot.unload();
        slot.begin_load();
        assert_eq!(settled(&mut slot).await, SlotState::Failed);
        assert!(slot.ready().await.is_err());
        assert_eq!(fake.loads(), 1);
        assert_eq!(slot.state(), SlotState::Unloaded);
        assert!(slot.begin_load(), "the next take tries again");
    }

    #[tokio::test]
    async fn a_new_take_after_an_unseen_failure_loads_again() {
        let fake = Fake::new(1);
        let (mut slot, _) = fake.slot();
        slot.unload();
        slot.begin_load();
        assert_eq!(settled(&mut slot).await, SlotState::Failed);
        assert!(slot.begin_load(), "a take that starts now loads again");
        assert!(slot.ready().await.is_ok());
        assert_eq!(fake.loads(), 2);
    }

    #[tokio::test]
    async fn unloading_clears_a_failure_that_holds_no_model() {
        let fake = Fake::new(1);
        let (mut slot, _) = fake.slot();
        slot.unload();
        slot.begin_load();
        assert_eq!(settled(&mut slot).await, SlotState::Failed);
        assert!(!slot.unload());
        assert_eq!(slot.state(), SlotState::Unloaded);
    }

    #[tokio::test]
    async fn unloading_during_a_load_drops_the_new_model() {
        let fake = Fake::new(0);
        let (mut slot, _) = fake.slot();
        slot.unload();
        slot.begin_load();
        assert!(slot.unload());
        assert_eq!(slot.state(), SlotState::Unloaded);
        for _ in 0..100 {
            if fake.loads() == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        tokio::task::yield_now().await;
        assert_eq!(fake.loads(), 1);
        assert!(fake.last_loaded().upgrade().is_none());
    }
}
