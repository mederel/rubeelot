//! The errors a request can end with, and the JSON body each one answers: `{"error": code, "message": text}`.
//!
//! Messages are Java's English text: the frontend translates them with the English as the key.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    /// A body or a parameter that cannot be read.
    #[error("bad request")]
    BadRequest,
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    /// A bad player token, a failed passkey, or a guest where an account is needed.
    #[error("{message}")]
    Unauthorized {
        code: &'static str,
        message: &'static str,
    },
    /// A missing CSRF token (`reason` is `None`), or a refusal the frontend acts on through its reason.
    #[error("forbidden")]
    Forbidden { reason: Option<&'static str> },
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("rate limited")]
    RateLimited { retry_after_secs: u64 },
    #[error("capacity exceeded")]
    CapacityExceeded,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub const BUSY_MESSAGE: &str = "The server is busy. Please try again later.";
pub const RATE_LIMITED_MESSAGE: &str = "Too many requests. Please slow down.";
pub const TABLE_UNAUTHORIZED_MESSAGE: &str = "You are not authorized for this table.";
pub const REAUTHENTICATE_MESSAGE: &str = "Sign in again to delete your account.";

impl AppError {
    /// A conflict carrying Java's message.
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        AppError::Conflict {
            code,
            message: message.into(),
        }
    }

    /// A missing or bad player token.
    pub fn table_unauthorized() -> Self {
        AppError::Unauthorized {
            code: "unauthorized",
            message: TABLE_UNAUTHORIZED_MESSAGE,
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::BadRequest => StatusCode::BAD_REQUEST,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            AppError::Forbidden { .. } => StatusCode::FORBIDDEN,
            AppError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::CapacityExceeded => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

fn body(code: &str, message: &str) -> serde_json::Value {
    json!({ "error": code, "message": message })
}

fn retry_after(mut response: Response, seconds: u64) -> Response {
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
    response
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        match self {
            AppError::NotFound => (status, Json(body("not-found", "Not found."))).into_response(),
            AppError::BadRequest => (
                status,
                Json(body("bad-request", "The request is not valid.")),
            )
                .into_response(),
            AppError::Conflict { code, message } => {
                (status, Json(body(code, &message))).into_response()
            }
            AppError::Unauthorized { code, message } => {
                (status, Json(body(code, message))).into_response()
            }
            AppError::Forbidden { reason: None } => {
                (status, Json(body("csrf", "Forbidden"))).into_response()
            }
            AppError::Forbidden {
                reason: Some(reason),
            } => {
                let message = match reason {
                    "reauthenticate" => REAUTHENTICATE_MESSAGE,
                    _ => "Forbidden",
                };
                let mut value = body(reason, message);
                value["reason"] = reason.into();
                (status, Json(value)).into_response()
            }
            AppError::PayloadTooLarge => (
                status,
                Json(body("payload-too-large", "The request is too large.")),
            )
                .into_response(),
            AppError::RateLimited { retry_after_secs } => retry_after(
                (status, Json(body("rate-limited", RATE_LIMITED_MESSAGE))).into_response(),
                retry_after_secs,
            ),
            AppError::CapacityExceeded => retry_after(
                (status, Json(body("capacity-exceeded", BUSY_MESSAGE))).into_response(),
                60,
            ),
            AppError::Internal(error) => {
                tracing::error!(error = %format!("{error:#}"), "request failed");
                (
                    status,
                    Json(body("internal", "Something went wrong. Please try again.")),
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    async fn parts(error: AppError) -> (StatusCode, Option<String>, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let retry = response
            .headers()
            .get(header::RETRY_AFTER)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, retry, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn each_error_has_its_status_and_body() {
        let (status, _, body) = parts(AppError::conflict("not-your-turn", "Wait.")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body, json!({"error": "not-your-turn", "message": "Wait."}));

        let (status, _, body) = parts(AppError::table_unauthorized()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], TABLE_UNAUTHORIZED_MESSAGE);

        let (status, _, body) = parts(AppError::Forbidden { reason: None }).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body, json!({"error": "csrf", "message": "Forbidden"}));

        let (_, _, body) = parts(AppError::Forbidden {
            reason: Some("reauthenticate"),
        })
        .await;
        assert_eq!(
            body,
            json!({"error": "reauthenticate", "message": REAUTHENTICATE_MESSAGE, "reason": "reauthenticate"})
        );

        let (status, retry, body) = parts(AppError::RateLimited {
            retry_after_secs: 12,
        })
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(retry.as_deref(), Some("12"));
        assert_eq!(body["message"], RATE_LIMITED_MESSAGE);

        let (status, retry, body) = parts(AppError::CapacityExceeded).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(retry.as_deref(), Some("60"));
        assert_eq!(body["message"], BUSY_MESSAGE);
    }

    #[tokio::test]
    async fn internal_errors_have_an_opaque_body() {
        let (status, _, body) =
            parts(AppError::Internal(anyhow::anyhow!("db password=hunter2"))).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!body.to_string().contains("hunter2"));
        assert_eq!(body["error"], "internal");
    }
}
