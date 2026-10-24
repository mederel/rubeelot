//! Accounts' sessions and the CSRF rule. Passkeys come with sign-up and sign-in.

pub mod csrf;
pub mod sessions;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::app::AppState;
pub use sessions::CurrentSession;

/// The request's session, or none for a guest. A session past its limits is ended, and the request goes on as a
/// guest's.
#[derive(Debug, Clone)]
pub struct MaybeSession(pub Option<CurrentSession>);

impl MaybeSession {
    pub fn account(&self) -> Option<uuid::Uuid> {
        self.0.as_ref().map(|session| session.account_id)
    }
}

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = crate::error::AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(found) = parts.extensions.get::<MaybeSession>() {
            return Ok(found.clone());
        }
        let session = match sessions::token(&parts.headers) {
            Some(token) => sessions::lookup(&state.pool, token, state.clock.now()).await?,
            None => None,
        };
        let found = MaybeSession(session);
        parts.extensions.insert(found.clone());
        Ok(found)
    }
}
