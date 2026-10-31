//! Deleting an account and everything Beelot keeps about it: its security events and, by cascade, its passkey and
//! sessions. Finished matches stay in the other players' history, with the account's seats renamed "Deleted player"
//! and linked to no account. Once that is committed, the account's live table seats go to bots and an anonymous
//! deletion event is recorded.

use std::net::IpAddr;

use uuid::Uuid;

use crate::app::AppState;
use crate::auth::sessions;
use crate::db::id;
use crate::privacy::events::{EventType, Outcome};
use crate::tables::finished::DELETED_PLAYER;

/// Deletes the account; `ip` is the client's when a player asks, none for the retention job.
pub async fn delete(state: &AppState, account: Uuid, ip: Option<IpAddr>) -> sqlx::Result<()> {
    let mut transaction = state.pool.begin().await?;
    sqlx::query("UPDATE match_seat SET account_id = NULL, name = ? WHERE account_id = ?")
        .bind(DELETED_PLAYER)
        .bind(id(account))
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM security_event WHERE account_id = ?")
        .bind(id(account))
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM account WHERE id = ?")
        .bind(id(account))
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    let registry = state.registry.clone();
    match tokio::task::spawn_blocking(move || registry.release_account(account)).await {
        Ok(outcomes) => state.hand_over(outcomes),
        Err(error) => {
            tracing::warn!(%error, "an account was deleted, but its table seats could not go to bots")
        }
    }
    if let Err(error) = sessions::end_all(&state.pool, account).await {
        tracing::warn!(%error, "an account was deleted, but its sessions could not be ended");
    }
    state
        .events
        .record(EventType::Deletion, Outcome::Success, None, ip)
        .await;
    Ok(())
}
