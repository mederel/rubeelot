//! `GET /api/account` and `POST /api/sign-out`.

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use time::OffsetDateTime;

use crate::app::AppState;
use crate::auth::{MaybeSession, csrf, sessions};
use crate::db::accounts;
use crate::error::AppError;
use crate::limits::ClientIp;
use crate::privacy::events::{self, EventType, Outcome};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/account", get(account))
        .route("/api/sign-out", post(sign_out))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountView {
    signed_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    last_sign_in_at: Option<OffsetDateTime>,
}

const GUEST: AccountView = AccountView {
    signed_in: false,
    name: None,
    last_sign_in_at: None,
};

async fn account(
    State(state): State<AppState>,
    session: MaybeSession,
) -> Result<Json<AccountView>, AppError> {
    let Some(session) = session.0 else {
        return Ok(Json(GUEST));
    };
    let Some(account) = accounts::find(&state.pool, session.account_id).await? else {
        // The account is gone, deleted while this session was away: the session goes with it.
        sessions::end(&state.pool, &session.token_hash).await?;
        return Ok(Json(GUEST));
    };
    // The latest sign-in other than this session's own: one made since, elsewhere; otherwise the one before it.
    let elsewhere = if account.last_sign_in_at > session.signed_in_at {
        Some(account.last_sign_in_at)
    } else {
        account.previous_sign_in_at
    };
    Ok(Json(AccountView {
        signed_in: true,
        name: Some(account.display_name),
        last_sign_in_at: elsewhere,
    }))
}

/// Ends the session, records the sign-out, takes the cookie away and gives a new CSRF token.
async fn sign_out(
    State(state): State<AppState>,
    session: MaybeSession,
    ClientIp(ip): ClientIp,
) -> Result<Response, AppError> {
    if let Some(session) = session.0 {
        sessions::end(&state.pool, &session.token_hash).await?;
        events::record(
            &state.pool,
            EventType::SignOut,
            Outcome::Success,
            Some(session.account_id),
            Some(ip),
            state.clock.now(),
        )
        .await;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    let cookies = response.headers_mut();
    let secure = state.secure_cookies();
    cookies.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&sessions::expired_cookie(secure)).expect("an ASCII cookie"),
    );
    cookies.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&csrf::new_cookie()).expect("an ASCII cookie"),
    );
    Ok(response)
}
