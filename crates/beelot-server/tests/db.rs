//! The schema and the account repository, on MariaDB.

#[path = "common/db.rs"]
mod db;

use beelot_server::db::accounts::{self, Account};
use beelot_server::db::events;
use time::Duration;
use time::macros::datetime;
use uuid::Uuid;

#[tokio::test]
async fn migrations_apply_and_account_round_trips() {
    let pool = db::test_pool().await;
    let created = datetime!(2026-10-24 12:00:00.123456 UTC);
    let account = Account::new(Uuid::new_v4(), "Lucky Otter".into(), created);
    accounts::insert(&pool, &account).await.unwrap();
    assert_eq!(
        accounts::find(&pool, account.id).await.unwrap(),
        Some(account.clone())
    );

    let later = created + Duration::days(2);
    accounts::record_sign_in(&pool, account.id, later)
        .await
        .unwrap();
    let found = accounts::find(&pool, account.id).await.unwrap().unwrap();
    assert_eq!(found.last_sign_in_at, later);
    assert_eq!(
        found.previous_sign_in_at,
        Some(created),
        "the previous sign-in shifts, as Account.signedIn does"
    );

    let latest = later + Duration::hours(1);
    accounts::record_sign_in(&pool, account.id, latest)
        .await
        .unwrap();
    let found = accounts::find(&pool, account.id).await.unwrap().unwrap();
    assert_eq!(
        (found.previous_sign_in_at, found.last_sign_in_at),
        (Some(later), latest)
    );

    events::insert(
        &pool,
        "SIGN_IN",
        "SUCCESS",
        Some(account.id),
        "10.0.0.1".parse().ok(),
        latest,
    )
    .await
    .unwrap();
    assert!(accounts::delete(&pool, account.id).await.unwrap());
    assert_eq!(accounts::find(&pool, account.id).await.unwrap(), None);
    assert!(!accounts::delete(&pool, account.id).await.unwrap());
}

#[tokio::test]
async fn names_keep_their_accents() {
    let pool = db::test_pool().await;
    let account = Account::new(
        Uuid::new_v4(),
        "Chloé Brûlée".into(),
        datetime!(2026-10-24 12:00 UTC),
    );
    accounts::insert(&pool, &account).await.unwrap();
    assert_eq!(
        accounts::find(&pool, account.id)
            .await
            .unwrap()
            .unwrap()
            .display_name,
        "Chloé Brûlée"
    );
}
