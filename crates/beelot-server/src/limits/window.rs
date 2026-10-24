//! A fixed-window counter per key, ported from Java's `RequestRateLimitFilter` and `HourlyIpLimiter`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use time::OffsetDateTime;

use crate::clock::Clock;

/// The keys tracked at most, so the limiter cannot itself be used to exhaust memory.
pub const MAX_TRACKED: usize = 50_000;

#[derive(Debug, Clone, Copy)]
struct Window {
    start: OffsetDateTime,
    count: u32,
}

pub struct FixedWindowLimiter {
    limit: u32,
    window: Duration,
    max_tracked: usize,
    clock: Arc<dyn Clock>,
    windows: Mutex<HashMap<String, Window>>,
}

impl std::fmt::Debug for FixedWindowLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixedWindowLimiter")
            .field("limit", &self.limit)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl FixedWindowLimiter {
    pub fn new(limit: u32, window: Duration, max_tracked: usize, clock: Arc<dyn Clock>) -> Self {
        Self {
            limit,
            window,
            max_tracked,
            clock,
            windows: Mutex::new(HashMap::new()),
        }
    }

    fn expired(&self, window: &Window, now: OffsetDateTime) -> bool {
        now - window.start >= self.window
    }

    /// Counts this attempt, refused or not. `Err` holds the seconds to wait before the window ends.
    pub fn consume(&self, key: &str) -> Result<(), u64> {
        let now = self.clock.now();
        let mut windows = self.windows.lock().unwrap();
        if windows.len() >= self.max_tracked && !windows.contains_key(key) {
            windows.retain(|_, window| !self.expired(window, now));
            if windows.len() >= self.max_tracked {
                return Err(self.window.as_secs() + 1);
            }
        }
        let window = windows
            .entry(key.to_owned())
            .and_modify(|current| {
                if self.expired(current, now) {
                    *current = Window {
                        start: now,
                        count: 1,
                    };
                } else {
                    current.count += 1;
                }
            })
            .or_insert(Window {
                start: now,
                count: 1,
            });
        if window.count <= self.limit {
            return Ok(());
        }
        let left_ms = (window.start + self.window - now).whole_milliseconds();
        Err(((left_ms + 999) / 1000).max(1) as u64)
    }

    /// Whether the key is within its limit now; when it is, this attempt is counted. A refusal isn't counted.
    pub fn try_acquire(&self, key: &str) -> bool {
        let now = self.clock.now();
        let mut windows = self.windows.lock().unwrap();
        match windows.get_mut(key) {
            Some(window) if !self.expired(window, now) => {
                if window.count >= self.limit {
                    return false;
                }
                window.count += 1;
                true
            }
            current => {
                if self.limit == 0 {
                    return false;
                }
                if current.is_none() && windows.len() >= self.max_tracked {
                    windows.retain(|_, window| !self.expired(window, now));
                    if windows.len() >= self.max_tracked {
                        return false;
                    }
                }
                windows.insert(
                    key.to_owned(),
                    Window {
                        start: now,
                        count: 1,
                    },
                );
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! The limiter cases of Java's `PasskeyRateLimiterTest`.

    use time::macros::datetime;

    use super::*;
    use crate::clock::FakeClock;

    const HOUR: Duration = Duration::from_secs(3600);

    fn limiter(limit: u32) -> (FixedWindowLimiter, Arc<FakeClock>) {
        let clock = Arc::new(FakeClock::new(datetime!(2026-01-01 0:00 UTC)));
        (
            FixedWindowLimiter::new(limit, HOUR, MAX_TRACKED, clock.clone()),
            clock,
        )
    }

    #[test]
    fn allows_up_to_the_limit_per_ip() {
        let (limiter, _) = limiter(30);
        for _ in 0..30 {
            assert!(limiter.try_acquire("10.0.0.1"));
        }
        assert!(!limiter.try_acquire("10.0.0.1"));
    }

    #[test]
    fn another_ip_is_not_affected() {
        let (limiter, _) = limiter(30);
        for _ in 0..31 {
            limiter.try_acquire("10.0.0.1");
        }
        assert!(limiter.try_acquire("10.0.0.2"));
    }

    #[test]
    fn window_resets_after_an_hour() {
        let (limiter, clock) = limiter(30);
        for _ in 0..30 {
            limiter.try_acquire("10.0.0.1");
        }
        assert!(!limiter.try_acquire("10.0.0.1"));
        clock.advance(time::Duration::hours(1));
        assert!(limiter.try_acquire("10.0.0.1"));
    }

    #[test]
    fn refused_requests_do_not_count() {
        let (limiter, clock) = limiter(30);
        for _ in 0..30 {
            limiter.try_acquire("10.0.0.1");
        }
        for _ in 0..20 {
            assert!(!limiter.try_acquire("10.0.0.1"));
        }
        clock.advance(time::Duration::hours(1));
        for _ in 0..30 {
            assert!(limiter.try_acquire("10.0.0.1"));
        }
        assert!(!limiter.try_acquire("10.0.0.1"));
    }

    #[test]
    fn bounds_tracked_ips() {
        let (generous, clock) = limiter(3);
        for i in 0..50_000 {
            assert!(generous.try_acquire(&format!("ip{i}")));
        }
        assert!(!generous.try_acquire("newcomer"));
        assert!(generous.try_acquire("ip0"));
        clock.advance(time::Duration::hours(1));
        assert!(generous.try_acquire("newcomer"));
    }

    #[test]
    fn consume_counts_refusals_and_says_how_long_to_wait() {
        let clock = Arc::new(FakeClock::new(datetime!(2026-01-01 0:00 UTC)));
        let limiter =
            FixedWindowLimiter::new(1, Duration::from_secs(60), MAX_TRACKED, clock.clone());
        assert_eq!(limiter.consume("a"), Ok(()));
        clock.advance(time::Duration::milliseconds(10_500));
        assert_eq!(limiter.consume("a"), Err(50));
        clock.advance(time::Duration::seconds(49));
        assert_eq!(limiter.consume("a"), Err(1));
        clock.advance(time::Duration::seconds(1));
        assert_eq!(limiter.consume("a"), Ok(()));
    }
}
