//! Server-sent events: each player's stream of snapshots, and the presence it carries.

mod common;

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use beelot_server::app::{AppState, router};
use beelot_server::clock::Clock;
use common::{call, json_request, test_state_with_clock};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Player {
    table: String,
    token: String,
}

async fn private(app: &Router) -> (Player, String) {
    let body = json!({"kind": "PRIVATE", "variant": "CONTREE", "playerName": "Ana"});
    let (status, created) = call(app, json_request("POST", "/api/tables", None, body)).await;
    assert_eq!(status, StatusCode::CREATED);
    let player = Player {
        table: created["tableId"].as_str().unwrap().into(),
        token: created["playerToken"].as_str().unwrap().into(),
    };
    (player, created["code"].as_str().unwrap().into())
}

async fn join(app: &Router, code: &str, name: &str) -> Player {
    let body = json!({"code": code, "playerName": name});
    let (_, joined) = call(app, json_request("POST", "/api/tables/join", None, body)).await;
    Player {
        table: joined["tableId"].as_str().unwrap().into(),
        token: joined["playerToken"].as_str().unwrap().into(),
    }
}

async fn act(app: &Router, player: &Player, action: Value) -> Value {
    let uri = format!("/api/tables/{}/actions", player.table);
    let (status, body) = call(app, json_request("POST", &uri, Some(&player.token), action)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn open(app: &Router, player: &Player) -> Response<Body> {
    let uri = format!("/api/tables/{}/events?token={}", player.table, player.token);
    let response = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    response
}

/// The next event of the stream, as its name and its JSON data; keep-alive comments are skipped.
async fn next_event(body: &mut Body) -> (String, Value) {
    let mut text = String::new();
    loop {
        if let Some(end) = text.find("\n\n") {
            let event: String = text.drain(..end + 2).collect();
            let mut name = String::from("message");
            let mut data = String::new();
            for line in event.lines() {
                if let Some(value) = line.strip_prefix("event:") {
                    name = value.trim().into();
                } else if let Some(value) = line.strip_prefix("data:") {
                    data.push_str(value.trim_start());
                }
            }
            if data.is_empty() {
                continue;
            }
            return (name, serde_json::from_str(&data).unwrap());
        }
        let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
            .await
            .expect("an event within 5 s")
            .expect("the stream is open")
            .unwrap();
        if let Ok(bytes) = frame.into_data() {
            text.push_str(std::str::from_utf8(&bytes).unwrap());
        }
    }
}

/// Waits until the seat's connection matches: a stream's end is handled off the async threads.
async fn wait_for(state: &AppState, table: &str, seat: usize, prefix: &str) {
    for _ in 0..200 {
        if connection(state, table, seat).starts_with(prefix) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!(
        "seat {seat} never became {prefix}: {}",
        connection(state, table, seat)
    );
}

fn connection(state: &AppState, table: &str, seat: usize) -> String {
    let id = table.parse().unwrap();
    let handle = state.registry.handle(id).unwrap();
    let table = handle.state.lock().unwrap();
    format!("{:?}", table.lobby().seat(seat).unwrap().connection)
}

#[tokio::test]
async fn stream_sends_snapshot_on_connect() {
    let (state, _) = test_state_with_clock();
    let app = router(state);
    let (owner, _) = private(&app).await;
    let mut body = open(&app, &owner).await.into_body();
    let (name, snapshot) = next_event(&mut body).await;
    assert_eq!(name, "snapshot");
    assert_eq!(snapshot["tableId"], owner.table);
    assert_eq!(snapshot["you"], 0);
}

#[tokio::test]
async fn stream_sends_snapshot_after_other_players_action() {
    let (state, _) = test_state_with_clock();
    let app = router(state);
    let (owner, code) = private(&app).await;
    let mut body = open(&app, &owner).await.into_body();
    let (_, first) = next_event(&mut body).await;

    let ben = join(&app, &code, "Ben").await;
    let (_, joined) = next_event(&mut body).await;
    assert!(joined["version"].as_u64() > first["version"].as_u64());
    assert_eq!(joined["table"]["seats"][1]["name"], "Ben");
    assert_eq!(joined["you"], 0, "each stream shows its own player's view");

    act(&app, &ben, json!({"type": "ready", "ready": true})).await;
    let (_, ready) = next_event(&mut body).await;
    assert_eq!(ready["table"]["seats"][1]["ready"], true);
}

#[tokio::test]
async fn header_or_query_token_is_required() {
    let (state, _) = test_state_with_clock();
    let app = router(state);
    let (owner, _) = private(&app).await;
    let uri = format!("/api/tables/{}/events", owner.table);
    let (status, error) = call(&app, Request::get(&uri).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!error.to_string().contains(&owner.token));
    let (status, _) = call(
        &app,
        Request::get(format!("{uri}?token=forged"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(
            Request::get(&uri)
                .header("authorization", format!("Bearer {}", owner.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn second_stream_keeps_seat_connected() {
    let (state, _) = test_state_with_clock();
    let app = router(state.clone());
    let (owner, _) = private(&app).await;
    let mut first = open(&app, &owner).await.into_body();
    next_event(&mut first).await;
    let mut second = open(&app, &owner).await.into_body();
    next_event(&mut second).await;

    drop(first);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        connection(&state, &owner.table, 0),
        "Connected",
        "one tab is still open"
    );

    drop(second);
    wait_for(&state, &owner.table, 0, "Disconnected").await;

    let mut again = open(&app, &owner).await.into_body();
    next_event(&mut again).await;
    assert_eq!(
        connection(&state, &owner.table, 0),
        "Connected",
        "opening a stream reconnects"
    );
}

#[tokio::test]
async fn seat_goes_to_bot_two_minutes_after_last_stream_closes() {
    let (state, clock) = test_state_with_clock();
    let app = router(state.clone());
    let (owner, code) = private(&app).await;
    let ben = join(&app, &code, "Ben").await;
    let mut stream = open(&app, &ben).await.into_body();
    next_event(&mut stream).await;
    drop(stream);
    wait_for(&state, &owner.table, 1, "Disconnected").await;

    clock.advance(time::Duration::seconds(119));
    state.registry.sweep(clock.now());
    assert!(connection(&state, &owner.table, 1).starts_with("Disconnected"));
    clock.advance(time::Duration::seconds(1));
    state.registry.sweep(clock.now());
    assert_eq!(connection(&state, &owner.table, 1), "BotTakeover");

    // The bot keeps the seat: the player's stream says so, then closes.
    let mut refused = open(&app, &ben).await.into_body();
    let (name, error) = next_event(&mut refused).await;
    assert_eq!(name, "error");
    assert_eq!(
        error["message"],
        "A bot has taken over this seat for the rest of the match."
    );
    let end = tokio::time::timeout(Duration::from_secs(5), refused.frame())
        .await
        .unwrap();
    assert!(end.is_none(), "the stream closes");
}

#[tokio::test]
async fn local_tables_have_no_presence() {
    let (state, _) = test_state_with_clock();
    let app = router(state.clone());
    let body = json!({"kind": "LOCAL", "variant": "CLASSIC", "difficulty": "RELAXED"});
    let (_, created) = call(&app, json_request("POST", "/api/tables", None, body)).await;
    let player = Player {
        table: created["tableId"].as_str().unwrap().into(),
        token: created["playerToken"].as_str().unwrap().into(),
    };
    let mut stream = open(&app, &player).await.into_body();
    let (_, snapshot) = next_event(&mut stream).await;
    assert_eq!(snapshot["kind"], "LOCAL");
    drop(stream);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(connection(&state, &player.table, 0), "Connected");
}

#[tokio::test]
async fn trick_is_collected_four_seconds_after_completion() {
    let (state, clock) = test_state_with_clock();
    let app = router(state.clone());
    let (owner, _) = private(&app).await;
    act(&app, &owner, json!({"type": "ready", "ready": true})).await;
    let mut view = act(&app, &owner, json!({"type": "start", "fillWithBots": true})).await;
    while view["bidding"]["complete"] != true {
        let action = if view["bidding"]["highestBid"] == 0 {
            json!({"type": "contract", "value": 80, "suit": "HEARTS"})
        } else {
            json!({"type": "pass"})
        };
        view = act(&app, &owner, action).await;
    }
    while view["board"]["reviewingCompletedTrick"] != true {
        let card = view["board"]["legalCards"][0].clone();
        view = act(
            &app,
            &owner,
            json!({"type": "card", "rank": card["rank"], "suit": card["suit"]}),
        )
        .await;
    }
    let mut stream = open(&app, &owner).await.into_body();
    let (_, shown) = next_event(&mut stream).await;
    assert_eq!(shown["board"]["reviewingCompletedTrick"], true);

    clock.advance(time::Duration::milliseconds(3999));
    state.registry.sweep(clock.now());
    clock.advance(time::Duration::milliseconds(1));
    state.registry.sweep(clock.now());
    let (_, collected) = next_event(&mut stream).await;
    assert_eq!(collected["board"]["reviewingCompletedTrick"], false);
    assert_eq!(collected["board"]["completedTricks"], 1);
}
