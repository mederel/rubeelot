//! Sessions, CSRF and `/api/account`, on MariaDB. Ports `SessionLifetimeFilterTest`, `AccountSessionsTest`, the CSRF
//! cases of `AccountIntegrationTest` and its `/api/account` cases.

mod common;
#[path = "common/db.rs"]
mod db;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use beelot_server::app::{AppState, router};
use beelot_server::auth::sessions;
use beelot_server::clock::{Clock, FakeClock};
use beelot_server::db::accounts::{self, Account};
use common::{body_json, test_state_with_clock};
use serde_json::json;
use sqlx::MySqlPool;
use time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

const CSRF: &str = "5f0c9a2e-test-csrf-token";

struct World {
    app: Router,
    clock: Arc<FakeClock>,
    pool: MySqlPool,
}

async fn world() -> World {
    let pool = db::test_pool().await;
    let (state, clock) = test_state_with_clock();
    let state: AppState = state.with_pool(pool.clone());
    World {
        app: router(state),
        clock,
        pool,
    }
}

impl World {
    /// A new account with a session signed in now; returns its id and the session cookie's token.
    async fn signed_in(&self, name: &str) -> (Uuid, String) {
        let account = Account::new(Uuid::new_v4(), name.into(), self.clock.now());
        accounts::insert(&self.pool, &account).await.unwrap();
        let token = sessions::start(&self.pool, account.id, self.clock.now())
            .await
            .unwrap();
        (account.id, token)
    }

    async fn send(&self, request: Request<Body>) -> axum::http::Response<Body> {
        self.app.clone().oneshot(request).await.unwrap()
    }

    async fn account(&self, session: Option<&str>) -> serde_json::Value {
        let mut request = Request::get("/api/account");
        if let Some(token) = session {
            request = request.header(header::COOKIE, format!("BEELOT_SESSION={token}"));
        }
        let response = self.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(response.status(), StatusCode::OK);
        body_json(response).await
    }

    async fn events(&self, r#type: &str) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM security_event WHERE type = ?")
            .bind(r#type)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn session_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM session")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }
}

fn cookie_named<'a>(response: &'a axum::http::Response<Body>, name: &str) -> Option<&'a str> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|cookie| cookie.starts_with(&format!("{name}=")))
}

fn post(uri: &str, session: Option<&str>, csrf: Option<&str>) -> Request<Body> {
    let mut cookies = Vec::new();
    if let Some(token) = session {
        cookies.push(format!("BEELOT_SESSION={token}"));
    }
    let mut request = Request::post(uri).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = csrf {
        cookies.push(format!("XSRF-TOKEN={token}"));
        request = request.header("X-XSRF-TOKEN", token);
    }
    if !cookies.is_empty() {
        request = request.header(header::COOKIE, cookies.join("; "));
    }
    request
        .body(Body::from(
            json!({"kind": "LOCAL", "difficulty": "RELAXED"}).to_string(),
        ))
        .unwrap()
}

// AccountIntegrationTest

#[tokio::test]
async fn a_guest_is_not_signed_in() {
    let world = world().await;
    assert_eq!(world.account(None).await, json!({"signedIn": false}));
}

#[tokio::test]
async fn a_passkey_player_is_signed_in_under_their_pseudonym() {
    let world = world().await;
    let (_, token) = world.signed_in("Lucky Otter").await;
    let account = world.account(Some(&token)).await;
    assert_eq!(account["signedIn"], true);
    assert_eq!(account["name"], "Lucky Otter");
    assert!(
        account.get("lastSignInAt").is_none(),
        "no sign-in before this one"
    );
}

#[tokio::test]
async fn a_player_whose_account_is_gone_is_signed_out() {
    let world = world().await;
    let (id, token) = world.signed_in("Lucky Otter").await;
    sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
        .execute(&world.pool)
        .await
        .ok();
    let mut connection = world.pool.acquire().await.unwrap();
    sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("DELETE FROM account WHERE id = ?")
        .bind(id.hyphenated())
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    assert_eq!(
        world.account(Some(&token)).await,
        json!({"signedIn": false})
    );
    assert_eq!(
        world.session_count().await,
        0,
        "the session goes with the account"
    );
}

#[tokio::test]
async fn an_unknown_session_cookie_is_a_guest() {
    let world = world().await;
    assert_eq!(
        world.account(Some("forged")).await,
        json!({"signedIn": false})
    );
}

#[tokio::test]
async fn the_last_sign_in_elsewhere_is_shown() {
    let world = world().await;
    let (id, first) = world.signed_in("Lucky Otter").await;
    let signed_up = world.clock.now();

    world.clock.advance(Duration::days(1));
    accounts::record_sign_in(&world.pool, id, world.clock.now())
        .await
        .unwrap();
    let second = sessions::start(&world.pool, id, world.clock.now())
        .await
        .unwrap();
    let second_at = world.clock.now();

    let rfc3339 = |time: time::OffsetDateTime| {
        time.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    // The newer session sees the sign-in before its own; the older one sees the newer one.
    assert_eq!(
        world.account(Some(&second)).await["lastSignInAt"],
        rfc3339(signed_up)
    );
    assert_eq!(
        world.account(Some(&first)).await["lastSignInAt"],
        rfc3339(second_at)
    );
}

/// Like the browser client: it reads the token from the cookie and sends it back in a header.
#[tokio::test]
async fn a_signed_in_player_must_send_the_csrf_token_to_sign_out() {
    let world = world().await;
    let (account, token) = world.signed_in("Lucky Otter").await;

    let refused = world.send(post("/api/sign-out", Some(&token), None)).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(refused).await,
        json!({"error": "csrf", "message": "Forbidden"})
    );

    let signed_out = world
        .send(post("/api/sign-out", Some(&token), Some(CSRF)))
        .await;
    assert_eq!(signed_out.status(), StatusCode::NO_CONTENT);
    let cleared = cookie_named(&signed_out, "BEELOT_SESSION").unwrap();
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    let rotated = cookie_named(&signed_out, "XSRF-TOKEN").unwrap();
    assert!(
        !rotated.starts_with(&format!("XSRF-TOKEN={CSRF}")),
        "the CSRF token is rotated: {rotated}"
    );
    assert_eq!(world.session_count().await, 0);
    assert_eq!(world.account(Some(&token)).await["signedIn"], false);
    let sign_outs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM security_event WHERE type = 'SIGN_OUT' AND outcome = 'SUCCESS' AND account_id = ?",
    )
    .bind(account.hyphenated())
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(sign_outs, 1);
    assert_eq!(
        world.events("SESSION_EXPIRED").await,
        0,
        "a sign-out is not an expiry"
    );
}

#[tokio::test]
async fn guest_post_needs_no_csrf() {
    let world = world().await;
    let response = world.send(post("/api/tables", None, None)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn signed_in_post_without_csrf_is_403() {
    let world = world().await;
    let (_, token) = world.signed_in("Lucky Otter").await;
    assert_eq!(
        world
            .send(post("/api/tables", Some(&token), None))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let mismatched = Request::post("/api/tables")
        .header(
            header::COOKIE,
            format!("BEELOT_SESSION={token}; XSRF-TOKEN={CSRF}"),
        )
        .header("X-XSRF-TOKEN", "something-else")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"kind": "LOCAL"}).to_string()))
        .unwrap();
    assert_eq!(world.send(mismatched).await.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        world
            .send(post("/api/tables", Some(&token), Some(CSRF)))
            .await
            .status(),
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn passkey_ceremonies_need_the_csrf_token_even_for_guests() {
    let world = world().await;
    for path in [
        "/api/passkeys/sign-up/options",
        "/api/passkeys/sign-up",
        "/api/passkeys/sign-in/options",
        "/api/passkeys/sign-in",
    ] {
        assert_eq!(
            world.send(post(path, None, None)).await.status(),
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
}

#[tokio::test]
async fn the_server_gives_the_client_a_csrf_token_cookie() {
    let world = world().await;
    let response = world
        .send(Request::get("/api/account").body(Body::empty()).unwrap())
        .await;
    let cookie = cookie_named(&response, "XSRF-TOKEN").expect("an XSRF-TOKEN cookie");
    assert!(cookie.contains("Path=/"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("HttpOnly"), "the client reads it");

    let again = Request::get("/api/account")
        .header(header::COOKIE, format!("XSRF-TOKEN={CSRF}"))
        .body(Body::empty())
        .unwrap();
    assert!(
        cookie_named(&world.send(again).await, "XSRF-TOKEN").is_none(),
        "a client with one keeps it"
    );
}

#[tokio::test]
async fn the_session_cookie_is_http_only_and_lax() {
    let cookie = sessions::cookie("token", false);
    assert_eq!(
        cookie,
        "BEELOT_SESSION=token; HttpOnly; SameSite=Lax; Path=/"
    );
    assert_eq!(
        sessions::cookie("token", true),
        "BEELOT_SESSION=token; HttpOnly; SameSite=Lax; Path=/; Secure"
    );
}

// SessionLifetimeFilterTest

#[tokio::test]
async fn session_ends_after_thirty_days_even_when_active() {
    let world = world().await;
    let (_, token) = world.signed_in("Lucky Otter").await;
    for _ in 0..29 {
        world.clock.advance(Duration::days(1));
        assert_eq!(world.account(Some(&token)).await["signedIn"], true);
    }
    world
        .clock
        .advance(Duration::days(1) - Duration::minutes(1));
    assert_eq!(
        world.account(Some(&token)).await["signedIn"],
        true,
        "a younger session goes on"
    );
    world.clock.advance(Duration::minutes(1));
    assert_eq!(world.account(Some(&token)).await["signedIn"], false);
    assert_eq!(world.session_count().await, 0);
    assert_eq!(
        world.events("SESSION_EXPIRED").await,
        1,
        "an end the player did not ask for is an expiry"
    );
    world.account(Some(&token)).await;
    assert_eq!(world.events("SESSION_EXPIRED").await, 1, "recorded once");
}

#[tokio::test]
async fn idle_session_ends_after_seven_days() {
    let world = world().await;
    let (_, token) = world.signed_in("Lucky Otter").await;
    world
        .clock
        .advance(Duration::days(7) - Duration::seconds(1));
    assert_eq!(world.account(Some(&token)).await["signedIn"], true);
    world
        .clock
        .advance(Duration::days(7) - Duration::seconds(1));
    assert_eq!(
        world.account(Some(&token)).await["signedIn"],
        true,
        "each use restarts the idle time"
    );
    world.clock.advance(Duration::days(7));
    assert_eq!(world.account(Some(&token)).await["signedIn"], false);
    assert_eq!(world.events("SESSION_EXPIRED").await, 1);
}

#[tokio::test]
async fn the_sweep_ends_expired_sessions_and_records_each_once() {
    let world = world().await;
    let (_, idle) = world.signed_in("Idle Otter").await;
    world.clock.advance(Duration::days(6));
    let (_, fresh) = world.signed_in("Fresh Otter").await;
    world.clock.advance(Duration::days(1));
    assert_eq!(
        sessions::sweep_expired(&world.pool, world.clock.now())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sessions::sweep_expired(&world.pool, world.clock.now())
            .await
            .unwrap(),
        0
    );
    assert_eq!(world.events("SESSION_EXPIRED").await, 1);
    assert_eq!(world.account(Some(&idle)).await["signedIn"], false);
    assert_eq!(world.account(Some(&fresh)).await["signedIn"], true);
}

// AccountSessionsTest

#[tokio::test]
async fn ending_all_sessions_of_an_account_leaves_the_others() {
    let world = world().await;
    let (a, first) = world.signed_in("Ana").await;
    let second = sessions::start(&world.pool, a, world.clock.now())
        .await
        .unwrap();
    let (_, other) = world.signed_in("Ben").await;
    assert_eq!(sessions::end_all(&world.pool, a).await.unwrap(), 2);
    assert_eq!(world.account(Some(&first)).await["signedIn"], false);
    assert_eq!(world.account(Some(&second)).await["signedIn"], false);
    assert_eq!(world.account(Some(&other)).await["signedIn"], true);
    assert_eq!(world.events("SESSION_EXPIRED").await, 0, "ended on purpose");
}

#[tokio::test]
async fn a_session_keeps_its_sign_in_time_and_knows_whether_it_is_recent() {
    let world = world().await;
    let (_, token) = world.signed_in("Ana").await;
    let signed_in = world.clock.now();
    world.clock.advance(Duration::minutes(9));
    let session = sessions::lookup(&world.pool, &token, world.clock.now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.signed_in_at, signed_in);
    assert!(session.signed_in_recently(world.clock.now(), std::time::Duration::from_secs(600)));
    world.clock.advance(Duration::minutes(1));
    assert!(!session.signed_in_recently(world.clock.now(), std::time::Duration::from_secs(600)));
}

#[tokio::test]
async fn sessions_keep_only_the_hash_of_their_token() {
    let world = world().await;
    let (_, token) = world.signed_in("Ana").await;
    let stored: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM session")
        .fetch_one(&world.pool)
        .await
        .unwrap();
    assert_eq!(stored.len(), 32);
    assert_ne!(stored, token.as_bytes());
    assert!(token.len() >= 43, "a 256-bit token");
}
