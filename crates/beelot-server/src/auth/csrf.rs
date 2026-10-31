//! The CSRF rule: the client reads the token from the `XSRF-TOKEN`
//! cookie and sends it back in the `X-XSRF-TOKEN` header. A request that changes state needs it once a session
//! exists, and always on the passkey ceremonies, which start one. Guests have no session to abuse.

use axum::extract::Request;
use axum::http::{HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use super::sessions::{self, cookie_value};
use crate::error::AppError;

pub const COOKIE: &str = "XSRF-TOKEN";
pub const HEADER: &str = "x-xsrf-token";

const CEREMONIES: [&str; 4] = [
    "/api/passkeys/sign-up/options",
    "/api/passkeys/sign-up",
    "/api/passkeys/sign-in/options",
    "/api/passkeys/sign-in",
];

/// A new token's `Set-Cookie` value: readable by the client's script.
pub fn new_cookie() -> String {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).expect("the operating system's random generator");
    format!(
        "{COOKIE}={}; SameSite=Lax; Path=/",
        URL_SAFE_NO_PAD.encode(bytes)
    )
}

fn equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

pub async fn protect(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let token = cookie_value(headers, COOKIE).filter(|token| !token.is_empty());
    let unsafe_method = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    );
    let needed = unsafe_method
        && (sessions::token(headers).is_some()
            || (request.method() == Method::POST && CEREMONIES.contains(&request.uri().path())));
    if needed {
        let sent = headers.get(HEADER).map(HeaderValue::as_bytes);
        let matches =
            matches!((token, sent), (Some(token), Some(sent)) if equal(token.as_bytes(), sent));
        if !matches {
            return AppError::Forbidden { reason: None }.into_response();
        }
    }
    let had_token = token.is_some();
    let mut response = next.run(request).await;
    let rotated = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|value| {
            value
                .as_bytes()
                .starts_with(format!("{COOKIE}=").as_bytes())
        });
    if !had_token && !rotated {
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&new_cookie()).expect("an ASCII cookie"),
        );
    }
    response
}
