# Create an account with my email address (US-075, ADR-004) — design

## Goal

As a player without a Google or GitHub account, I want to create an account
with my email address, so that my matches are recorded.

ADR-004 replaces Zitadel with sign-in links sent by Spring, and brings Google
and GitHub back as direct OAuth providers. This design delivers US-075 again
on that basis and removes Zitadel. Removing Zitadel also removes Google and
GitHub sign-in, so their return is part of this story, and so is the rate
limit of the link form, which must not ship without it. Both are moved from
US-077 in the backlog.

## Acceptance criteria and how they are met

| Criterion | Met by |
| --- | --- |
| "Sign in" asks for an email address and offers Google and GitHub | Sign-in panel on the home screen (client) |
| Spring emails a link that works once and expires after 15 minutes | `oneTimeTokenLogin()`, `SignInTokenService`, `SignInLinkSender` |
| Only a hash of the token is stored | `SignInTokenService` (SHA-256) |
| The link opens a page with a button, so mail scanners cannot use it up | The `/sign-in` view posts the token |
| The first link opened creates the account and proves the address | `SignInSuccessHandler`, `AccountService.signInWithEmail` |
| Links never opened create no account; expired tokens are purged | `SignInTokenService.purgeExpired` |
| The answer is always "check your inbox" | `SignInLinkSender`: one answer and one email for every address |
| A random pseudonym; the address in a table of its own | `account` and `account_email` |
| Link requests are rate limited (from US-077) | `SignInRateLimiter` |
| Google and GitHub sign in directly, with the user id only (from US-077) | `spring.security.oauth2.client` registrations |
| Zitadel is removed | See "Removed" |

## Decisions

- **One flow for registration and sign-in.** The form never says whether an
  address has an account. Every address gets the same email: "Use this link to
  sign in to Beelot. If you have no account yet, it creates one." The
  response, its timing and the email are identical either way.
- **The token.** 32 random bytes from `SecureRandom`, encoded as base64url
  (43 characters). The database keeps its SHA-256 hash, the normalised email
  address and the expiry. Using the token deletes its row in the same
  transaction, so it works once. A token is a secret with 256 bits of
  entropy, so a salt-free fast hash is enough.
- **Email addresses are normalised**: trimmed and lower-cased as a whole. At
  most 254 characters, with one `@` and a dot in the domain. Lower-casing the
  local part is not strictly correct, but mailboxes that tell cases apart are
  rare, and two accounts for one person would be worse.
- **The link uses a configured base URL** (`beelot.public-url`), never the
  request's `Host` header. Otherwise anyone could request a link for someone
  else's address whose URL points to their own site.
- **The link opens the client, not Spring.** `/sign-in?token=…` is a view of
  the single-page client. It removes the token from the address bar
  (`history.replaceState`), and the player confirms with "Sign in". The
  client then posts the token to `/login/ott` with the CSRF header. The page
  is served with `Referrer-Policy: no-referrer`. Spring's default submit page
  is turned off.
- **CSRF on both endpoints.** Requests that change state carry the CSRF token
  only once a session exists (US-057). The link request and the link sign-in
  happen before that, so both always require the token, which the client
  already reads from the `XSRF-TOKEN` cookie. This prevents a login CSRF,
  where a site signs a visitor into the attacker's account.
- **The session principal is the email address.** After a link sign-in,
  Spring Security's authentication holds the address, and
  `AccountService.current` finds the account through `account_email`. Google
  and GitHub keep their lookup by provider and subject.
- **Email accounts in the `account` table.** `provider = 'email'`, and
  `provider_subject` is the account id, so the unique key still holds and no
  address is copied there. The address lives in `account_email(account_id,
  email)`.
- **Plain-text emails** in English, French or Dutch, chosen from the client's
  `language` parameter, else `Accept-Language` (the pseudonym follows the
  same rule as the email). Plain text renders everywhere and needs no template
  engine.
- **Sending is synchronous.** If the SMTP server fails, the request answers
  503, "The email could not be sent. Please try again later.", for every
  address, so the answer reveals nothing. The log line holds no address.
- **Rate limits.** At most 3 links per address per 15 minutes, and 10 per IP
  address per hour. Over either limit, the request answers 429, "Too many
  links requested. Please try again later.", and no email is sent. The limit
  applies to every address, known or not. Counters live in memory, bounded
  like `RequestRateLimitFilter`'s (one instance, ADR-004).
- **Google and GitHub ask for the user id only.** Google uses the `openid`
  scope, so its ID token holds `sub` and Spring does not call the userinfo
  endpoint. GitHub uses no scope; Spring reads `/user` and Beelot keeps only
  `id`. Credentials come from `SPRING_SECURITY_OAUTH2_CLIENT_REGISTRATION_*`,
  bound at run time as in US-057, so the native image works without them. A
  provider without credentials is not offered.
- **Development data.** Accounts with `provider = 'zitadel'` exist only in
  development databases. A migration deletes them and the matches recorded
  for them, as V3 did for the first OAuth accounts.

## Components

### Security configuration (changed)

```
http.oneTimeTokenLogin(ott -> ott
        .tokenGeneratingUrl("/api/sign-in/link")
        .tokenGenerationSuccessHandler(signInLinkSender)
        .tokenService(signInTokenService)
        .loginProcessingUrl("/login/ott")
        .showDefaultSubmitPage(false)
        .successHandler(signInSuccessHandler)
        .failureHandler(statusOnly(401)))
    .oauth2Login(login -> login.loginPage("/").successHandler(signInSuccessHandler)
        .failureUrl("/?signin=failed"))
    .logout(logout -> logout.logoutUrl("/logout")
        .logoutSuccessHandler(new HttpStatusReturningLogoutSuccessHandler(HttpStatus.NO_CONTENT)));
```

- The CSRF matcher also requires the token on `/api/sign-in/link` and
  `/login/ott`.
- `ClientRegistrationRepository` binds `spring.security.oauth2.client` at run
  time (US-057's version, restored).
- One-time-token login needs a `UserDetailsService`. `EmailUserDetailsService`
  returns, for any address, a user with that name, no password and the
  `EMAIL_USER` authority. It runs only after a token has been checked and
  used, so it can trust the address.

### `SignInTokenService` (new, `fr.beelot.application.account`)

Implements Spring's `OneTimeTokenService`.

- `generate(request)` creates the token, stores `sign_in_token(token_hash,
  email, expires_at)` with an expiry of `beelot.sign-in.link-lifetime`
  (default `PT15M`), and returns a `OneTimeToken` holding the clear token.
- `consume(authentication)` hashes the presented token, deletes the row and
  returns it, all in one transaction. It returns null when the row is missing
  or expired.
- `purgeExpired()` runs every hour and deletes rows that expired more than a
  day ago (US-083).

### `SignInLinkSender` (new)

Implements `OneTimeTokenGenerationSuccessHandler`. It builds
`{beelot.public-url}/sign-in?token=…`, picks the language, sends the email
through `MailSender` from `beelot.mail.from`, and answers 204. A
`MailException` answers 503.

Before the token is generated, the address is validated (400 when invalid)
and the rate limiter is consulted (429). Both run in a filter in front of
Spring's generation filter, so no token is stored for a refused request.

### `SignInRateLimiter` (new)

Two fixed-window counters, per address and per IP, bounded in size. Limits:
`beelot.sign-in.limit-per-address=3` per `PT15M`, and
`beelot.sign-in.limit-per-ip=10` per `PT1H`.

### Accounts (changed)

- `V4__email_accounts.sql` creates `account_email(account_id UUID PRIMARY KEY
  REFERENCES account(id) ON DELETE CASCADE, email VARCHAR(254) NOT NULL
  UNIQUE)` and `sign_in_token(token_hash CHAR(64) PRIMARY KEY, email
  VARCHAR(254) NOT NULL, expires_at DATETIME(6) NOT NULL)`. It also deletes
  the `zitadel` accounts and their matches.
- `AccountService.signInWithEmail(email, locale)` finds the account through
  `account_email`, or creates the `account` row with a pseudonym and the
  `account_email` row. `current(authentication)` handles both kinds of
  authentication.
- `SignInSuccessHandler` calls `signInWithEmail` for a one-time-token
  authentication, and `signIn(provider, subject, locale)` for OAuth, as
  today. For the link sign-in it answers 204 rather than redirecting, because
  the client posts with `fetch`.

### `/api/account` (changed)

A guest gets `{"signedIn": false, "emailSignIn": true, "providers": [{"id":
"google", "name": "Google", "signInUrl": "/oauth2/authorization/google"},
…]}`. A signed-in player gets `{"signedIn": true, "name": …}`.
`registerUrl` and `signInUrl` are removed.

### Client (changed)

- The guest account bar shows "Sign in", which opens a panel: an email field,
  "Email me a sign-in link", and "Continue with Google" or "Continue with
  GitHub" for each configured provider. After the request it says: "Check your
  inbox. If you can't find the email, look in your spam folder. The link works
  for 15 minutes." The 429 and 503 messages are shown in the panel.
- The `/sign-in` view says "Sign in to Beelot" with a "Sign in" button. On
  success it goes home. On a 401 it says "This link has expired or has already
  been used." and offers the panel.
- The statistics page's sign-in prompt opens the same panel.
- New texts are translated into French and Dutch.

### Configuration

```properties
# The address players use to reach Beelot; sign-in links point here (ADR-004).
beelot.public-url=${BEELOT_PUBLIC_URL:http://localhost:8080}
spring.mail.host=${BEELOT_MAIL_HOST:localhost}
spring.mail.port=${BEELOT_MAIL_PORT:1025}
spring.mail.username=${BEELOT_MAIL_USERNAME:}
spring.mail.password=${BEELOT_MAIL_PASSWORD:}
beelot.mail.from=${BEELOT_MAIL_FROM:Beelot <no-reply@beelot.localhost>}
beelot.sign-in.link-lifetime=PT15M
spring.security.oauth2.client.registration.google.scope=openid
```

GitHub's empty scope is set in code, because an empty property cannot
override Spring Boot's `read:user` default. The defaults point at Mailpit,
as the database defaults point at MariaDB.

The new dependency is `spring-boot-starter-mail`.

### Removed

- `compose.yaml`: the Zitadel services, its proxy and its database. Mailpit
  stays and exposes SMTP on port 1025.
- `zitadel/`, and the `BEELOT_ZITADEL_*` variables.
- `ZitadelProperties`, `ZitadelUsers`, `ZitadelApiException`,
  `ZitadelLogoutSuccessHandler`, `RegisterAuthorizationRequestResolver`,
  `UnconfirmedAccountCleanup`, and their tests.
- The README's Zitadel section, replaced by the SMTP, public URL and
  Google/GitHub variables.

## Error handling

| Situation | Behaviour |
| --- | --- |
| Invalid address | 400, "Enter a valid email address." |
| Rate limit reached | 429, no token stored, no email |
| SMTP fails | 503, the token row stays and expires; WARN without the address |
| Expired, used or unknown token | 401; the client offers a new link |
| No CSRF token | 403 |
| A provider without credentials | Not offered |

## Testing

- `SignInTokenServiceTest` (MariaDB in Testcontainers): only the hash is
  stored; a token works once; an expired token is refused; the purge deletes
  only rows expired for over a day.
- `SignInLinkSenderTest`: the link uses `beelot.public-url` whatever the
  `Host` header; the email is in the requested language; an SMTP failure
  answers 503 without the address in the log.
- `SignInRateLimiterTest`: the per-address and per-IP limits, and the window
  reset.
- `EmailSignInIntegrationTest` (MockMvc and a mocked `MailSender`):
  - a request without a CSRF token is refused;
  - an unknown and a known address get the same answer and the same email;
  - posting the token from the email signs the player in, creates the account
    once, and `/api/account` shows the pseudonym;
  - the same token posted twice fails the second time;
  - the session id changes on sign-in.
- `AccountIntegrationTest`: Google and GitHub are offered when configured;
  Google asks for `openid` only, and GitHub for no scope.
- The migration runs on a database holding `zitadel` accounts.
- End to end, in a headless browser against `docker compose`: request a link,
  open it from Mailpit, sign in, see the pseudonym, sign out, request again
  and use the old link (refused).
- `./gradlew test` and the native build stay green.

## Out of scope

- Session lifetimes, sign-out wording and the last sign-in time (US-077).
- Changing the email address (US-079), deleting the account (US-081), the
  privacy policy link on the form (US-083).
- Linking an email account with a Google or GitHub account.
