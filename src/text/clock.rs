//! Time for the insertion waits, behind a trait so tests run without
//! sleeping.

use std::time::{Duration, Instant};

pub trait Clock {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
pub mod fake {
    use super::Clock;
    use parking_lot::Mutex;
    use std::time::{Duration, Instant};

    /// Time moves only when something sleeps or calls [`FakeClock::advance`].
    pub struct FakeClock {
        start: Instant,
        now: Mutex<Instant>,
    }

    impl Default for FakeClock {
        fn default() -> Self {
            let start = Instant::now();
            Self {
                start,
                now: Mutex::new(start),
            }
        }
    }

    impl FakeClock {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn advance(&self, duration: Duration) {
            *self.now.lock() += duration;
        }

        pub fn elapsed(&self) -> Duration {
            *self.now.lock() - self.start
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.now.lock()
        }

        fn sleep(&self, duration: Duration) {
            self.advance(duration);
        }
    }
}
