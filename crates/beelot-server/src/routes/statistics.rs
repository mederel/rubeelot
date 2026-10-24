//! `GET /api/statistics`: the signed-in player's own statistics; guests are asked to sign in.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::MaybeSession;
use crate::db::matches;
use crate::error::AppError;
use crate::history::statistics::{self, PlayerStatistics};

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/statistics", get(statistics))
}

async fn statistics(
    State(state): State<AppState>,
    session: MaybeSession,
) -> Result<Json<PlayerStatistics>, AppError> {
    let account = session.account().ok_or(AppError::Unauthorized {
        code: "sign-in-required",
        message: "Sign in to see your statistics.",
    })?;
    let played = matches::played_by(&state.pool, account).await?;
    Ok(Json(statistics::of(account, &played)))
}
