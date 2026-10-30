# Email accounts through Zitadel (US-075) — design

## Goal

As a player without a Google or GitHub account, I want to create an account
with my email address and a password, so that my matches are recorded.

ADR-003 proposes Zitadel as the identity provider. This design delivers
US-075 and makes Zitadel the only sign-in provider: Google and GitHub move
behind Zitadel now rather than in US-077.

## Decisions

- **Zitadel is the only provider.** Spring signs players in with OpenID
  Connect against Zitadel. The direct `google` and `github` client
  registrations are removed. Google and GitHub become identity providers
  configured inside Zitadel, shown on Zitadel's sign-in page.
- **The user id is the only data that crosses from Zitadel to Beelot.**
  Spring requests the `openid` scope only. It receives the Zitadel user id
  (`sub`) and never asks for, stores, logs or displays a name, an email
  address or any other claim. The user id is still pseudonymised personal
  data under the GDPR, so deletion and export (US-081, US-082) still apply
  to it.
- **Players appear under a random pseudonym.** On the first sign-in, Beelot
  generates a name such as "Swift Otter 42" and stores it in
  `account.display_name`. It is used at tables, in match history and in
  statistics. US-072 later lets the player change it.
- **Zitadel enforces email confirmation.** Its login container runs with
  `EMAIL_VERIFICATION=true`, so an unconfirmed account cannot finish signing
  in. Spring has no claim to check it with, and needs none.
- **Existing accounts are not migrated.** No real accounts exist yet, so a
  Flyway migration deletes the development accounts and their match history.
- **Development runs Zitadel locally** with Docker Compose. A script applies
  Zitadel's settings through its API, both to the local stack and to Zitadel
  Cloud.
- **Tests simulate the sign-in.** They use Spring Security's `oidcLogin()`
  with MariaDB in Testcontainers; `./gradlew test` needs no Zitadel. The real
  flow is checked once in a headless browser against the local stack.
- **The privacy policy link is deferred to US-083.** Until that page exists,
  Zitadel's registration page shows no policy link.

## Sign-in flow

1. A guest clicks **"Create an account"**
   (`/oauth2/authorization/zitadel?register`) or **"Sign in"**
   (`/oauth2/authorization/zitadel`).
2. A custom `OAuth2AuthorizationRequestResolver` wraps the default one and
   adds `prompt=create` when the `register` parameter is present, so Zitadel
   opens its registration form directly.
3. Zitadel asks for the first name, last name, email address and password
   (at least 12 characters), sends a confirmation code, and completes the
   flow only once the address is confirmed.
4. Spring exchanges the code and validates the ID token (issuer, audience,
   signature through Zitadel's keys).
5. `SignInSuccessHandler` calls `AccountService.signIn("zitadel", sub,
   locale)`:
   - an unknown user id creates an account with a new pseudonym in the
     request's language;
   - a known user id only updates `last_sign_in_at`; the pseudonym is never
     overwritten.
6. The player returns to the home screen, which shows the pseudonym.

### Client registration

`ZitadelProperties` (`beelot.zitadel.issuer`, `client-id`, `client-secret`,
from `BEELOT_ZITADEL_ISSUER`, `BEELOT_ZITADEL_CLIENT_ID` and
`BEELOT_ZITADEL_CLIENT_SECRET`) feeds a `ClientRegistrationRepository` built
in code:

| Field | Value |
| --- | --- |
| registration id | `zitadel` |
| scope | `openid` |
| authorization URI | `{issuer}/oauth/v2/authorize` |
| token URI | `{issuer}/oauth/v2/token` |
| JWK set URI | `{issuer}/oauth/v2/keys` |
| user info URI | none (the `sub` comes from the ID token) |
| user name attribute | `sub` |
| issuer URI | `{issuer}` |
| end session endpoint | `{issuer}/oidc/v1/end_session` |

Because the endpoints are built from the issuer, Spring fetches nothing from
Zitadel at startup: the server starts while Zitadel is down. The native image
needs no binding hints for `OAuth2ClientProperties`, so they are removed.
When no issuer is configured, the repository is empty and sign-in is hidden.

### Sign-out

`POST /logout` ends the Spring session and returns `200 {"logoutUrl": …}`.
That URL is Zitadel's end-session endpoint with `id_token_hint`,
`client_id` and `post_logout_redirect_uri={BEELOT_URL}/`. The client
navigates to it, so Zitadel's session also ends and a shared device does not
sign the previous player back in. Without a Zitadel session (no ID token),
`logoutUrl` is `/`.

### Pseudonyms

`PseudonymGenerator` picks an adjective, an animal and a number from 10 to
99, from word lists in English, French and Dutch:

- English: "Swift Otter 42"; French: "Loutre Agile 42"; Dutch:
  "Snelle Otter 42".
- The language is the best match of the `Accept-Language` header among
  English, French and Dutch, with English as the fallback.
- Every combination is at most 30 characters (checked by a test over the
  lists).
- Pseudonyms are not unique; the number makes repeats rare, and players are
  identified by their account id, not their name.

The current profile-name logic (`AccountService.displayName(attributes)`) is
removed.

## API and interface

`GET /api/account` returns:

- guest: `{"signedIn": false, "signInUrl": "/oauth2/authorization/zitadel",
  "registerUrl": "/oauth2/authorization/zitadel?register"}`, without the two
  URLs when Zitadel is not configured;
- signed in: `{"signedIn": true, "name": "Swift Otter 42"}`.

The `providers` list and the `provider` field are removed.

`app.js` shows **"Sign in"** and **"Create an account"** on the home screen
and in the statistics page's invitation to sign in, the pseudonym and
**"Sign out"** when signed in, and follows `logoutUrl` after signing out. The
text "Sign in with {0}" is removed. New texts and the word lists are
translated into French and Dutch.

## Errors

- A cancelled or failed sign-in (`error` in the callback, state mismatch,
  token exchange failure) sends the player to `/?signin=failed`, which shows
  a translated message.
- If Zitadel is unreachable, the browser shows the connection error when the
  player clicks "Sign in". Guests keep playing every mode.
- If Zitadel is unreachable at sign-out, the Spring session has already
  ended.
- Nothing from Zitadel is logged except the user id, at debug level.

## Data

Flyway `V3__reset_accounts_for_zitadel.sql` deletes the match records that
have a seat linked to an account (with their seats and rounds), clears
`match_seat.account_id`, then deletes every account. The schema is
unchanged: `account(provider, provider_subject)` now always holds
`("zitadel", <user id>)`.

## Local Zitadel stack

`compose.yaml` gains, next to MariaDB, services adapted from Zitadel's
`deploy/compose` and pinned to Zitadel v4.19.4:

| Service | Role | Published |
| --- | --- | --- |
| `zitadel-db` | PostgreSQL 17 for Zitadel | — |
| `zitadel-api` | Zitadel, `start-from-init` | — |
| `zitadel-login` | Login pages, `EMAIL_VERIFICATION=true` | — |
| `zitadel-proxy` | Traefik, routes the API and the login pages under one host | `localhost:8081` |
| `mailpit` | Catches every email | SMTP `1025` (internal), inbox `localhost:8025` |

- Zitadel's external domain is `localhost`, port `8081`, without TLS.
- At its first start, Zitadel creates an admin user for the console and a
  machine user `beelot-setup` with a personal access token written to
  `./zitadel/.bootstrap/` (git-ignored, bind-mounted).
- Secrets (master key, login cookie secret, database and admin passwords) are
  fixed development values, commented as local-only.

## Provisioning script

`zitadel/provision.sh` (bash, curl, jq) applies Zitadel's settings through
its API and can be run any number of times. Inputs, all with local defaults:

- `ZITADEL_URL` (`http://localhost:8081`), `BEELOT_URL`
  (`http://localhost:8080`), `ZITADEL_TOKEN_FILE`
  (`zitadel/.bootstrap/beelot-setup.pat`);
- optional `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASSWORD`,
  `SMTP_SENDER` (default: Mailpit, no authentication);
- optional `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` and
  `GITHUB_CLIENT_ID`/`GITHUB_CLIENT_SECRET`.

Steps:

1. Wait until Zitadel is ready.
2. Instance settings:
   - password complexity: minimum length 12, no required character classes;
   - lockout: 10 password attempts, 10 one-time-code attempts;
   - login: registration on, username and password on, external identity
     providers on, ignore unknown usernames, passwordless not allowed, MFA
     not forced;
   - allowed languages: English, French and Dutch;
   - branding: Beelot's colours, activated.
3. SMTP provider, activated.
4. Project "Beelot" and OIDC application `beelot-web`: web application,
   authorization code, client secret (basic), redirect URI
   `{BEELOT_URL}/login/oauth2/code/zitadel`, post-logout redirect URI
   `{BEELOT_URL}/`, development mode only when `BEELOT_URL` uses http. When
   the application exists but `.env` holds no secret, a new secret is
   generated.
5. Google and GitHub identity providers when their credentials are set,
   allowed to create and link users, and added to the login settings.
6. Write `BEELOT_ZITADEL_ISSUER`, `BEELOT_ZITADEL_CLIENT_ID` and
   `BEELOT_ZITADEL_CLIENT_SECRET` to the git-ignored `.env`, imported by
   Spring through `spring.config.import=optional:file:.env[.properties]`.

Production runs the same script against Zitadel Cloud with its URL, a
machine user's token and real SMTP settings.

## Testing

- `PseudonymGeneratorTest`: every combination is at most 30 characters;
  language selection, including the English fallback.
- `AccountServiceTest`: the first sign-in creates an account with a
  pseudonym; a later sign-in keeps it and updates `last_sign_in_at`.
- `RegisterAuthorizationRequestResolverTest`: `?register` adds
  `prompt=create`; the plain sign-in URL does not.
- `AccountIntegrationTest` (MariaDB in Testcontainers, `oidcLogin()` on the
  `zitadel` registration):
  - `/api/account` for a guest, a signed-in player, and with Zitadel not
    configured;
  - sign-out returns Zitadel's end-session URL with `id_token_hint` and the
    post-logout redirect;
  - requests that change state still need the CSRF token once signed in.
- Manual check in a headless browser against the compose stack: register,
  confirm that an unconfirmed account cannot sign in, confirm through
  Mailpit, sign in, see the pseudonym, sign out; then check that MariaDB and
  the Spring logs hold no email address or name.
- `zitadel/provision.sh` run twice in a row succeeds.
- `./gradlew nativeCompile` still builds.

## Documentation

- README: an "Accounts (Zitadel)" section replaces the Google and GitHub
  setup: starting the stack, running the provisioning script, environment
  variables, Mailpit and the Zitadel console.
- Backlog, US-075: a pseudonym instead of a display-name field, Zitadel's
  registration form, email confirmation enforced by Zitadel, the privacy link
  moved to US-083, and Google and GitHub behind Zitadel (taken from US-077).
- ADR-003: record the "user id only" rule, and add a check that Zitadel
  Cloud's hosted login enforces email verification.

## Out of scope

Unconfirmed-account cleanup (US-076), the 30-minute unlock and the session
limits (US-077), password reset and account pages (US-078, US-079), second
factors (US-080), deletion and export (US-081, US-082), the privacy policy
page (US-083), choosing a display name (US-072), and TLS for the local stack.
