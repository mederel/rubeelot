//! The application state and the router.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use sqlx::MySqlPool;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::auth::csrf;
use crate::auth::passkeys::Passkeys;
use crate::clock::Clock;
use crate::config::Config;
use crate::error::AppError;
use crate::limits::{self, RateLimits};
use crate::routes;
use crate::tables::TableRegistry;
use crate::tables::TableSetup;
use crate::tables::finished::{MatchSink, NoopSink};

/// What every handler can reach. Cloning it is cheap.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub clock: Arc<dyn Clock>,
    pub rate_limits: Arc<RateLimits>,
    pub registry: Arc<TableRegistry>,
    pub sink: Arc<dyn MatchSink>,
    /// Lazy until a request needs it: guests' requests never do.
    pub pool: MySqlPool,
    pub passkeys: Arc<Passkeys>,
}

impl AppState {
    pub fn new(config: Config, clock: Arc<dyn Clock>) -> Self {
        let pool = MySqlPoolOptions::new().connect_lazy_with(
            config
                .database_url
                .parse::<MySqlConnectOptions>()
                .unwrap_or_default()
                .username(&config.database_username)
                .password(&config.database_password),
        );
        let passkeys = Passkeys::new(&config.public_url, &config.limits, clock.clone())
            .expect("a public URL checked by the configuration");
        Self {
            pool,
            passkeys: Arc::new(passkeys),
            rate_limits: Arc::new(RateLimits::new(&config, clock.clone())),
            registry: Arc::new(TableRegistry::new(&config.limits, clock.clone())),
            config: Arc::new(config),
            clock,
            sink: Arc::new(NoopSink),
        }
    }

    /// Tables are set up by `setup`: tests deal from seeded generators. Call it before any table exists.
    pub fn with_table_setup(
        mut self,
        setup: impl Fn() -> TableSetup + Send + Sync + 'static,
    ) -> Self {
        self.registry =
            Arc::new(TableRegistry::new(&self.config.limits, self.clock.clone()).with_setup(setup));
        self
    }

    pub fn with_pool(mut self, pool: MySqlPool) -> Self {
        self.pool = pool;
        self
    }

    /// Whether cookies are only sent over HTTPS: when players reach Beelot through it.
    pub fn secure_cookies(&self) -> bool {
        self.config.public_url.starts_with("https://")
    }

    /// Finished matches go to `sink`.
    pub fn with_sink(mut self, sink: Arc<dyn MatchSink>) -> Self {
        self.sink = sink;
        self
    }

    /// Hands a finished match to the sink, in the background.
    pub fn hand_over(&self, outcomes: impl IntoIterator<Item = crate::tables::Outcome>) {
        for finished in outcomes.into_iter().filter_map(|outcome| outcome.finished) {
            let sink = self.sink.clone();
            tokio::spawn(async move { sink.record(finished).await });
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
    let mut router = routes::api().route("/api/{*rest}", axum::routing::any(not_found));
    for path in SPA_PATHS {
        router = router.route_service(path, index.clone());
    }
    let files = ServeDir::new(static_dir).not_found_service(axum::routing::any(not_found));
    let max_header_bytes = state.config.limits.max_header_bytes;
    let max_request_bytes = state.config.limits.max_request_bytes;
    let rate_limits = state.rate_limits.clone();

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
            }))
            .layer(middleware::from_fn_with_state(
                rate_limits,
                limits::rate::rate_limit,
            ))
            .layer(middleware::from_fn(csrf::protect))
            .layer(limits::size::layer(max_request_bytes)),
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
