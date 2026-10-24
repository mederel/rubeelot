//! The `security_event` table.

use std::net::IpAddr;

use sqlx::{Executor, MySql};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{id, utc};

pub async fn insert<'c>(
    db: impl Executor<'c, Database = MySql>,
    r#type: &str,
    outcome: &str,
    account: Option<Uuid>,
    ip: Option<IpAddr>,
    at: OffsetDateTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO security_event (account_id, type, outcome, ip_address, occurred_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(account.map(id))
    .bind(r#type)
    .bind(outcome)
    .bind(ip.map(|ip| ip.to_string()))
    .bind(utc(at))
    .execute(db)
    .await?;
    Ok(())
}
