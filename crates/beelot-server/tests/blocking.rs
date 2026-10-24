//! A table busy with its bots must not stall the server: requests and event streams wait for its lock off the async
//! threads. Each test holds a table's lock from another thread, as a Challenging bot's search does, and checks that the
//! (single-threaded) runtime still runs a short timer on time.

mod common;

use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use beelot_game::Variant;
use beelot_server::app::{AppState, router};
use beelot_server::tables::Access;
use common::{get_request, json_request, test_state};
use serde_json::json;
use tower::ServiceExt;

const HELD: Duration = Duration::from_millis(600);

fn setup() -> (Router, AppState, Access) {
    let state = test_state();
    let access = state
        .registry
        .create_private("Ana", Variant::Classic, None)
        .unwrap();
    (router(state.clone()), state, access)
}

/// Holds the table's lock for a while from another thread; returns once the lock is taken.
fn hold_table(state: &AppState, access: &Access) -> std::thread::JoinHandle<()> {
    let handle = state.registry.handle(access.table_id).unwrap();
    let (taken, wait) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _table = handle.state.lock().unwrap();
        taken.send(()).unwrap();
        std::thread::sleep(HELD);
    });
    wait.recv().unwrap();
    thread
}

/// Runs the request while the table is held, and returns how late a 20 ms timer fired meanwhile.
async fn timer_delay_during(app: Router, request: Request<Body>) -> Duration {
    let call = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    let start = Instant::now();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let late = start.elapsed();
    call.await.unwrap();
    late
}

#[tokio::test]
async fn a_snapshot_waits_off_the_async_threads() {
    let (app, state, access) = setup();
    let holder = hold_table(&state, &access);
    let uri = format!("/api/tables/{}", access.table_id);
    let late = timer_delay_during(app, get_request(&uri, Some(&access.player_token))).await;
    holder.join().unwrap();
    assert!(
        late < Duration::from_millis(300),
        "the runtime was blocked for {late:?}"
    );
}

#[tokio::test]
async fn an_action_waits_off_the_async_threads() {
    let (app, state, access) = setup();
    let holder = hold_table(&state, &access);
    let uri = format!("/api/tables/{}/actions", access.table_id);
    let request = json_request(
        "POST",
        &uri,
        Some(&access.player_token),
        json!({"type": "ready", "ready": true}),
    );
    let late = timer_delay_during(app, request).await;
    holder.join().unwrap();
    assert!(
        late < Duration::from_millis(300),
        "the runtime was blocked for {late:?}"
    );
}

#[tokio::test]
async fn joining_waits_off_the_async_threads() {
    let (app, state, access) = setup();
    let holder = hold_table(&state, &access);
    let body = json!({"code": access.code.clone().unwrap(), "playerName": "Ben"});
    let late = timer_delay_during(app, json_request("POST", "/api/tables/join", None, body)).await;
    holder.join().unwrap();
    assert!(
        late < Duration::from_millis(300),
        "the runtime was blocked for {late:?}"
    );
}

#[tokio::test]
async fn opening_a_stream_waits_off_the_async_threads() {
    let (app, state, access) = setup();
    let holder = hold_table(&state, &access);
    let uri = format!(
        "/api/tables/{}/events?token={}",
        access.table_id, access.player_token
    );
    let request = Request::get(uri).body(Body::empty()).unwrap();
    let late = timer_delay_during(app, request).await;
    holder.join().unwrap();
    assert!(
        late < Duration::from_millis(300),
        "the runtime was blocked for {late:?}"
    );
}
