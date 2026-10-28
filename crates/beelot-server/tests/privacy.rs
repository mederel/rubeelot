//! Privacy, on MariaDB: security events and their caps, the data export, account deletion, retention and the policy
//! values. Ports `SecurityEventsTest`, `DataExportIntegrationTest`, `AccountDeletionIntegrationTest`,
//! `RetentionJobIntegrationTest`, `PolicyInfoTest` and `UnsetPolicyInfoTest`.

mod common;
#[path = "common/db.rs"]
mod db;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use beelot_game::scoring::{RoundResult, RoundSummary};
use beelot_game::{Team, Trump, Variant};
use beelot_server::app::{AppState, router};
use beelot_server::auth::sessions;
use beelot_server::clock::{Clock, FakeClock};
use beelot_server::config::{Config, Limits, PrivacyConfig};
use beelot_server::db::accounts::{self, Account};
use beelot_server::db::{matches, passkeys};
use beelot_server::history::HistoryRecorder;
use beelot_server::privacy::events::{EventType, Outcome, SecurityEvents};
use beelot_server::privacy::retention;
use beelot_server::tables::finished::{FinishedMatch, FinishedSeat, MatchSink, Mode};
use common::{body_json, state_with};
use serde_json::{Value, json};
use sqlx::MySqlPool;
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;
use uuid::Uuid;

const CSRF: &str = "5f0c9a2e-test-csrf-token";

struct World {
    app: Router,
    state: AppState,
    clock: Arc<FakeClock>,
    pool: MySqlPool,
}

async fn world_with(config: Config) -> World {
    let pool = db::test_pool().await;
    let (state, clock) = state_with(config);
    let state = state
        .with_pool(pool.clone())
        .with_sink(Arc::new(HistoryRecorder::new(pool.clone())));
    World {
        app: router(state.clone()),
        state,
        clock,
        pool,
    }
}

async fn world() -> World {
    world_with(Config::default()).await
}

impl World {
    async fn account(&self, name: &str) -> Uuid {
        let account = Account::new(Uuid::new_v4(), name.into(), self.clock.now());
        accounts::insert(&self.pool, &account).await.unwrap();
        account.id
    }

    async fn signed_in(&self, name: &str) -> (Uuid, String) {
        let account = self.account(name).await;
        (
            account,
            sessions::start(&self.pool, account, self.clock.now())
                .await
                .unwrap(),
        )
    }

    async fn send(
        &self,
        method: &str,
        uri: &str,
        session: Option<&str>,
        body: Option<Value>,
    ) -> axum::http::Response<Body> {
        let mut cookies = vec![format!("XSRF-TOKEN={CSRF}")];
        if let Some(session) = session {
            cookies.push(format!("BEELOT_SESSION={session}"));
        }
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, cookies.join("; "))
            .header("X-XSRF-TOKEN", CSRF);
        let body = match body {
            Some(body) => {
                request = request.header(header::CONTENT_TYPE, "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        self.app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap()
    }

    async fn count(&self, sql: &str, account: Uuid) -> i64 {
        sqlx::query_scalar(sql)
            .bind(account.hyphenated().to_string())
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn all(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }

    async fn event_at(&self, account: Uuid, at: OffsetDateTime) {
        beelot_server::db::events::insert(
            &self.pool,
            "SIGN_IN",
            "SUCCESS",
            Some(account),
            None,
            at,
        )
        .await
        .unwrap();
    }
}

fn round() -> RoundSummary {
    RoundSummary {
        declaring_team: Team::NorthSouth,
        trump: Trump::Hearts,
        contract_value: Some(90),
        coinched: false,
        result: RoundResult {
            north_south_card_points: 100,
            east_west_card_points: 62,
            north_south_dix_de_der: 10,
            east_west_dix_de_der: 0,
            north_south_belote_bonus: 0,
            east_west_belote_bonus: 0,
            contract_made: true,
            north_south_awarded: 190,
            east_west_awarded: 0,
            capot_team: None,
            north_south_announcements: 0,
            east_west_announcements: 0,
            announcements: Vec::new(),
        },
    }
}

/// A private match won by North–South: the account sits North, the other account East.
fn private_match(account: Uuid, other: Uuid, at: OffsetDateTime) -> FinishedMatch {
    let seat = |account, name: &str, bot| FinishedSeat {
        account,
        name: name.into(),
        bot,
    };
    FinishedMatch {
        id: Uuid::new_v4(),
        ended_at: at,
        mode: Mode::Private,
        variant: Variant::Contree,
        difficulty: None,
        north_south_score: 1010,
        east_west_score: 600,
        winning_team: Team::NorthSouth,
        seats: vec![
            seat(Some(account), "You", false),
            seat(Some(other), "Other", false),
            seat(None, "Camille", true),
            seat(None, "Luc", true),
        ],
        rounds: vec![round()],
        announcements: false,
        all_trumps: false,
        target: 1_000,
    }
}

// SecurityEventsTest

#[tokio::test]
async fn recording_never_fails() {
    let world = world().await;
    let events = SecurityEvents::new(world.pool.clone(), &Limits::default(), world.clock.clone());
    world.pool.close().await;
    events
        .record(
            EventType::SignIn,
            Outcome::Success,
            Some(Uuid::new_v4()),
            None,
        )
        .await;
}

#[tokio::test]
async fn anonymous_events_stop_at_the_limit_per_ip() {
    let world = world().await;
    let limits = Limits {
        anonymous_events_per_ip_per_hour: 3,
        ..Limits::default()
    };
    let events = SecurityEvents::new(world.pool.clone(), &limits, world.clock.clone());
    let ip = |text: &str| Some(text.parse().unwrap());
    for _ in 0..5 {
        events
            .record(EventType::SignIn, Outcome::Failure, None, ip("10.0.0.1"))
            .await;
    }
    events
        .record(
            EventType::RateLimited,
            Outcome::Refused,
            None,
            ip("10.0.0.1"),
        )
        .await;
    events
        .record(EventType::SignUp, Outcome::Failure, None, ip("10.0.0.2"))
        .await;
    assert_eq!(
        world
            .all("SELECT COUNT(*) FROM security_event WHERE ip_address = '10.0.0.1'")
            .await,
        3
    );
    assert_eq!(
        world
            .all("SELECT COUNT(*) FROM security_event WHERE ip_address = '10.0.0.2'")
            .await,
        1
    );

    world.clock.advance(Duration::hours(1));
    events
        .record(EventType::SignIn, Outcome::Failure, None, ip("10.0.0.1"))
        .await;
    assert_eq!(
        world
            .all("SELECT COUNT(*) FROM security_event WHERE ip_address = '10.0.0.1'")
            .await,
        4
    );
}

#[tokio::test]
async fn anonymous_events_stop_at_the_overall_limit() {
    let world = world().await;
    let limits = Limits {
        anonymous_events_per_hour: 4,
        ..Limits::default()
    };
    let events = SecurityEvents::new(world.pool.clone(), &limits, world.clock.clone());
    for index in 0..10 {
        let ip = format!("10.0.0.{index}").parse().ok();
        events
            .record(EventType::SignIn, Outcome::Failure, None, ip)
            .await;
    }
    assert_eq!(world.all("SELECT COUNT(*) FROM security_event").await, 4);
}

#[tokio::test]
async fn account_events_and_deletions_are_never_limited() {
    let world = world().await;
    let limits = Limits {
        anonymous_events_per_ip_per_hour: 1,
        anonymous_events_per_hour: 1,
        ..Limits::default()
    };
    let events = SecurityEvents::new(world.pool.clone(), &limits, world.clock.clone());
    let account = Uuid::new_v4();
    let ip = "10.0.0.1".parse().ok();
    for _ in 0..3 {
        events
            .record(EventType::SignIn, Outcome::Success, Some(account), ip)
            .await;
        events
            .record(EventType::Deletion, Outcome::Success, None, ip)
            .await;
    }
    assert_eq!(
        world
            .all("SELECT COUNT(*) FROM security_event WHERE type = 'SIGN_IN'")
            .await,
        3
    );
    assert_eq!(
        world
            .all("SELECT COUNT(*) FROM security_event WHERE type = 'DELETION'")
            .await,
        3
    );
}

// DataExportIntegrationTest

#[tokio::test]
async fn export_holds_account_events_and_matches() {
    let world = world().await;
    let (account, session) = world.signed_in("Lucky Otter 42").await;
    let other = world.account("Other Otter 7").await;
    world
        .state
        .events
        .record(
            EventType::SignIn,
            Outcome::Success,
            Some(account),
            "127.0.0.1".parse().ok(),
        )
        .await;
    HistoryRecorder::new(world.pool.clone())
        .record(private_match(account, other, world.clock.now()))
        .await;

    let response = world
        .send("GET", "/api/account/export", Some(&session), None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"beelot-data-2026-10-24.json\""
    );
    let export = body_json(response).await;
    assert_eq!(export["format"], "beelot-export-2");
    assert_eq!(export["exportedAt"], "2026-10-24T12:00:00Z");
    assert_eq!(export["account"]["id"], account.to_string());
    assert_eq!(export["account"]["displayName"], "Lucky Otter 42");
    assert_eq!(export["account"]["createdAt"], "2026-10-24T12:00:00Z");
    assert_eq!(export["account"]["lastSignInAt"], "2026-10-24T12:00:00Z");
    assert!(
        export["account"]
            .get("previousSignInAt")
            .is_some_and(Value::is_null)
    );
    assert!(export["account"]["passkey"].is_null());
    let events = export["securityEvents"].as_array().unwrap();
    assert!(events.iter().any(|event| event["type"] == "SIGN_IN"));
    assert_eq!(events[0]["ipAddress"], "127.0.0.1");
    let matches = export["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    let played = &matches[0];
    assert_eq!(played["mode"], "PRIVATE");
    assert_eq!(played["variant"], "CONTREE");
    assert!(played["difficulty"].is_null());
    assert_eq!(played["yourTeam"], "NORTH_SOUTH");
    assert_eq!(played["won"], true);
    assert_eq!(played["northSouthScore"], 1010);
    assert_eq!(played["seats"].as_array().unwrap().len(), 4);
    assert_eq!(played["seats"][1]["name"], "Other Otter 7");
    assert_eq!(played["seats"][1]["position"], 1);
    assert_eq!(played["seats"][2]["bot"], true);
    assert_eq!(played["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(played["rounds"][0]["contractValue"], 90);
    assert_eq!(played["rounds"][0]["trump"], "HEARTS");
    assert_eq!(played["rounds"][0]["coinched"], false);
    assert!(played["rounds"][0]["capotTeam"].is_null());
    assert_eq!(played["announcements"], false);
    assert_eq!(played["target"], 1_000);
    assert_eq!(played["allTrumps"], false);
    assert_eq!(played["rounds"][0]["northSouthAnnouncements"], 0);
    assert_eq!(
        world
            .count(
                "SELECT COUNT(*) FROM security_event WHERE type = 'EXPORT' AND account_id = ?",
                account
            )
            .await,
        1
    );
}

#[tokio::test]
async fn export_holds_no_account_ids_of_others_and_no_credentials() {
    let world = world().await;
    let (account, session) = world.signed_in("Lucky Otter 42").await;
    let other = world.account("Other Otter 7").await;
    HistoryRecorder::new(world.pool.clone())
        .record(private_match(account, other, world.clock.now()))
        .await;
    passkeys::insert(
        &world.pool,
        account,
        b"credential-id-bytes",
        "{\"cred\":{\"cred_id\":\"Y3JlZGVudGlhbC1pZA\",\"key\":\"publicKey\"}}",
        world.clock.now(),
    )
    .await
    .unwrap();
    passkeys::update(
        &world.pool,
        account,
        "{\"cred\":{\"cred_id\":\"Y3JlZGVudGlhbC1pZA\"}}",
        world.clock.now(),
    )
    .await
    .unwrap();
    let response = world
        .send("GET", "/api/account/export", Some(&session), None)
        .await;
    let body = String::from_utf8(common::body_bytes(response).await).unwrap();
    assert!(!body.contains(&other.to_string()));
    assert!(
        !body.contains("cred")
            && !body.contains("publicKey")
            && !body.contains("Y3JlZGVudGlhbC1pZA"),
        "{body}"
    );
    let export: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        export["account"]["passkey"]["createdAt"],
        "2026-10-24T12:00:00Z"
    );
    assert_eq!(
        export["account"]["passkey"]["lastUsedAt"],
        "2026-10-24T12:00:00Z"
    );
}

#[tokio::test]
async fn a_second_export_within_an_hour_is_refused() {
    let world = world().await;
    let (_, session) = world.signed_in("Lucky Otter 42").await;
    assert_eq!(
        world
            .send("GET", "/api/account/export", Some(&session), None)
            .await
            .status(),
        StatusCode::OK
    );
    world
        .clock
        .advance(Duration::minutes(20) + Duration::seconds(30));
    let refused = world
        .send("GET", "/api/account/export", Some(&session), None)
        .await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        refused.headers()[header::RETRY_AFTER],
        (40 * 60).to_string()
    );
    assert_eq!(body_json(refused).await, json!({"minutesLeft": 40}));
    world.clock.advance(Duration::minutes(40));
    assert_eq!(
        world
            .send("GET", "/api/account/export", Some(&session), None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_guest_cannot_export_or_delete() {
    let world = world().await;
    assert_eq!(
        world
            .send("GET", "/api/account/export", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        world
            .send("DELETE", "/api/account", None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

// AccountDeletionIntegrationTest

#[tokio::test]
async fn deletion_removes_the_account_its_passkey_events_and_sessions() {
    let world = world().await;
    let (account, session) = world.signed_in("Lucky Otter 42").await;
    let second = sessions::start(&world.pool, account, world.clock.now())
        .await
        .unwrap();
    passkeys::insert(&world.pool, account, b"credential", "{}", world.clock.now())
        .await
        .unwrap();
    world.event_at(account, world.clock.now()).await;

    let deleted = world
        .send("DELETE", "/api/account", Some(&session), None)
        .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let cookies: Vec<&str> = deleted
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect();
    assert!(
        cookies
            .iter()
            .any(|cookie| cookie.starts_with("BEELOT_SESSION=;")),
        "{cookies:?}"
    );

    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", account)
            .await,
        0
    );
    assert_eq!(
        world
            .count(
                "SELECT COUNT(*) FROM security_event WHERE account_id = ?",
                account
            )
            .await,
        0
    );
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM passkey WHERE account_id = ?", account)
            .await,
        0
    );
    assert_eq!(world.all("SELECT COUNT(*) FROM session").await, 0);
    assert_eq!(
        world.all("SELECT COUNT(*) FROM security_event WHERE type = 'DELETION' AND outcome = 'SUCCESS' AND account_id IS NULL").await,
        1
    );
    let response = world.send("GET", "/api/account", Some(&second), None).await;
    assert_eq!(
        body_json(response).await["signedIn"],
        false,
        "every session ends"
    );
}

#[tokio::test]
async fn matches_stay_with_the_seat_renamed() {
    let world = world().await;
    let (account, session) = world.signed_in("Lucky Otter 42").await;
    let other = world.account("Other Otter 7").await;
    let finished = private_match(account, other, world.clock.now());
    HistoryRecorder::new(world.pool.clone())
        .record(finished.clone())
        .await;
    assert_eq!(
        world
            .send("DELETE", "/api/account", Some(&session), None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    let kept = &matches::played_by(&world.pool, other).await.unwrap()[0];
    assert_eq!(kept.id, finished.id);
    assert_eq!(kept.seats[0].account, None);
    assert_eq!(kept.seats[0].name, "Deleted player");
    assert_eq!(kept.seats[1].account, Some(other));
    assert_eq!(kept.seats[1].name, "Other Otter 7");
}

#[tokio::test]
async fn deletion_needs_recent_sign_in() {
    let world = world().await;
    let (account, session) = world.signed_in("Lucky Otter 42").await;
    world
        .clock
        .advance(Duration::minutes(9) + Duration::seconds(59));
    let (other, recent) = world.signed_in("Recent Otter").await;
    world.clock.advance(Duration::seconds(1));
    let refused = world
        .send("DELETE", "/api/account", Some(&session), None)
        .await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        body_json(refused).await,
        json!({"error": "reauthenticate", "message": "Sign in again to delete your account.", "reason": "reauthenticate"})
    );
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", account)
            .await,
        1
    );
    assert_eq!(
        world
            .send("DELETE", "/api/account", Some(&recent), None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", other)
            .await,
        0
    );
}

#[tokio::test]
async fn deletion_needs_the_csrf_token() {
    let world = world().await;
    let (account, session) = world.signed_in("Lucky Otter 42").await;
    let request = Request::delete("/api/account")
        .header(header::COOKIE, format!("BEELOT_SESSION={session}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        world.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", account)
            .await,
        1
    );
}

/// A signed-in player seated at a live private table deletes their account.
#[tokio::test]
async fn deleted_account_seat_goes_to_bot() {
    let world = world().await;
    let (owner, owner_session) = world.signed_in("Owner Otter").await;
    let (_, ben_session) = world.signed_in("Ben Badger").await;
    let created = body_json(
        world
            .send(
                "POST",
                "/api/tables",
                Some(&owner_session),
                Some(json!({"kind": "PRIVATE", "variant": "CLASSIC", "playerName": "Ana"})),
            )
            .await,
    )
    .await;
    let table = created["tableId"].as_str().unwrap().to_owned();
    let owner_token = created["playerToken"].as_str().unwrap().to_owned();
    let joined = body_json(
        world
            .send(
                "POST",
                "/api/tables/join",
                Some(&ben_session),
                Some(json!({"code": created["code"], "playerName": "Ben"})),
            )
            .await,
    )
    .await;
    let ben_token = joined["playerToken"].as_str().unwrap().to_owned();
    let act = |token: String, action: Value| {
        let uri = format!("/api/tables/{table}/actions");
        let app = world.app.clone();
        let session = owner_session.clone();
        async move {
            let request = Request::post(uri)
                .header(
                    header::COOKIE,
                    format!("BEELOT_SESSION={session}; XSRF-TOKEN={CSRF}"),
                )
                .header("X-XSRF-TOKEN", CSRF)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(action.to_string()))
                .unwrap();
            app.oneshot(request).await.unwrap()
        }
    };
    let ready = json!({"type": "ready", "ready": true});
    assert_eq!(
        act(owner_token.clone(), ready.clone()).await.status(),
        StatusCode::OK
    );
    assert_eq!(act(ben_token.clone(), ready).await.status(), StatusCode::OK);
    let started = act(
        owner_token.clone(),
        json!({"type": "start", "fillWithBots": true}),
    )
    .await;
    assert_eq!(started.status(), StatusCode::OK);

    assert_eq!(
        world
            .send("DELETE", "/api/account", Some(&ben_session), None)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );

    let mut view = body_json(
        world
            .app
            .clone()
            .oneshot(common::get_request(
                &format!("/api/tables/{table}"),
                Some(&owner_token),
            ))
            .await
            .unwrap(),
    )
    .await;
    let seat = &view["table"]["seats"][1];
    assert_eq!(seat["name"], "Deleted player");
    assert_eq!(seat["connectionState"], "BOT_TAKEOVER");
    let refused = act(ben_token.clone(), json!({"type": "pass"})).await;
    assert_eq!(
        refused.status(),
        StatusCode::UNAUTHORIZED,
        "the deleted player's table token stops working"
    );

    // The owner plays the match to its end.
    for guard in 0.. {
        assert!(guard < 3000, "the match did not end");
        if view["match"]["complete"] == true {
            break;
        }
        let action = if view["bidding"]["complete"] != true {
            let upturned = view["bidding"]["upturnedCard"]["suit"]
                .as_str()
                .unwrap()
                .to_owned();
            if view["bidding"]["round"] == 1 {
                json!({"type": "trump", "suit": upturned})
            } else {
                json!({"type": "trump", "suit": if upturned == "CLUBS" { "SPADES" } else { "CLUBS" }})
            }
        } else if !view["board"]["roundResult"].is_null() {
            json!({"type": "nextRound"})
        } else if view["board"]["reviewingCompletedTrick"] == true {
            json!({"type": "continueTrick"})
        } else {
            let card = &view["board"]["legalCards"][0];
            json!({"type": "card", "rank": card["rank"], "suit": card["suit"]})
        };
        let response = act(owner_token.clone(), action).await;
        assert_eq!(response.status(), StatusCode::OK);
        view = body_json(response).await;
    }
    let mut stored = Vec::new();
    for _ in 0..200 {
        stored = matches::played_by(&world.pool, owner).await.unwrap();
        if !stored.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].seats[1].account, None);
    assert_eq!(stored[0].seats[1].name, "Deleted player");
    assert!(stored[0].seats[1].bot);
    assert_eq!(stored[0].seats[0].account, Some(owner));
}

// RetentionJobIntegrationTest

#[tokio::test]
async fn old_events_are_purged() {
    let world = world().await;
    let old = world.account("Old").await;
    let recent = world.account("Recent").await;
    world
        .event_at(old, world.clock.now() - Duration::days(366))
        .await;
    world
        .event_at(recent, world.clock.now() - Duration::days(364))
        .await;
    retention::run(&world.state, world.clock.now()).await;
    assert_eq!(
        world
            .count(
                "SELECT COUNT(*) FROM security_event WHERE account_id = ?",
                old
            )
            .await,
        0
    );
    assert_eq!(
        world
            .count(
                "SELECT COUNT(*) FROM security_event WHERE account_id = ?",
                recent
            )
            .await,
        1
    );
}

#[tokio::test]
async fn accounts_inactive_for_three_years_are_deleted_with_their_passkey() {
    let world = world().await;
    let inactive = world.account("Inactive").await;
    passkeys::insert(
        &world.pool,
        inactive,
        b"credential",
        "{}",
        world.clock.now(),
    )
    .await
    .unwrap();
    world.clock.advance(Duration::days(3 * 365 + 1));
    let active = world.account("Active").await;
    retention::run(&world.state, world.clock.now()).await;
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", inactive)
            .await,
        0
    );
    assert_eq!(
        world
            .count(
                "SELECT COUNT(*) FROM passkey WHERE account_id = ?",
                inactive
            )
            .await,
        0
    );
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", active)
            .await,
        1
    );
    assert_eq!(
        world
            .all(
                "SELECT COUNT(*) FROM security_event WHERE type = 'DELETION' AND ip_address IS NULL"
            )
            .await,
        1
    );
}

#[tokio::test]
async fn accounts_signed_in_within_three_years_are_kept() {
    let world = world().await;
    let account = world.account("Kept").await;
    world.clock.advance(Duration::days(3 * 365 - 2));
    retention::run(&world.state, world.clock.now()).await;
    assert_eq!(
        world
            .count("SELECT COUNT(*) FROM account WHERE id = ?", account)
            .await,
        1
    );
}

#[test]
fn the_retention_job_runs_at_half_past_three_utc() {
    use time::macros::datetime;
    assert_eq!(
        retention::next_run(datetime!(2026-10-24 12:00 UTC)),
        datetime!(2026-10-25 3:30 UTC)
    );
    assert_eq!(
        retention::next_run(datetime!(2026-10-24 3:29:59 UTC)),
        datetime!(2026-10-24 3:30 UTC)
    );
    assert_eq!(
        retention::next_run(datetime!(2026-10-24 3:30 UTC)),
        datetime!(2026-10-25 3:30 UTC)
    );
}

// PolicyInfoTest and UnsetPolicyInfoTest

#[tokio::test]
async fn policy_info_comes_from_the_environment() {
    let privacy = PrivacyConfig {
        controller: "Beelot SAS".into(),
        contact: " privacy@example.com ".into(),
        hosting_provider: "Hetzner".into(),
        backup_retention_days: "30".into(),
    };
    let world = world_with(Config {
        privacy,
        ..Config::default()
    })
    .await;
    let response = world
        .send("GET", "/api/privacy/policy-info", None, None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_json(response).await,
        json!({"controller": "Beelot SAS", "contact": "privacy@example.com", "hostingProvider": "Hetzner", "backupRetentionDays": "30"})
    );
}

#[tokio::test]
async fn unset_policy_values_are_left_out() {
    let world = world_with(Config {
        privacy: PrivacyConfig {
            contact: "  ".into(),
            ..PrivacyConfig::default()
        },
        ..Config::default()
    })
    .await;
    let response = world
        .send("GET", "/api/privacy/policy-info", None, None)
        .await;
    assert_eq!(body_json(response).await, json!({}));
    assert_eq!(
        beelot_server::privacy::unset_policy_values(&world.state.config.privacy),
        [
            "BEELOT_PRIVACY_CONTROLLER",
            "BEELOT_PRIVACY_CONTACT",
            "BEELOT_HOSTING_PROVIDER",
            "BEELOT_BACKUP_RETENTION_DAYS"
        ]
    );
}
