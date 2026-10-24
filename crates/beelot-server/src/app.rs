//! The application state and the router.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::clock::Clock;
use crate::config::Config;
use crate::error::AppError;

/// What every handler can reach. Cloning it is cheap.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub clock: Arc<dyn Clock>,
}

impl AppState {
    pub fn new(config: Config, clock: Arc<dyn Clock>) -> Self {
        Self {
            config: Arc::new(config),
            clock,
        }
    }
}

/// The paths of the single-page frontend, answered with `index.html` as Java's `HomeController` does.
const SPA_PATHS: [&str; 13] = [
    "/play/bot",
    "/play/local",
    "/play/bot/bidding/{id}",
    "/play/bot/game/{id}",
    "/online/private",
    "/online/private/table/{id}",
    "/online/public",
    "/online/public/table/{id}",
    "/tutorial",
    "/rules",
    "/settings",
    "/stats",
    "/privacy",
];

static REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

pub fn router(state: AppState) -> Router {
    let static_dir = &state.config.static_dir;
    let index = ServeFile::new(static_dir.join("index.html"));
    let mut router = Router::new().route("/api/{*rest}", axum::routing::any(not_found));
    for path in SPA_PATHS {
        router = router.route_service(path, index.clone());
    }
    let files = ServeDir::new(static_dir).not_found_service(axum::routing::any(not_found));
    let max_header_bytes = state.config.limits.max_header_bytes;

    router.fallback_service(files).with_state(state).layer(
        ServiceBuilder::new()
            .layer(SetResponseHeaderLayer::overriding(
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            ))
            .layer(SetRequestIdLayer::new(REQUEST_ID.clone(), MakeRequestUuid))
            .layer(TraceLayer::new_for_http().make_span_with(request_span))
            .layer(PropagateRequestIdLayer::new(REQUEST_ID.clone()))
            .layer(middleware::from_fn(move |request, next| {
                limit_headers(max_header_bytes, request, next)
            })),
    )
}

async fn not_found() -> AppError {
    AppError::NotFound
}

/// One span per request with its method, its path and its request id. The query string is left out: it may carry
/// a player token.
fn request_span(request: &Request<Body>) -> tracing::Span {
    let request_id = request
        .headers()
        .get(&REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    tracing::info_span!(
        "request",
        method = %request.method(),
        path = %request.uri().path(),
        request_id = %request_id,
    )
}

/// Refuses a request whose headers, names and values together, exceed the limit.
async fn limit_headers(max_bytes: usize, request: Request, next: Next) -> Response {
    let size: usize = request
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len())
        .sum();
    if size > max_bytes {
        return StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE.into_response();
    }
    next.run(request).await
}
