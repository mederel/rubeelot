//! The `account` table.

use sqlx::{Executor, FromRow, MySql};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

use super::{DbUuid, from_utc, id, utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    pub display_name: String,
    pub created_at: OffsetDateTime,
    pub last_sign_in_at: OffsetDateTime,
    pub previous_sign_in_at: Option<OffsetDateTime>,
}

impl Account {
    /// A new account, signed in now.
    pub fn new(id: Uuid, display_name: String, now: OffsetDateTime) -> Self {
        Self {
            id,
            display_name,
            created_at: now,
            last_sign_in_at: now,
            previous_sign_in_at: None,
        }
    }
}

#[derive(FromRow)]
struct Row {
    id: DbUuid,
    display_name: String,
    created_at: PrimitiveDateTime,
    last_sign_in_at: PrimitiveDateTime,
    previous_sign_in_at: Option<PrimitiveDateTime>,
}

impl From<Row> for Account {
    fn from(row: Row) -> Self {
        Self {
            id: row.id.0,
            display_name: row.display_name,
            created_at: from_utc(row.created_at),
            last_sign_in_at: from_utc(row.last_sign_in_at),
            previous_sign_in_at: row.previous_sign_in_at.map(from_utc),
        }
    }
}

pub async fn insert<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: &Account,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO account (id, display_name, created_at, last_sign_in_at, previous_sign_in_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id(account.id))
    .bind(&account.display_name)
    .bind(utc(account.created_at))
    .bind(utc(account.last_sign_in_at))
    .bind(account.previous_sign_in_at.map(utc))
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
) -> sqlx::Result<Option<Account>> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, display_name, created_at, last_sign_in_at, previous_sign_in_at FROM account WHERE id = ?",
    )
    .bind(id(account))
    .fetch_optional(db)
    .await?;
    Ok(row.map(Account::from))
}

/// A sign-in now: the latest sign-in becomes the previous one.
pub async fn record_sign_in<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
    now: OffsetDateTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE account SET previous_sign_in_at = last_sign_in_at, last_sign_in_at = ? WHERE id = ?",
    )
    .bind(utc(now))
    .bind(id(account))
    .execute(db)
    .await?;
    Ok(())
}

/// Deletes the account; its passkey and sessions go with it, and its match seats lose it.
pub async fn delete<'c>(
    db: impl Executor<'c, Database = MySql>,
    account: Uuid,
) -> sqlx::Result<bool> {
    let deleted = sqlx::query("DELETE FROM account WHERE id = ?")
        .bind(id(account))
        .execute(db)
        .await?;
    Ok(deleted.rows_affected() > 0)
}
