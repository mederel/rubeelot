//! The security event log: sign-ups, sign-ins, sign-outs, expired sessions, refusals, exports and deletions, each
//! with the account when known, the time, the client's address and the outcome. Never a token or a credential.
//! Recording never blocks the action it records: a failed insert is logged and ignored.

use std::net::IpAddr;

use sqlx::MySqlPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db;

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
