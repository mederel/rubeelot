//! The security event log: sign-ups, sign-ins, sign-outs, expired sessions, refusals, exports and deletions, each
//! with the account when known, the time, the client's address and the outcome. Never a token or a credential.
//! Recording never blocks the action it records: a failed insert is logged and ignored.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use sqlx::MySqlPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::clock::Clock;
use crate::config::Limits;
use crate::db;
use crate::limits::FixedWindowLimiter;
use crate::limits::window::MAX_TRACKED;

const HOUR: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    SignUp,
    SignIn,
    SignOut,
    SessionExpired,
    RateLimited,
    Export,
    Deletion,
}

impl EventType {
    pub fn name(self) -> &'static str {
        match self {
            EventType::SignUp => "SIGN_UP",
            EventType::SignIn => "SIGN_IN",
            EventType::SignOut => "SIGN_OUT",
            EventType::SessionExpired => "SESSION_EXPIRED",
            EventType::RateLimited => "RATE_LIMITED",
            EventType::Export => "EXPORT",
            EventType::Deletion => "DELETION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
    Refused,
}

impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Success => "SUCCESS",
            Outcome::Failure => "FAILURE",
            Outcome::Refused => "REFUSED",
        }
    }
}

/// Records one event; never fails.
pub async fn record(
    pool: &MySqlPool,
    r#type: EventType,
    outcome: Outcome,
    account: Option<Uuid>,
    ip: Option<IpAddr>,
    now: OffsetDateTime,
) {
    if let Err(error) =
        db::events::insert(pool, r#type.name(), outcome.name(), account, ip, now).await
    {
        tracing::warn!(%error, event = r#type.name(), outcome = outcome.name(), "could not record a security event");
    }
}

/// The log, with a cap on events without an account: anyone can cause those with a failed sign-in or sign-up, so
/// beyond an hourly limit per address and overall they are dropped, and cannot fill the table. Deletions, anonymous
/// once the account is gone, are always kept.
pub struct SecurityEvents {
    pool: MySqlPool,
    clock: Arc<dyn Clock>,
    per_ip: FixedWindowLimiter,
    overall: FixedWindowLimiter,
}

impl SecurityEvents {
    pub fn new(pool: MySqlPool, limits: &Limits, clock: Arc<dyn Clock>) -> Self {
        Self {
            per_ip: FixedWindowLimiter::new(
                limits.anonymous_events_per_ip_per_hour,
                HOUR,
                MAX_TRACKED,
                clock.clone(),
            ),
            overall: FixedWindowLimiter::new(
                limits.anonymous_events_per_hour,
                HOUR,
                1,
                clock.clone(),
            ),
            pool,
            clock,
        }
    }

    /// Records one event now; never fails.
    pub async fn record(
        &self,
        r#type: EventType,
        outcome: Outcome,
        account: Option<Uuid>,
        ip: Option<IpAddr>,
    ) {
        if account.is_none() && r#type != EventType::Deletion {
            let key = ip.map_or_else(|| "unknown".to_owned(), |ip| ip.to_string());
            if !self.per_ip.try_acquire(&key) || !self.overall.try_acquire("all") {
                tracing::debug!(
                    event = r#type.name(),
                    "dropped an anonymous security event over the limit"
                );
                return;
            }
        }
        record(&self.pool, r#type, outcome, account, ip, self.clock.now()).await;
    }
}
