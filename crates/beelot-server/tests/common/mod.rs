#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use beelot_game::Difficulty;
use beelot_game::bot::strategies;
use beelot_server::app::AppState;
use beelot_server::clock::FakeClock;
use beelot_server::config::Config;
use beelot_server::tables::TableSetup;
use http_body_util::BodyExt;
use time::macros::datetime;
use tower::ServiceExt;

/// A state with the default configuration and a fake clock set to a fixed instant.
pub fn test_state() -> AppState {
    test_state_with_clock().0
}

pub fn test_state_with_clock() -> (AppState, Arc<FakeClock>) {
    state_with(Config::default())
}

/// A state with this configuration and a fake clock set to a fixed instant.
pub fn state_with(config: Config) -> (AppState, Arc<FakeClock>) {
    let clock = Arc::new(FakeClock::new(datetime!(2026-10-24 12:00 UTC)));
    let state = AppState::new(config, clock.clone()).with_table_setup(|| {
        TableSetup::seeded(11).with_strategy(strategies::for_difficulty(Difficulty::Relaxed))
    });
    (state, clock)
}

/// A JSON request, with a bearer token when there is one.
pub fn json_request(
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

/// A GET, with a bearer token when there is one.
pub fn get_request(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::get(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

/// Sends the request to the app and returns the status and the JSON body (null when empty).
pub async fn call(app: &axum::Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = body_bytes(response).await;
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into())
    };
    (status, body)
}

pub async fn body_bytes(response: Response<Body>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
        .to_vec()
}

pub async fn body_json(response: Response<Body>) -> serde_json::Value {
    serde_json::from_slice(&body_bytes(response).await).expect("JSON body")
}
