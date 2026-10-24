//! Signing up and signing in with a passkey: `/api/passkeys/…`. A refusal never says why; it records a failure.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use uuid::Uuid;
use webauthn_rs::prelude::{Passkey, PublicKeyCredential, RegisterPublicKeyCredential};

use crate::app::AppState;
use crate::auth::passkeys::{COOKIE, Ceremony};
use crate::auth::{csrf, pseudonyms, sessions};
use crate::db::{accounts, passkeys};
use crate::error::AppError;
use crate::limits::ClientIp;
use crate::privacy::events::{self, EventType, Outcome};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/passkeys/sign-up/options", post(sign_up_options))
        .route("/api/passkeys/sign-up", post(sign_up))
        .route("/api/passkeys/sign-in/options", post(sign_in_options))
        .route("/api/passkeys/sign-in", post(sign_in))
}

fn set_cookie(response: &mut Response, cookie: &str) {
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(cookie).expect("an ASCII cookie"),
    );
}

fn ceremony_cookie(id: &str) -> String {
    format!("{COOKIE}={id}; HttpOnly; SameSite=Strict; Path=/api/passkeys; Max-Age=300")
}

fn ceremony_taken(headers: &HeaderMap, state: &AppState) -> Option<Ceremony> {
    sessions::cookie_value(headers, COOKIE).and_then(|id| state.passkeys.take(id))
}

/// Starting a ceremony counts against the address's hourly limit; finishing one never does.
async fn limited(state: &AppState, ip: std::net::IpAddr) -> Option<Response> {
    if state.passkeys.limiter.try_acquire(&ip.to_string()) {
        return None;
    }
    events::record(
        &state.pool,
        EventType::RateLimited,
        Outcome::Refused,
        None,
        Some(ip),
        state.clock.now(),
    )
    .await;
    let mut refused = StatusCode::TOO_MANY_REQUESTS.into_response();
    refused
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("3600"));
    Some(refused)
}

/// The options to answer, with the cookie that names the ceremony.
fn options(id: &str, body: impl serde::Serialize) -> Response {
    let mut response = Json(body).into_response();
    set_cookie(&mut response, &ceremony_cookie(id));
    response
}

/// The form's `language`, else `Accept-Language`, names the pseudonym's language.
async fn sign_up_options(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    if let Some(refused) = limited(&state, ip).await {
        return Ok(refused);
    }
    let form_language = url::form_urlencoded::parse(&body)
        .find(|(key, _)| key == "language")
        .map(|(_, value)| value.into_owned());
    let accept = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok());
    let language = pseudonyms::language(form_language.as_deref(), accept);
    let (id, challenge) = state.passkeys.sign_up_options(language)?;
    Ok(options(&id, challenge))
}

async fn sign_in_options(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
) -> Result<Response, AppError> {
    if let Some(refused) = limited(&state, ip).await {
        return Ok(refused);
    }
    let (id, challenge) = state.passkeys.sign_in_options()?;
    Ok(options(&id, challenge))
}

/// Starts the session of a player who has just signed up or in: a new session and a new CSRF token.
async fn signed_in(
    state: &AppState,
    account: Uuid,
    ip: std::net::IpAddr,
) -> Result<Response, AppError> {
    let now = state.clock.now();
    let token = sessions::start(&state.pool, account, now).await?;
    events::record(
        &state.pool,
        EventType::SignIn,
        Outcome::Success,
        Some(account),
        Some(ip),
        now,
    )
    .await;
    let mut response = StatusCode::NO_CONTENT.into_response();
    set_cookie(
        &mut response,
        &sessions::cookie(&token, state.secure_cookies()),
    );
    set_cookie(&mut response, &csrf::new_cookie());
    set_cookie(
        &mut response,
        &format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/api/passkeys; Max-Age=0"),
    );
    Ok(response)
}

async fn refused(
    state: &AppState,
    r#type: EventType,
    status: StatusCode,
    ip: std::net::IpAddr,
) -> Response {
    events::record(
        &state.pool,
        r#type,
        Outcome::Failure,
        None,
        Some(ip),
        state.clock.now(),
    )
    .await;
    status.into_response()
}

/// Verifies the new passkey and stores it with its account, both or neither, then signs the browser in.
async fn sign_up(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let ceremony = ceremony_taken(&headers, &state);
    let credential = serde_json::from_slice::<RegisterPublicKeyCredential>(&body).ok();
    let made =
        credential.and_then(|credential| state.passkeys.finish_sign_up(ceremony, &credential));
    let Some((account, name, passkey)) = made else {
        return Ok(refused(&state, EventType::SignUp, StatusCode::BAD_REQUEST, ip).await);
    };
    let now = state.clock.now();
    let stored = serde_json::to_string(&passkey).map_err(anyhow::Error::from)?;
    let mut transaction = state.pool.begin().await?;
    accounts::insert(
        &mut *transaction,
        &accounts::Account::new(account, name, now),
    )
    .await?;
    if passkeys::insert(&mut *transaction, account, passkey.cred_id(), &stored, now)
        .await
        .is_err()
    {
        // A credential id another account already holds.
        transaction.rollback().await?;
        return Ok(refused(&state, EventType::SignUp, StatusCode::BAD_REQUEST, ip).await);
    }
    transaction.commit().await?;
    events::record(
        &state.pool,
        EventType::SignUp,
        Outcome::Success,
        Some(account),
        Some(ip),
        now,
    )
    .await;
    signed_in(&state, account, ip).await
}

/// Verifies the assertion with the account's passkey, stores its new counter, and signs the browser in.
async fn sign_in(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let ceremony = ceremony_taken(&headers, &state);
    let failed = || refused(&state, EventType::SignIn, StatusCode::UNAUTHORIZED, ip);
    let Ok(assertion) = serde_json::from_slice::<PublicKeyCredential>(&body) else {
        return Ok(failed().await);
    };
    let Some((account, credential_id)) = state.passkeys.identify(&assertion) else {
        return Ok(failed().await);
    };
    let Some(stored) = passkeys::find(&state.pool, account, &credential_id).await? else {
        return Ok(failed().await);
    };
    let Ok(mut passkey) = serde_json::from_str::<Passkey>(&stored) else {
        return Ok(failed().await);
    };
    let Some(result) = state
        .passkeys
        .finish_sign_in(ceremony, &assertion, &passkey)
    else {
        return Ok(failed().await);
    };
    let now = state.clock.now();
    passkey.update_credential(&result);
    let updated = serde_json::to_string(&passkey).map_err(anyhow::Error::from)?;
    passkeys::update(&state.pool, account, &updated, now).await?;
    accounts::record_sign_in(&state.pool, account, now).await?;
    signed_in(&state, account, ip).await
}
