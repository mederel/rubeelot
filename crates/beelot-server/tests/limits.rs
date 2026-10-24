//! Ports `RequestLimitFiltersTest`: the per-client request and creation limits, and the body size limit.

mod common;

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use beelot_server::app::router;
use beelot_server::config::{Config, Limits};
use common::{body_json, state_with, test_state};
use time::Duration;
use tower::ServiceExt;

fn limited(
    requests: u32,
    creations: u32,
) -> (Router, std::sync::Arc<beelot_server::clock::FakeClock>) {
    let config = Config {
        limits: Limits {
            requests_per_window: requests,
            creations_per_window: creations,
            ..Limits::default()
        },
        ..Config::default()
    };
    let (state, clock) = state_with(config);
    (router(state), clock)
}

fn request(method: &str, uri: &str, ip: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let peer: SocketAddr = format!("{ip}:40000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    request
}

async fn send(app: &Router, method: &str, uri: &str, ip: &str) -> axum::http::Response<Body> {
    app.clone().oneshot(request(method, uri, ip)).await.unwrap()
}

async fn status(app: &Router, method: &str, uri: &str, ip: &str) -> StatusCode {
    send(app, method, uri, ip).await.status()
}

#[tokio::test]
async fn rejects_requests_over_the_per_client_limit_and_recovers_after_the_window() {
    let (app, clock) = limited(3, 1);
    for _ in 0..3 {
        assert_ne!(
            status(&app, "GET", "/api/tables/x", "1.1.1.1").await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    let refused = send(&app, "GET", "/api/tables/x", "1.1.1.1").await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(refused.headers().contains_key("retry-after"));
    assert_eq!(
        body_json(refused).await,
        serde_json::json!({"error": "rate-limited", "message": "Too many requests. Please slow down."})
    );
    assert_ne!(
        status(&app, "GET", "/api/tables/x", "2.2.2.2").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    clock.advance(Duration::seconds(61));
    assert_ne!(
        status(&app, "GET", "/api/tables/x", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn applies_a_stricter_limit_to_creating_tables() {
    let (app, _) = limited(100, 2);
    assert_ne!(
        status(&app, "POST", "/api/tables", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_ne!(
        status(&app, "POST", "/api/tables/join", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        status(&app, "POST", "/api/tables", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_ne!(
        status(&app, "GET", "/api/tables/x", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_ne!(
        status(&app, "POST", "/api/tables/x/actions", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn counts_quick_match_as_creating_a_table() {
    let (app, _) = limited(100, 1);
    let path = "/api/matchmaking/quick-match";
    assert_ne!(
        status(&app, "POST", path, "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        status(&app, "POST", path, "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn ignores_static_resources() {
    let (app, _) = limited(1, 1);
    for _ in 0..5 {
        assert_eq!(
            status(&app, "GET", "/app.js", "1.1.1.1").await,
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn creation_limit_is_separate() {
    let app = router(test_state());
    for _ in 0..20 {
        assert_ne!(
            status(&app, "POST", "/api/tables", "1.1.1.1").await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    assert_eq!(
        status(&app, "POST", "/api/tables", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_ne!(
        status(&app, "GET", "/api/tables/x", "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn tracks_a_bounded_number_of_clients_and_frees_expired_ones() {
    let (app, clock) = limited(300, 20);
    for client in 0u32..50_000 {
        let ip = format!(
            "10.{}.{}.{}",
            client >> 16,
            (client >> 8) & 255,
            client & 255
        );
        send(&app, "GET", "/api/statistics", &ip).await;
    }
    let newcomer = send(&app, "GET", "/api/statistics", "192.168.0.1").await;
    assert_eq!(newcomer.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(newcomer.headers()["retry-after"], "61");
    assert_ne!(
        status(&app, "GET", "/api/statistics", "10.0.0.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );

    clock.advance(Duration::seconds(60));
    assert_ne!(
        status(&app, "GET", "/api/statistics", "192.168.0.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn forwarded_header_ignored_unless_trusted() {
    let forwarded = |app: Router, client: &'static str| async move {
        let mut request = request("GET", "/api/tables/x", "9.9.9.9");
        request
            .headers_mut()
            .insert("x-forwarded-for", client.parse().unwrap());
        app.oneshot(request).await.unwrap().status()
    };

    let (app, _) = limited(1, 1);
    assert_ne!(
        forwarded(app.clone(), "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        forwarded(app.clone(), "2.2.2.2").await,
        StatusCode::TOO_MANY_REQUESTS
    );

    let config = Config {
        trust_forwarded_headers: true,
        limits: Limits {
            requests_per_window: 1,
            ..Limits::default()
        },
        ..Config::default()
    };
    let app = router(state_with(config).0);
    assert_ne!(
        forwarded(app.clone(), "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_ne!(
        forwarded(app.clone(), "2.2.2.2").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        forwarded(app.clone(), "1.1.1.1").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn rejects_oversized_bodies_by_declared_length() {
    let app = router(test_state());
    let mut request = request("POST", "/api/tables", "1.1.1.1");
    *request.body_mut() = Body::from(vec![b' '; 5000]);
    request
        .headers_mut()
        .insert("content-length", "5000".parse().unwrap());
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
