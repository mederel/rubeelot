//! Signing up and signing in with a passkey, through the router, on MariaDB. Ports `PasskeySignUpIntegrationTest`,
//! `PasskeySignInIntegrationTest`, `SignInSuccessHandlerTest`, `AccountServiceTest` and `PasskeyRequestFilterTest`.
//!
//! The software passkey of webauthn-authenticator-rs keeps no resident keys and returns no user handle, so the test
//! plays the browser's part around it: it asks it for a non-resident key, names the credential to sign with, and adds
//! the user handle a discoverable passkey returns. The server's checks are unchanged.

mod common;
#[path = "common/db.rs"]
mod db;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use beelot_server::app::router;
use beelot_server::clock::{Clock, FakeClock};
use beelot_server::config::{Config, Limits};
use common::{body_bytes, state_with};
use serde_json::Value;
use sqlx::MySqlPool;
use time::Duration;
use tower::ServiceExt;
use url::Url;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_rs::prelude::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};
use webauthn_rs_proto::{AllowCredentials, AuthenticatorTransport};

const CSRF: &str = "5f0c9a2e-test-csrf-token";
const ORIGIN: &str = "http://localhost:8080";

struct World {
    app: Router,
    clock: Arc<FakeClock>,
    pool: MySqlPool,
}

/// A browser: its cookies, and its passkey once made.
struct Browser {
    ceremony: Option<String>,
    session: Option<String>,
    authenticator: WebauthnAuthenticator<SoftPasskey>,
    credential: Option<(Vec<u8>, Vec<u8>)>,
}

impl Browser {
    fn new() -> Self {
        Self {
            ceremony: None,
            session: None,
            authenticator: WebauthnAuthenticator::new(SoftPasskey::new(true)),
            credential: None,
        }
    }

    fn cookies(&self) -> String {
        let mut cookies = vec![format!("XSRF-TOKEN={CSRF}")];
        if let Some(ceremony) = &self.ceremony {
            cookies.push(format!("BEELOT_CEREMONY={ceremony}"));
        }
        if let Some(session) = &self.session {
            cookies.push(format!("BEELOT_SESSION={session}"));
        }
        cookies.join("; ")
    }
}

async fn world_with(limits: Limits) -> World {
    let pool = db::test_pool().await;
    let (state, clock) = state_with(Config {
        limits,
        ..Config::default()
    });
    World {
        app: router(state.with_pool(pool.clone())),
        clock,
        pool,
    }
}

async fn world() -> World {
    world_with(Limits::default()).await
}

fn set_cookie(response: &axum::http::Response<Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| cookie.strip_prefix(&format!("{name}=")).map(str::to_owned))
}

fn cookie_value(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_owned()
}

impl World {
    async fn post(
        &self,
        browser: &mut Browser,
        path: &str,
        content_type: &str,
        body: String,
    ) -> (StatusCode, Vec<u8>, axum::http::HeaderMap) {
        let request = Request::post(path)
            .header(header::COOKIE, browser.cookies())
            .header("X-XSRF-TOKEN", CSRF)
            .header(header::CONTENT_TYPE, content_type)
            .header("x-forwarded-for", "203.0.113.7")
            .body(Body::from(body))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        if let Some(ceremony) = set_cookie(&response, "BEELOT_CEREMONY") {
            browser.ceremony = Some(cookie_value(&ceremony)).filter(|value| !value.is_empty());
        }
        if let Some(session) = set_cookie(&response, "BEELOT_SESSION") {
            browser.session = Some(cookie_value(&session)).filter(|value| !value.is_empty());
        }
        let status = response.status();
        let headers = response.headers().clone();
        (status, body_bytes(response).await, headers)
    }

    async fn sign_up_options(&self, browser: &mut Browser, language: &str) -> (StatusCode, Value) {
        let (status, body, _) = self
            .post(
                browser,
                "/api/passkeys/sign-up/options",
                "application/x-www-form-urlencoded",
                format!("language={language}"),
            )
            .await;
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    /// The passkey's answer to the options, made as the browser makes it.
    fn register(
        browser: &mut Browser,
        options: &Value,
        origin: &str,
    ) -> RegisterPublicKeyCredential {
        let mut challenge: CreationChallengeResponse =
            serde_json::from_value(options.clone()).unwrap();
        // The software passkey keeps no resident keys.
        if let Some(selection) = challenge.public_key.authenticator_selection.as_mut() {
            selection.require_resident_key = false;
            selection.resident_key = None;
        }
        let user = challenge.public_key.user.id.to_vec();
        let credential = browser
            .authenticator
            .do_registration(Url::parse(origin).unwrap(), challenge)
            .unwrap();
        browser.credential = Some((credential.raw_id.to_vec(), user));
        credential
    }

    async fn finish_sign_up(
        &self,
        browser: &mut Browser,
        credential: &RegisterPublicKeyCredential,
    ) -> StatusCode {
        self.post(
            browser,
            "/api/passkeys/sign-up",
            "application/json",
            serde_json::to_string(credential).unwrap(),
        )
        .await
        .0
    }

    /// Signs up a new browser; returns it and its account id.
    async fn signed_up(&self) -> (Browser, uuid::Uuid) {
        let mut browser = Browser::new();
        let (status, options) = self.sign_up_options(&mut browser, "en").await;
        assert_eq!(status, StatusCode::OK);
        let credential = Self::register(&mut browser, &options, ORIGIN);
        assert_eq!(
            self.finish_sign_up(&mut browser, &credential).await,
            StatusCode::NO_CONTENT
        );
        let user = browser.credential.as_ref().unwrap().1.clone();
        (browser, uuid::Uuid::from_slice(&user).unwrap())
    }

    async fn sign_in_options(&self, browser: &mut Browser) -> (StatusCode, Value) {
        let (status, body, _) = self
            .post(
                browser,
                "/api/passkeys/sign-in/options",
                "application/json",
                String::new(),
            )
            .await;
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    /// The passkey's assertion for the options, with the user handle a discoverable passkey returns.
    fn assert(browser: &mut Browser, options: &Value) -> PublicKeyCredential {
        let mut challenge: RequestChallengeResponse =
            serde_json::from_value(options.clone()).unwrap();
        let (credential_id, user) = browser.credential.clone().unwrap();
        challenge.public_key.allow_credentials = vec![AllowCredentials {
            type_: "public-key".into(),
            id: credential_id.into(),
            transports: Some(vec![AuthenticatorTransport::Usb]),
        }];
        let mut assertion = browser
            .authenticator
            .do_authentication(Url::parse(ORIGIN).unwrap(), challenge)
            .unwrap();
        assertion.response.user_handle = Some(user.into());
        assertion
    }

    async fn finish_sign_in(
        &self,
        browser: &mut Browser,
        assertion: &PublicKeyCredential,
    ) -> (StatusCode, Vec<u8>) {
        let (status, body, _) = self
            .post(
                browser,
                "/api/passkeys/sign-in",
                "application/json",
                serde_json::to_string(assertion).unwrap(),
            )
            .await;
        (status, body)
    }

    async fn sign_in(&self, browser: &mut Browser) -> (StatusCode, Vec<u8>) {
        let (status, options) = self.sign_in_options(browser).await;
        assert_eq!(status, StatusCode::OK);
        let assertion = Self::assert(browser, &options);
        self.finish_sign_in(browser, &assertion).await
    }

    async fn count(&self, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn events(&self, r#type: &str, outcome: &str, account: Option<uuid::Uuid>) -> i64 {
        let (sql, account) = match account {
            Some(account) => (
                "SELECT COUNT(*) FROM security_event WHERE type = ? AND outcome = ? AND account_id = ?",
                Some(account.hyphenated().to_string()),
            ),
            None => (
                "SELECT COUNT(*) FROM security_event WHERE type = ? AND outcome = ? AND account_id IS NULL",
                None,
            ),
        };
        let mut query = sqlx::query_scalar(sql).bind(r#type).bind(outcome);
        if let Some(account) = account {
            query = query.bind(account);
        }
        query.fetch_one(&self.pool).await.unwrap()
    }

    async fn account(&self, browser: &Browser) -> Value {
        let request = Request::get("/api/account")
            .header(header::COOKIE, browser.cookies())
            .body(Body::empty())
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        serde_json::from_slice(&body_bytes(response).await).unwrap()
    }
}

// Sign-up

#[tokio::test]
async fn sign_up_creates_one_account_and_signs_in() {
    let world = world().await;
    let mut browser = Browser::new();
    let (status, options) = world.sign_up_options(&mut browser, "fr").await;
    assert_eq!(status, StatusCode::OK);
    let pseudonym = options["publicKey"]["user"]["displayName"]
        .as_str()
        .unwrap()
        .to_owned();
    let without_number = pseudonym.rsplit_once(' ').unwrap().0;
    assert!(
        beelot_server::auth::pseudonyms::all(beelot_server::auth::pseudonyms::Language::French)
            .contains(&without_number.to_owned()),
        "{pseudonym}"
    );
    let credential = World::register(&mut browser, &options, ORIGIN);
    assert_eq!(
        world.finish_sign_up(&mut browser, &credential).await,
        StatusCode::NO_CONTENT
    );

    let account = world.account(&browser).await;
    assert_eq!(account["signedIn"], true);
    assert_eq!(account["name"], pseudonym);
    assert_eq!(world.count("account").await, 1);
    assert_eq!(world.count("passkey").await, 1);
    assert_eq!(world.count("session").await, 1);
    let id = uuid::Uuid::from_slice(&browser.credential.as_ref().unwrap().1).unwrap();
    assert_eq!(world.events("SIGN_UP", "SUCCESS", Some(id)).await, 1);
    assert_eq!(world.events("SIGN_IN", "SUCCESS", Some(id)).await, 1);
}

#[tokio::test]
async fn registration_requires_resident_key() {
    let world = world().await;
    let (_, options) = world.sign_up_options(&mut Browser::new(), "en").await;
    let selection = &options["publicKey"]["authenticatorSelection"];
    assert_eq!(selection["residentKey"], "required");
    assert_eq!(selection["requireResidentKey"], true);
    assert_eq!(selection["userVerification"], "required");
    assert_eq!(options["publicKey"]["rp"]["id"], "localhost");
    assert_eq!(options["publicKey"]["rp"]["name"], "Beelot");
}

#[tokio::test]
async fn a_cancelled_sign_up_creates_no_account() {
    let world = world().await;
    let mut browser = Browser::new();
    let (_, first) = world.sign_up_options(&mut browser, "en").await;
    let (_, second) = world.sign_up_options(&mut browser, "en").await;
    assert_ne!(
        first["publicKey"]["challenge"],
        second["publicKey"]["challenge"]
    );
    assert_eq!(world.count("account").await, 0);
}

#[tokio::test]
async fn ceremony_state_is_single_use() {
    let world = world().await;
    let mut browser = Browser::new();
    let (_, options) = world.sign_up_options(&mut browser, "en").await;
    let credential = World::register(&mut browser, &options, ORIGIN);
    let ceremony = browser.ceremony.clone();
    assert_eq!(
        world.finish_sign_up(&mut browser, &credential).await,
        StatusCode::NO_CONTENT
    );
    browser.ceremony = ceremony;
    browser.session = None;
    assert_eq!(
        world.finish_sign_up(&mut browser, &credential).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(world.count("account").await, 1);
    assert_eq!(world.count("passkey").await, 1);
}

#[tokio::test]
async fn a_sign_up_without_options_is_refused() {
    let world = world().await;
    let mut browser = Browser::new();
    let (_, options) = world.sign_up_options(&mut browser, "en").await;
    let credential = World::register(&mut browser, &options, ORIGIN);
    let mut stranger = Browser::new();
    let (status, body, _) = world
        .post(
            &mut stranger,
            "/api/passkeys/sign-up",
            "application/json",
            serde_json::to_string(&credential).unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.is_empty(), "never why");
    assert_eq!(world.count("account").await, 0);
    assert_eq!(world.events("SIGN_UP", "FAILURE", None).await, 1);
}

#[tokio::test]
async fn a_passkey_from_another_origin_is_refused() {
    let world = world().await;
    let mut browser = Browser::new();
    let (_, options) = world.sign_up_options(&mut browser, "en").await;
    let credential = World::register(&mut browser, &options, "http://localhost:9999");
    assert_eq!(
        world.finish_sign_up(&mut browser, &credential).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(world.count("account").await, 0);
    assert_eq!(world.count("passkey").await, 0);
}

#[tokio::test]
async fn a_malformed_answer_is_refused() {
    let world = world().await;
    let mut browser = Browser::new();
    world.sign_up_options(&mut browser, "en").await;
    let (status, _, _) = world
        .post(
            &mut browser,
            "/api/passkeys/sign-up",
            "application/json",
            "{\"id\": 1}".into(),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(world.events("SIGN_UP", "FAILURE", None).await, 1);
}

#[tokio::test]
async fn ceremony_state_expires_after_five_minutes() {
    let world = world().await;
    let mut browser = Browser::new();
    let (_, options) = world.sign_up_options(&mut browser, "en").await;
    let credential = World::register(&mut browser, &options, ORIGIN);
    world
        .clock
        .advance(Duration::minutes(5) + Duration::seconds(1));
    assert_eq!(
        world.finish_sign_up(&mut browser, &credential).await,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn the_ceremony_cookie_is_short_lived_and_strict() {
    let world = world().await;
    let request = Request::post("/api/passkeys/sign-in/options")
        .header(header::COOKIE, format!("XSRF-TOKEN={CSRF}"))
        .header("X-XSRF-TOKEN", CSRF)
        .body(Body::empty())
        .unwrap();
    let response = world.app.clone().oneshot(request).await.unwrap();
    let cookie = set_cookie(&response, "BEELOT_CEREMONY").unwrap();
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Strict"), "{cookie}");
    assert!(cookie.contains("Max-Age=300"), "{cookie}");
}

// Sign-in

#[tokio::test]
async fn sign_in_with_the_passkey() {
    let world = world().await;
    let (mut browser, account) = world.signed_up().await;
    let signed_up_at = world.clock.now();
    browser.session = None;
    world.clock.advance(Duration::hours(1));

    let (status, body) = world.sign_in(&mut browser).await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let signed_in = world.account(&browser).await;
    assert_eq!(signed_in["signedIn"], true);
    assert_eq!(
        signed_in["lastSignInAt"],
        signed_up_at
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
        "the sign-up was the sign-in before"
    );
    assert_eq!(world.events("SIGN_IN", "SUCCESS", Some(account)).await, 2);
    let used: Option<time::PrimitiveDateTime> =
        sqlx::query_scalar("SELECT last_used_at FROM passkey")
            .fetch_one(&world.pool)
            .await
            .unwrap();
    assert_eq!(used.map(|at| at.assume_utc()), Some(world.clock.now()));
}

#[tokio::test]
async fn signing_in_twice_works_and_the_counter_goes_up() {
    let world = world().await;
    let (mut browser, account) = world.signed_up().await;
    assert_eq!(world.sign_in(&mut browser).await.0, StatusCode::NO_CONTENT);
    let first = stored_counter(&world.pool).await;
    assert_eq!(world.sign_in(&mut browser).await.0, StatusCode::NO_CONTENT);
    assert!(stored_counter(&world.pool).await > first);
    assert_eq!(world.events("SIGN_IN", "SUCCESS", Some(account)).await, 3);
}

async fn stored_counter(pool: &MySqlPool) -> u64 {
    let passkey: Vec<u8> = sqlx::query_scalar("SELECT passkey FROM passkey")
        .fetch_one(pool)
        .await
        .unwrap();
    let passkey: Value = serde_json::from_slice(&passkey).unwrap();
    passkey["cred"]["counter"].as_u64().unwrap()
}

#[tokio::test]
async fn sign_in_replaces_the_session_and_the_csrf_token() {
    let world = world().await;
    let (mut browser, _) = world.signed_up().await;
    let before = browser.session.clone();
    let (status, options) = world.sign_in_options(&mut browser).await;
    assert_eq!(status, StatusCode::OK);
    let assertion = World::assert(&mut browser, &options);
    let (status, _, headers) = world
        .post(
            &mut browser,
            "/api/passkeys/sign-in",
            "application/json",
            serde_json::to_string(&assertion).unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_ne!(browser.session, before, "a new session");
    let csrf = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|cookie| cookie.starts_with("XSRF-TOKEN="))
        .expect("a new CSRF token");
    assert!(!csrf.starts_with(&format!("XSRF-TOKEN={CSRF}")));
}

#[tokio::test]
async fn an_unknown_passkey_fails_without_a_reason() {
    let world = world().await;
    let (mut browser, _) = world.signed_up().await;
    sqlx::query("DELETE FROM passkey")
        .execute(&world.pool)
        .await
        .unwrap();
    browser.session = None;
    let (status, body) = world.sign_in(&mut browser).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.is_empty());
    assert_eq!(world.events("SIGN_IN", "FAILURE", None).await, 1);
}

#[tokio::test]
async fn sign_in_with_cloned_counter_is_refused() {
    let world = world().await;
    let (mut browser, _) = world.signed_up().await;
    assert_eq!(world.sign_in(&mut browser).await.0, StatusCode::NO_CONTENT);
    // A clone signed in meanwhile: the stored counter is ahead of this passkey's.
    let passkey: Vec<u8> = sqlx::query_scalar("SELECT passkey FROM passkey")
        .fetch_one(&world.pool)
        .await
        .unwrap();
    let mut passkey: Value = serde_json::from_slice(&passkey).unwrap();
    passkey["cred"]["counter"] = 100.into();
    sqlx::query("UPDATE passkey SET passkey = ?")
        .bind(passkey.to_string())
        .execute(&world.pool)
        .await
        .unwrap();
    browser.session = None;
    let (status, body) = world.sign_in(&mut browser).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.is_empty());
    assert_eq!(world.events("SIGN_IN", "FAILURE", None).await, 1);
}

#[tokio::test]
async fn sign_in_without_options_fails() {
    let world = world().await;
    let (mut browser, _) = world.signed_up().await;
    let (_, options) = world.sign_in_options(&mut browser).await;
    let assertion = World::assert(&mut browser, &options);
    browser.ceremony = None;
    let (status, _) = world.finish_sign_in(&mut browser, &assertion).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(world.events("SIGN_IN", "FAILURE", None).await, 1);
}

#[tokio::test]
async fn a_deleted_accounts_passkey_fails() {
    let world = world().await;
    let (mut browser, account) = world.signed_up().await;
    beelot_server::db::accounts::delete(&world.pool, account)
        .await
        .unwrap();
    browser.session = None;
    let (status, body) = world.sign_in(&mut browser).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.is_empty());
}

#[tokio::test]
async fn sign_up_and_sign_in_need_the_csrf_token() {
    let world = world().await;
    for path in [
        "/api/passkeys/sign-up/options",
        "/api/passkeys/sign-in/options",
    ] {
        let request = Request::post(path).body(Body::empty()).unwrap();
        assert_eq!(
            world.app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
}

#[tokio::test]
async fn there_is_no_endpoint_to_register_another_passkey() {
    let world = world().await;
    let (mut browser, _) = world.signed_up().await;
    for path in [
        "/webauthn/register/options",
        "/webauthn/register",
        "/api/passkeys/register",
    ] {
        let (status, _, _) = world
            .post(&mut browser, path, "application/json", "{}".into())
            .await;
        assert!(
            [StatusCode::NOT_FOUND, StatusCode::METHOD_NOT_ALLOWED].contains(&status),
            "{path}: {status}"
        );
    }
    assert_eq!(world.count("passkey").await, 1);
}

// PasskeyRequestFilterTest

#[tokio::test]
async fn ceremonies_are_limited_per_ip_with_429() {
    let world = world_with(Limits {
        passkey_ceremonies_per_ip_per_hour: 1,
        ..Limits::default()
    })
    .await;
    let mut browser = Browser::new();
    assert_eq!(
        world.sign_up_options(&mut browser, "en").await.0,
        StatusCode::OK
    );
    let (status, _, headers) = world
        .post(
            &mut browser,
            "/api/passkeys/sign-in/options",
            "application/json",
            String::new(),
        )
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(headers[header::RETRY_AFTER], "3600");
    assert_eq!(world.events("RATE_LIMITED", "REFUSED", None).await, 1);
    let ip: Option<String> =
        sqlx::query_scalar("SELECT ip_address FROM security_event WHERE type = 'RATE_LIMITED'")
            .fetch_one(&world.pool)
            .await
            .unwrap();
    assert_eq!(
        ip.as_deref(),
        Some("0.0.0.0"),
        "the peer's address: forwarded headers are not trusted"
    );
}

#[tokio::test]
async fn finishing_a_ceremony_is_never_limited() {
    let world = world_with(Limits {
        passkey_ceremonies_per_ip_per_hour: 1,
        ..Limits::default()
    })
    .await;
    let mut browser = Browser::new();
    let (_, options) = world.sign_up_options(&mut browser, "en").await;
    let credential = World::register(&mut browser, &options, ORIGIN);
    assert_eq!(
        world.finish_sign_up(&mut browser, &credential).await,
        StatusCode::NO_CONTENT
    );
}
