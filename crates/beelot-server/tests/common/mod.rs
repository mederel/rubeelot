#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::Response;
use beelot_server::app::AppState;
use beelot_server::clock::FakeClock;
use beelot_server::config::Config;
use http_body_util::BodyExt;
use time::macros::datetime;

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
    (AppState::new(config, clock.clone()), clock)
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
