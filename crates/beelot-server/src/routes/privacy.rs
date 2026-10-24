//! The privacy endpoints: the values the policy shows, the signed-in player's data as a download once an hour, and
//! deleting their account, which needs a sign-in less than 10 minutes old.

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;

use crate::app::AppState;
use crate::auth::{MaybeSession, csrf, sessions};
use crate::error::AppError;
use crate::limits::ClientIp;
use crate::privacy::export::{self, Export};
use crate::privacy::{deletion, policy_value};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/account/export", get(export_data))
        .route("/api/account", axum::routing::delete(delete_account))
        .route("/api/privacy/policy-info", get(policy_info))
}

fn sign_in_first() -> AppError {
    AppError::Unauthorized {
        code: "sign-in-required",
        message: "Sign in first.",
    }
}

async fn export_data(
    State(state): State<AppState>,
    session: MaybeSession,
    ClientIp(ip): ClientIp,
) -> Result<Response, AppError> {
    let account = session.account().ok_or_else(sign_in_first)?;
    let now = state.clock.now();
    match export::export(&state.pool, &state.events, account, Some(ip), now).await? {
        None => Err(sign_in_first()),
        Some(Export::TooSoon { minutes_left }) => {
            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"minutesLeft": minutes_left})),
            )
                .into_response();
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(minutes_left * 60));
            Ok(response)
        }
        Some(Export::Ready(export)) => {
            let mut response = Json(export).into_response();
            let filename = format!("attachment; filename=\"beelot-data-{}.json\"", now.date());
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&filename).expect("an ASCII file name"),
            );
            Ok(response)
        }
    }
}

async fn delete_account(
    State(state): State<AppState>,
    session: MaybeSession,
    ClientIp(ip): ClientIp,
) -> Result<Response, AppError> {
    let session = session.0.ok_or_else(sign_in_first)?;
    if !session.signed_in_recently(state.clock.now(), state.config.limits.recent_sign_in) {
        return Err(AppError::Forbidden {
            reason: Some("reauthenticate"),
        });
    }
    deletion::delete(&state, session.account_id, Some(ip)).await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    for cookie in [
        sessions::expired_cookie(state.secure_cookies()),
        csrf::new_cookie(),
    ] {
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).expect("an ASCII cookie"),
        );
    }
    Ok(response)
}

/// The policy shows "not configured" for each value left out.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyInfo<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    controller: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contact: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hosting_provider: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup_retention_days: Option<&'a str>,
}

async fn policy_info(State(state): State<AppState>) -> Response {
    let privacy = &state.config.privacy;
    Json(PolicyInfo {
        controller: policy_value(&privacy.controller),
        contact: policy_value(&privacy.contact),
        hosting_provider: policy_value(&privacy.hosting_provider),
        backup_retention_days: policy_value(&privacy.backup_retention_days),
    })
    .into_response()
}
