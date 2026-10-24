//! The time every timed rule reads, so tests can move it forward without sleeping.

use std::sync::Mutex;

use time::{Duration, OffsetDateTime};

pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

/// The system's UTC time.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// A clock that stays still until a test advances it.
#[derive(Debug)]
pub struct FakeClock(Mutex<OffsetDateTime>);

impl FakeClock {
    pub fn new(start: OffsetDateTime) -> Self {
        Self(Mutex::new(start))
    }

    pub fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        *self.0.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn fake_clock_moves_only_when_advanced() {
        let clock = FakeClock::new(datetime!(2026-01-01 0:00 UTC));
        assert_eq!(clock.now(), datetime!(2026-01-01 0:00 UTC));
        clock.advance(Duration::minutes(90));
        assert_eq!(clock.now(), datetime!(2026-01-01 1:30 UTC));
    }
}
