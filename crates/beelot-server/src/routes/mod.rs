//! The HTTP API, one module per area.

pub mod account;
pub mod events;
pub mod passkeys;
pub mod privacy;
pub mod statistics;
pub mod tables;

use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::http::HeaderMap;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;

pub fn api() -> Router<AppState> {
    tables::routes()
        .merge(events::routes())
        .merge(account::routes())
        .merge(passkeys::routes())
        .merge(statistics::routes())
        .merge(privacy::routes())
}

/// A JSON body; one that cannot be read is a bad request with the usual error body.
pub struct ApiJson<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for ApiJson<T> {
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(ApiJson(value)),
            Err(JsonRejection::BytesRejection(_)) => Err(AppError::PayloadTooLarge),
            Err(_) => Err(AppError::BadRequest),
        }
    }
}

/// A table id from the path; anything else names no table.
pub fn table_id(id: &str) -> Result<Uuid, AppError> {
    id.parse().map_err(|_| AppError::NotFound)
}

/// The token of `Authorization: Bearer <token>`.
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
}
