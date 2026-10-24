//! The `session` table: signed-in browsers, keyed by the SHA-256 of their cookie's token.

use sqlx::{Executor, FromRow, MySql};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use super::{DbUuid, from_utc, id, utc};

pub type TokenHash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSession {
    pub token_hash: TokenHash,
    pub account_id: Uuid,
    pub signed_in_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
}

#[derive(FromRow)]
struct Row {
    token_hash: Vec<u8>,
    account_id: DbUuid,
    signed_in_at: PrimitiveDateTime,
    last_seen_at: PrimitiveDateTime,
}

impl From<Row> for StoredSession {
    fn from(row: Row) -> Self {
        Self {
            token_hash: row.token_hash.try_into().unwrap_or([0; 32]),
            account_id: row.account_id.0,
            signed_in_at: from_utc(row.signed_in_at),
            last_seen_at: from_utc(row.last_seen_at),
        }
    }
}

pub async fn insert<'c>(
    db: impl Executor<'c, Database = MySql>,
    session: &StoredSession,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO session (token_hash, account_id, signed_in_at, last_seen_at) VALUES (?, ?, ?, ?)")
        .bind(&session.token_hash[..])
        .bind(id(session.account_id))
        .bind(utc(session.signed_in_at))
        .bind(utc(session.last_seen_at))
        .execute(db)
        .await?;
    Ok(())
}

pub async fn find<'c>(
    db: impl Executor<'c, Database = MySql>,
    token_hash: &TokenHash,
) -> sqlx::Result<Option<StoredSession>> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT token_hash, account_id, signed_in_at, last_seen_at FROM session WHERE token_hash = ?",
    )
    .bind(&token_hash[..])
    .fetch_optional(db)
    .await?;
    Ok(row.map(StoredSession::from))
}

/// Deletes the session; false when it was already gone.
pub async fn delete<'c>(
    db: impl Executor<'c, Database = MySql>,
    token_hash: &TokenHash,
) -> sqlx::Result<bool> {
    let deleted = sqlx::query("DELETE FROM session WHERE token_hash = ?")
        .bind(&token_hash[..])
        .execute(db)
        .await?;
    Ok(deleted.rows_affected() > 0)
}

pub async fn delete_for_account<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
) -> sqlx::Result<u64> {
    let deleted = sqlx::query("DELETE FROM session WHERE account_id = ?")
        .bind(id(account))
        .execute(db)
        .await?;
    Ok(deleted.rows_affected())
}

pub async fn touch<'c>(
    db: impl Executor<'c, Database = MySql>,
    token_hash: &TokenHash,
    now: OffsetDateTime,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE session SET last_seen_at = ? WHERE token_hash = ?")
        .bind(utc(now))
        .bind(&token_hash[..])
        .execute(db)
        .await?;
    Ok(())
}

/// The sessions last seen at or before `idle_before`, or signed in at or before `signed_in_before`.
pub async fn expired<'c>(
    db: impl Executor<'c, Database = MySql>,
    idle_before: OffsetDateTime,
    signed_in_before: OffsetDateTime,
) -> sqlx::Result<Vec<StoredSession>> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT token_hash, account_id, signed_in_at, last_seen_at FROM session \
         WHERE last_seen_at <= ? OR signed_in_at <= ?",
    )
    .bind(utc(idle_before))
    .bind(utc(signed_in_before))
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(StoredSession::from).collect())
}
