//! The match history and the statistics, on MariaDB. Ports `MatchHistoryServiceTest` and
//! `MatchHistoryIntegrationTest`.

mod common;
#[path = "common/db.rs"]
mod db;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use beelot_game::scoring::{RoundResult, RoundSummary};
use beelot_game::{Difficulty, Team, Trump, Variant};
use beelot_server::app::router;
use beelot_server::auth::sessions;
use beelot_server::clock::Clock;
use beelot_server::db::accounts::{self, Account};
use beelot_server::db::matches;
use beelot_server::history::HistoryRecorder;
use beelot_server::tables::finished::{FinishedMatch, FinishedSeat, MatchSink, Mode};
use common::{body_json, test_state_with_clock};
use serde_json::{Value, json};
use sqlx::MySqlPool;
use time::macros::datetime;
use tower::ServiceExt;
use uuid::Uuid;

const CSRF: &str = "5f0c9a2e-test-csrf-token";

async fn account(pool: &MySqlPool, name: &str) -> Uuid {
    let account = Account::new(Uuid::new_v4(), name.into(), datetime!(2026-10-24 12:00 UTC));
    accounts::insert(pool, &account).await.unwrap();
    account.id
}

fn result(made: bool, capot: Option<Team>, north_south: i32, east_west: i32) -> RoundResult {
    RoundResult {
        north_south_card_points: 100,
        east_west_card_points: 62,
        north_south_dix_de_der: 10,
        east_west_dix_de_der: 0,
        north_south_belote_bonus: 20,
        east_west_belote_bonus: 0,
        contract_made: made,
        north_south_awarded: north_south,
        east_west_awarded: east_west,
        capot_team: capot,
        north_south_announcements: 0,
        east_west_announcements: 0,
        announcements: Vec::new(),
    }
}

fn round(
    declaring: Team,
    contract: i32,
    coinched: bool,
    made: bool,
    capot: Option<Team>,
    ns: i32,
    ew: i32,
) -> RoundSummary {
    RoundSummary {
        declaring_team: declaring,
        trump: Trump::Hearts,
        contract_value: Some(contract),
        coinched,
        result: result(made, capot, ns, ew),
    }
}

fn finished(
    account: Option<Uuid>,
    rounds: Vec<RoundSummary>,
    announcements: bool,
) -> FinishedMatch {
    let seat = |account, name: &str, bot| FinishedSeat {
        account,
        name: name.into(),
        bot,
    };
    FinishedMatch {
        id: Uuid::new_v4(),
        ended_at: datetime!(2026-10-24 13:00:00.5 UTC),
        mode: Mode::Solo,
        variant: Variant::Contree,
        difficulty: Some(Difficulty::Challenging),
        north_south_score: 1010,
        east_west_score: 600,
        winning_team: Team::NorthSouth,
        seats: vec![
            seat(account, "You", false),
            seat(None, "Camille", true),
            seat(None, "Luc", true),
            seat(None, "Manon", true),
        ],
        rounds,
        announcements,
        all_trumps: false,
        target: 1_000,
    }
}

async fn stored(pool: &MySqlPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM match_record")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn stores_the_match_with_its_seats_and_rounds_and_names_seats_after_their_account() {
    let pool = db::test_pool().await;
    let chloe = account(&pool, "Lucky Otter 42").await;
    let match_ = finished(
        Some(chloe),
        vec![
            round(Team::NorthSouth, 100, false, true, None, 20, 0),
            round(
                Team::EastWest,
                160,
                true,
                false,
                Some(Team::NorthSouth),
                520,
                0,
            ),
        ],
        false,
    );
    HistoryRecorder::new(pool.clone())
        .record(match_.clone())
        .await;

    let found = matches::played_by(&pool, chloe).await.unwrap();
    assert_eq!(found.len(), 1);
    let stored = &found[0];
    assert_eq!(stored.id, match_.id);
    assert_eq!(stored.ended_at, match_.ended_at);
    assert_eq!(stored.mode, Mode::Solo);
    assert_eq!(stored.variant, Variant::Contree);
    assert_eq!(stored.difficulty, Some(Difficulty::Challenging));
    assert_eq!(stored.winning_team, Team::NorthSouth);
    let names: Vec<&str> = stored.seats.iter().map(|seat| seat.name.as_str()).collect();
    assert_eq!(names, ["Lucky Otter 42", "Camille", "Luc", "Manon"]);
    let teams: Vec<Team> = stored.seats.iter().map(|seat| seat.team).collect();
    assert_eq!(
        teams,
        [
            Team::NorthSouth,
            Team::EastWest,
            Team::NorthSouth,
            Team::EastWest
        ]
    );
    assert_eq!(stored.seats[0].account, Some(chloe));
    assert!(stored.seats[1].bot);
    let first = &stored.rounds[0];
    assert_eq!(first.declaring_team, Team::NorthSouth);
    assert_eq!(first.trump, Trump::Hearts);
    assert_eq!(first.contract_value, Some(100));
    assert!(first.contract_made);
    assert_eq!(first.belote_team, Some(Team::NorthSouth));
    assert_eq!(first.capot_team, None);
    let second = &stored.rounds[1];
    assert!(second.coinched);
    assert!(!second.contract_made);
    assert_eq!(second.capot_team, Some(Team::NorthSouth));
    assert_eq!(second.north_south_points, 520);
}

#[tokio::test]
async fn records_announcements() {
    let pool = db::test_pool().await;
    let eve = account(&pool, "Eve").await;
    let mut summary = round(Team::NorthSouth, 100, false, true, None, 160, 52);
    summary.contract_value = None;
    summary.result.north_south_announcements = 50;
    HistoryRecorder::new(pool.clone())
        .record(finished(Some(eve), vec![summary], true))
        .await;
    let stored = &matches::played_by(&pool, eve).await.unwrap()[0];
    assert!(stored.announcements);
    assert_eq!(stored.rounds[0].north_south_announcements, 50);
    assert_eq!(stored.rounds[0].east_west_announcements, 0);
    assert_eq!(stored.rounds[0].contract_value, None);
}

#[tokio::test]
async fn records_the_match_target() {
    let pool = db::test_pool().await;
    let fay = account(&pool, "Fay").await;
    let mut match_ = finished(
        Some(fay),
        vec![round(Team::NorthSouth, 80, false, true, None, 0, 0)],
        false,
    );
    match_.target = 1_500;
    HistoryRecorder::new(pool.clone()).record(match_).await;
    assert_eq!(
        matches::played_by(&pool, fay).await.unwrap()[0].target,
        1_500
    );
}

#[tokio::test]
async fn records_whether_the_match_played_all_trumps() {
    let pool = db::test_pool().await;
    let hal = account(&pool, "Hal").await;
    let mut summary = round(Team::NorthSouth, 100, false, true, None, 160, 2);
    summary.trump = Trump::NoTrumps;
    let mut match_ = finished(Some(hal), vec![summary], false);
    match_.all_trumps = true;
    HistoryRecorder::new(pool.clone()).record(match_).await;
    let stored = &matches::played_by(&pool, hal).await.unwrap()[0];
    assert!(stored.all_trumps);
    assert_eq!(stored.rounds[0].trump, Trump::NoTrumps);
}

#[tokio::test]
async fn matches_recorded_before_the_target_read_as_played_to_one_thousand() {
    let pool = db::test_pool().await;
    let column: Option<String> = sqlx::query_scalar(
        "SELECT COLUMN_DEFAULT FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'match_record' AND COLUMN_NAME = 'target'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(column.as_deref(), Some("1000"));
}

#[tokio::test]
async fn stores_a_match_once() {
    let pool = db::test_pool().await;
    let dan = account(&pool, "Dan").await;
    let match_ = finished(
        Some(dan),
        vec![round(Team::NorthSouth, 80, false, true, None, 0, 0)],
        false,
    );
    let recorder = HistoryRecorder::new(pool.clone());
    recorder.record(match_.clone()).await;
    recorder.record(match_).await;
    assert_eq!(stored(&pool).await, 1);
}

#[tokio::test]
async fn does_not_store_a_match_without_a_signed_in_player() {
    let pool = db::test_pool().await;
    HistoryRecorder::new(pool.clone())
        .record(finished(None, Vec::new(), false))
        .await;
    assert_eq!(stored(&pool).await, 0);
}

#[tokio::test]
async fn a_match_finished_after_deletion_is_recorded_anonymously() {
    let pool = db::test_pool().await;
    let kept = account(&pool, "Kept").await;
    let deleted = Uuid::new_v4();
    let mut match_ = finished(Some(deleted), Vec::new(), false);
    match_.seats[2].account = Some(kept);
    HistoryRecorder::new(pool.clone()).record(match_).await;
    let stored = &matches::played_by(&pool, kept).await.unwrap()[0];
    assert_eq!(stored.seats[0].account, None);
    assert_eq!(stored.seats[0].name, "Deleted player");
}

#[tokio::test]
async fn a_database_failure_never_stops_the_game() {
    let pool = db::test_pool().await;
    let ana = account(&pool, "Ana").await;
    pool.close().await;
    HistoryRecorder::new(pool)
        .record(finished(Some(ana), Vec::new(), false))
        .await;
}

#[tokio::test]
async fn a_guest_is_asked_to_sign_in_to_see_statistics() {
    let pool = db::test_pool().await;
    let (state, _) = test_state_with_clock();
    let app = router(state.with_pool(pool));
    let response = app
        .oneshot(Request::get("/api/statistics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        body_json(response).await,
        json!({"error": "sign-in-required", "message": "Sign in to see your statistics."})
    );
}

#[tokio::test]
async fn a_player_sees_only_their_own_statistics() {
    let pool = db::test_pool().await;
    let (state, clock) = test_state_with_clock();
    let app = router(state.with_pool(pool.clone()));
    let finn = account(&pool, "Finn").await;
    let gail = account(&pool, "Gail").await;
    let recorder = HistoryRecorder::new(pool.clone());
    for player in [finn, gail] {
        recorder
            .record(finished(
                Some(player),
                vec![round(Team::NorthSouth, 100, false, true, None, 150, 12)],
                false,
            ))
            .await;
    }
    let token = sessions::start(&pool, finn, clock.now()).await.unwrap();
    let request = Request::get("/api/statistics")
        .header(header::COOKIE, format!("BEELOT_SESSION={token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let statistics = body_json(response).await;
    assert_eq!(statistics["overall"]["played"], 1);
    assert_eq!(statistics["overall"]["won"], 1);
    assert_eq!(statistics["recentMatches"][0]["partner"], "Luc");
    assert_eq!(statistics["recentMatches"][0]["teamScore"], 1010);
    assert_eq!(
        statistics["recentMatches"][0]["endedAt"],
        "2026-10-24T13:00:00.5Z"
    );
    assert_eq!(statistics["recentMatches"][0]["mode"], "SOLO");
    assert_eq!(statistics["contracts"]["taken"], 1);
    assert_eq!(statistics["averagePointsPerRound"], 150.0);
}

/// From the signed-in player's first request to the stored match.
#[tokio::test]
async fn a_signed_in_players_solo_match_is_stored_when_won() {
    let pool = db::test_pool().await;
    let (state, clock) = test_state_with_clock();
    let app = router(
        state
            .with_pool(pool.clone())
            .with_sink(Arc::new(HistoryRecorder::new(pool.clone()))),
    );
    let eve = account(&pool, "Eve 12").await;
    let session = sessions::start(&pool, eve, clock.now()).await.unwrap();
    let cookies = format!("BEELOT_SESSION={session}; XSRF-TOKEN={CSRF}");
    let send = |uri: String, body: Value, token: Option<String>| {
        let mut request = Request::post(uri)
            .header(header::COOKIE, &cookies)
            .header("X-XSRF-TOKEN", CSRF)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        app.clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
    };
    let created = send(
        "/api/tables".into(),
        json!({"kind": "LOCAL", "variant": "CONTREE", "difficulty": "RELAXED"}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = body_json(created).await;
    let (table, token) = (
        created["tableId"].as_str().unwrap().to_owned(),
        created["playerToken"].as_str().unwrap().to_owned(),
    );
    let actions = format!("/api/tables/{table}/actions");

    let mut view = body_json(
        send(
            actions.clone(),
            json!({"type": "rematch"}),
            Some(token.clone()),
        )
        .await
        .unwrap(),
    )
    .await;
    for guard in 0.. {
        assert!(guard < 3000, "the match did not end");
        if view["match"]["complete"] == true {
            break;
        }
        let action = if view["bidding"]["complete"] != true {
            if view["bidding"]["highestBid"].as_i64().unwrap() < 160 {
                json!({"type": "contract", "value": 160, "suit": "SPADES"})
            } else {
                json!({"type": "pass"})
            }
        } else if !view["board"]["roundResult"].is_null() {
            json!({"type": "nextRound"})
        } else if view["board"]["reviewingCompletedTrick"] == true {
            json!({"type": "continueTrick"})
        } else {
            let card = &view["board"]["legalCards"][0];
            json!({"type": "card", "rank": card["rank"], "suit": card["suit"]})
        };
        let response = send(actions.clone(), action, Some(token.clone()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        view = body_json(response).await;
    }
    let mut found = Vec::new();
    for _ in 0..200 {
        found = matches::played_by(&pool, eve).await.unwrap();
        if !found.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(found.len(), 1, "the match is handed to the history");
    let stored = &found[0];
    assert_eq!(stored.seats[0].name, "Eve 12");
    assert_eq!(stored.mode, Mode::Solo);
    assert_eq!(
        stored.north_south_score,
        stored
            .rounds
            .iter()
            .map(|round| round.north_south_points)
            .sum::<i32>()
    );
}
