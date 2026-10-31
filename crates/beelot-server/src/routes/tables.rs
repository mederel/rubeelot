//! Creating, joining and playing at tables: `/api/tables` and `/api/matchmaking/quick-match`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use beelot_game::match_score::DEFAULT_TARGET;
use beelot_game::{Difficulty, Variant};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiJson, bearer, blocking, table_id};
use crate::api::{Snapshot, snapshot};
use crate::app::AppState;
use crate::auth::MaybeSession;
use crate::error::AppError;
use crate::tables::registry::{LocalOptions, viewer_of};
use crate::tables::{Access, Action, Hint};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/tables", post(create))
        .route("/api/tables/join", post(join))
        .route("/api/matchmaking/quick-match", post(quick_match))
        .route("/api/tables/{id}", get(table))
        .route("/api/tables/{id}/actions", post(act))
        .route("/api/tables/{id}/hint", get(hint))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Kind {
    Local,
    Private,
}

/// `seats`, for pass-and-play only: the four seat names from North, empty or null for a bot.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRequest {
    kind: Kind,
    variant: Option<Variant>,
    difficulty: Option<Difficulty>,
    announcements: Option<bool>,
    /// Whether Contrée bids may name tout-atout and sans-atout; on when absent.
    all_trumps: Option<bool>,
    target: Option<i32>,
    seats: Option<Vec<Option<String>>>,
    #[serde(default)]
    player_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinRequest {
    #[serde(default)]
    code: String,
    #[serde(default)]
    player_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuickMatchRequest {
    variant: Option<Variant>,
    #[serde(default)]
    player_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccessResponse {
    table_id: Uuid,
    player_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

fn created(access: Access) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(AccessResponse {
            table_id: access.table_id,
            player_token: access.player_token,
            code: access.code,
        }),
    )
}

async fn create(
    State(state): State<AppState>,
    session: MaybeSession,
    ApiJson(request): ApiJson<CreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let variant = request.variant.unwrap_or(Variant::Classic);
    let account = session.account();
    let registry = state.registry.clone();
    // Making room may evict idle tables, which locks them.
    let access = blocking(move || match request.kind {
        Kind::Local => registry.create_local(
            variant,
            request.difficulty.unwrap_or(Difficulty::Relaxed),
            LocalOptions {
                announcements: request.announcements.unwrap_or(true),
                all_trumps: request.all_trumps.unwrap_or(true),
                target: request.target.unwrap_or(DEFAULT_TARGET),
            },
            request.seats,
            account,
        ),
        Kind::Private => registry.create_private(&request.player_name, variant, account),
    })
    .await??;
    Ok(created(access))
}

async fn join(
    State(state): State<AppState>,
    session: MaybeSession,
    ApiJson(request): ApiJson<JoinRequest>,
) -> Result<impl IntoResponse, AppError> {
    let account = session.account();
    let registry = state.registry.clone();
    let access =
        blocking(move || registry.join(&request.code, &request.player_name, account)).await??;
    Ok(created(access))
}

async fn quick_match(
    State(state): State<AppState>,
    session: MaybeSession,
    ApiJson(request): ApiJson<QuickMatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let account = session.account();
    let variant = request.variant.unwrap_or(Variant::Classic);
    let registry = state.registry.clone();
    let name = request.player_name;
    // Quick matches wait for each other, and a fourth player's join may start a match with bots.
    let access = blocking(move || registry.quick_match(&name, variant, account)).await??;
    Ok(created(access))
}

async fn table(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Snapshot>, AppError> {
    let token = bearer(&headers)
        .ok_or_else(AppError::table_unauthorized)?
        .to_owned();
    let id = table_id(&id)?;
    let (registry, clock) = (state.registry.clone(), state.clock.clone());
    let view = blocking(move || {
        let (handle, actor) = registry.authorize(id, &token)?;
        let table = handle.state.lock().unwrap();
        Ok::<_, AppError>(snapshot(&table, viewer_of(actor), clock.now()))
    })
    .await??;
    Ok(Json(view))
}

/// Applies one action and answers the table as the player now sees it, off the async threads: the bots may search
/// before answering.
async fn act(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    ApiJson(action): ApiJson<Action>,
) -> Result<Json<Snapshot>, AppError> {
    let token = bearer(&headers)
        .ok_or_else(AppError::table_unauthorized)?
        .to_owned();
    let id = table_id(&id)?;
    let registry = state.registry.clone();
    let (outcome, view) = blocking(move || {
        let (handle, actor) = registry.authorize(id, &token)?;
        registry.act(&handle, actor, action)
    })
    .await??;
    state.hand_over([outcome]);
    Ok(Json(view))
}

/// What the Challenging bot would do in the player's place (US-065). Searching takes a while, off the async threads.
async fn hint(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Hint>, AppError> {
    let token = bearer(&headers)
        .ok_or_else(AppError::table_unauthorized)?
        .to_owned();
    let id = table_id(&id)?;
    let registry = state.registry.clone();
    let hint = blocking(move || {
        let (handle, actor) = registry.authorize(id, &token)?;
        let table = handle.state.lock().unwrap();
        table.hint(actor)
    })
    .await??;
    Ok(Json(hint))
}
