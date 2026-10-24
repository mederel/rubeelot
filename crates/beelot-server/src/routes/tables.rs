//! Creating, joining and playing at tables: `/api/tables` and `/api/matchmaking/quick-match`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use beelot_game::{Difficulty, Variant};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiJson, bearer, table_id};
use crate::api::{Snapshot, snapshot};
use crate::app::AppState;
use crate::auth::MaybeSession;
use crate::error::AppError;
use crate::tables::registry::viewer_of;
use crate::tables::{Access, Action};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/tables", post(create))
        .route("/api/tables/join", post(join))
        .route("/api/matchmaking/quick-match", post(quick_match))
        .route("/api/tables/{id}", get(table))
        .route("/api/tables/{id}/actions", post(act))
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
    let access = match request.kind {
        Kind::Local => state.registry.create_local(
            variant,
            request.difficulty.unwrap_or(Difficulty::Relaxed),
            request.announcements.unwrap_or(true),
            request.seats,
            account,
        )?,
        Kind::Private => state
            .registry
            .create_private(&request.player_name, variant, account)?,
    };
    Ok(created(access))
}

async fn join(
    State(state): State<AppState>,
    session: MaybeSession,
    ApiJson(request): ApiJson<JoinRequest>,
) -> Result<impl IntoResponse, AppError> {
    let account = session.account();
    let access = state
        .registry
        .join(&request.code, &request.player_name, account)?;
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
    let access = tokio::task::spawn_blocking(move || registry.quick_match(&name, variant, account))
        .await
        .map_err(|error| AppError::Internal(error.into()))??;
    Ok(created(access))
}

async fn table(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Snapshot>, AppError> {
    let token = bearer(&headers).ok_or_else(AppError::table_unauthorized)?;
    let (handle, actor) = state.registry.authorize(table_id(&id)?, token)?;
    let table = handle.state.lock().unwrap();
    Ok(Json(snapshot(&table, viewer_of(actor), state.clock.now())))
}

/// Applies one action and answers the table as the player now sees it. A Challenging bot's search runs off the
/// async threads.
async fn act(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    ApiJson(action): ApiJson<Action>,
) -> Result<Json<Snapshot>, AppError> {
    let token = bearer(&headers).ok_or_else(AppError::table_unauthorized)?;
    let (handle, actor) = state.registry.authorize(table_id(&id)?, token)?;
    let blocking = handle.state.lock().unwrap().needs_blocking_bot();
    let registry = state.registry.clone();
    let (outcome, view) = if blocking {
        tokio::task::spawn_blocking(move || registry.act(&handle, actor, action))
            .await
            .map_err(|error| AppError::Internal(error.into()))??
    } else {
        registry.act(&handle, actor, action)?
    };
    state.hand_over([outcome]);
    Ok(Json(view))
}
