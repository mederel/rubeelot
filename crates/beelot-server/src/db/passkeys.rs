//! The `passkey` table: each account's one passkey, as webauthn-rs serializes it.

use sqlx::{Executor, MySql};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{id, utc};

pub async fn insert<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
    credential_id: &[u8],
    passkey: &str,
    now: OffsetDateTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO passkey (account_id, credential_id, passkey, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(id(account))
    .bind(credential_id)
    .bind(passkey)
    .bind(utc(now))
    .execute(db)
    .await?;
    Ok(())
}

/// The serialized passkey of the account with that credential id.
pub async fn find<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
    credential_id: &[u8],
) -> sqlx::Result<Option<String>> {
    // MariaDB's JSON is a binary-collated LONGTEXT.
    let stored: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT passkey FROM passkey WHERE account_id = ? AND credential_id = ?",
    )
    .bind(id(account))
    .bind(credential_id)
    .fetch_optional(db)
    .await?;
    Ok(stored.and_then(|bytes| String::from_utf8(bytes).ok()))
}

/// Stores the passkey after a sign-in, with its new counter, and when it was used.
pub async fn update<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
    passkey: &str,
    now: OffsetDateTime,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE passkey SET passkey = ?, last_used_at = ? WHERE account_id = ?")
        .bind(passkey)
        .bind(utc(now))
        .bind(id(account))
        .execute(db)
        .await?;
    Ok(())
}
