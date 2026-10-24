//! Sessions of signed-in players. The browser holds a random 256-bit token in the `BEELOT_SESSION` cookie; the
//! database keeps only its SHA-256. A session ends 7 days after its last use or 30 days after its sign-in, whichever
//! comes first; each end the server notices is recorded once as `SESSION_EXPIRED`.

use std::time::Duration;

use axum::http::HeaderMap;
use axum::http::header;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::sessions::{self as store, StoredSession, TokenHash};
use crate::privacy::events::{self, EventType, Outcome};

pub const COOKIE: &str = "BEELOT_SESSION";
pub const IDLE_LIMIT: Duration = Duration::from_secs(7 * 86_400);
pub const ABSOLUTE_LIMIT: Duration = Duration::from_secs(30 * 86_400);
/// `last_seen_at` is written at most this often.
const SEEN_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSession {
    pub account_id: Uuid,
    pub signed_in_at: OffsetDateTime,
    pub token_hash: TokenHash,
}

impl CurrentSession {
    /// Whether the session signed in less than `limit` ago.
    pub fn signed_in_recently(&self, now: OffsetDateTime, limit: Duration) -> bool {
        self.signed_in_at > now - limit
    }
}

pub fn hash(token: &str) -> TokenHash {
    Sha256::digest(token.as_bytes()).into()
}

/// The `Set-Cookie` value that gives the browser its session.
pub fn cookie(token: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/{secure}")
}

/// The `Set-Cookie` value that takes the session cookie away.
pub fn expired_cookie(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0{secure}")
}

/// The value of a cookie of the request.
pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

/// The session token the request carries.
pub fn token(headers: &HeaderMap) -> Option<&str> {
    cookie_value(headers, COOKIE).filter(|token| !token.is_empty())
}

/// Starts a session for the account and returns its token.
pub async fn start(pool: &MySqlPool, account: Uuid, now: OffsetDateTime) -> sqlx::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system's random generator");
    let token = URL_SAFE_NO_PAD.encode(bytes);
    store::insert(
        pool,
        &StoredSession {
            token_hash: hash(&token),
            account_id: account,
            signed_in_at: now,
            last_seen_at: now,
        },
    )
    .await?;
    Ok(token)
}

/// Ends the session on purpose: it isn't an expiry.
pub async fn end(pool: &MySqlPool, token_hash: &TokenHash) -> sqlx::Result<bool> {
    store::delete(pool, token_hash).await
}

/// Ends every session of the account.
pub async fn end_all(pool: &MySqlPool, account: Uuid) -> sqlx::Result<u64> {
    store::delete_for_account(pool, account).await
}

fn expired(session: &StoredSession, now: OffsetDateTime) -> bool {
    session.last_seen_at + IDLE_LIMIT <= now || session.signed_in_at + ABSOLUTE_LIMIT <= now
}

async fn expire(
    pool: &MySqlPool,
    session: &StoredSession,
    now: OffsetDateTime,
) -> sqlx::Result<bool> {
    let ended = store::delete(pool, &session.token_hash).await?;
    if ended {
        events::record(
            pool,
            EventType::SessionExpired,
            Outcome::Success,
            Some(session.account_id),
            None,
            now,
        )
        .await;
    }
    Ok(ended)
}

/// The live session of that token. One past its limits is ended and recorded as an expiry; one in use has its
/// last use bumped, at most once a minute.
pub async fn lookup(
    pool: &MySqlPool,
    token: &str,
    now: OffsetDateTime,
) -> sqlx::Result<Option<CurrentSession>> {
    let Some(session) = store::find(pool, &hash(token)).await? else {
        return Ok(None);
    };
    if expired(&session, now) {
        expire(pool, &session, now).await?;
        return Ok(None);
    }
    if session.last_seen_at + SEEN_EVERY <= now {
        store::touch(pool, &session.token_hash, now).await?;
    }
    Ok(Some(CurrentSession {
        account_id: session.account_id,
        signed_in_at: session.signed_in_at,
        token_hash: session.token_hash,
    }))
}

/// Ends every session past its limits, recording each expiry once; returns how many ended.
pub async fn sweep_expired(pool: &MySqlPool, now: OffsetDateTime) -> sqlx::Result<u64> {
    let mut ended = 0;
    for session in store::expired(pool, now - IDLE_LIMIT, now - ABSOLUTE_LIMIT).await? {
        if expire(pool, &session, now).await? {
            ended += 1;
        }
    }
    Ok(ended)
}
