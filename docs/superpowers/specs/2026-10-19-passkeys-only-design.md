# Passkeys only (US-084) — design

## Goal

Players sign in with a passkey and nothing else. Email-link sign-in, Google,
GitHub and the SMTP sender are removed. Beelot then holds no email address,
sends no email and depends on no identity provider. Every existing account is
deleted: there is no migration path.

## Decisions

- **Clean break.** Flyway migration V8 deletes every account with the rules of
  US-081. Finished matches stay in other players' history; the deleted seats
  show "Deleted player". Sessions end on deploy.
- **One passkey per account.** It is registered at sign-up and can't be added,
  replaced or removed later. A passkey synced by the platform (iCloud Keychain,
  Google Password Manager) works on all the player's devices; a player on a new
  device without sync creates a new account.
- **No recovery.** No link, no code, no support process. A lost passkey is a
  lost account. Sign-up says so before the passkey is created.
- **Spring Security's WebAuthn support** (`http.webAuthn(...)`, module
  `spring-security-webauthn` with webauthn4j) with its JDBC repositories, so
  that challenge, origin and signature checks are not Beelot code. Spring does
  not check the signature counter; Beelot does, in a decorator of the relying
  party (synced passkeys report 0 and are exempt).
  A hosted passkey service is rejected: it adds a processor (ADR-004's reason
  for leaving Zitadel). Using webauthn4j directly is rejected: more
  security-critical code for no gain.
- **Discoverable credentials.** Sign-in shows one button; the browser offers
  the Beelot passkey, and the player types no username. Registration requires
  a resident key and user verification.
- **The pseudonym is the WebAuthn display name.** Sign-up asks for nothing:
  the account gets a random pseudonym (US-075) and keeps it until the player
  chooses a display name (US-072). The WebAuthn user name is the account id:
  Spring finds user entities by name, and pseudonyms are not unique. The
  WebAuthn user handle is random and not the account id.

## Flows

### Sign-up

1. The sign-in form offers "Create an account". It shows the warning: the
   passkey is the only way to sign in; losing it loses the account, with no
   recovery.
2. The player confirms. Spring creates the registration options; the browser
   creates the passkey; Spring verifies and stores it.
3. Beelot creates the `account` row with a pseudonym, links the credential to
   it and signs the browser in.

A registration that fails or is cancelled creates no account.

### Sign-in

"Sign in with a passkey" starts a WebAuthn authentication without a user name.
Spring verifies the assertion; Beelot checks the signature counter, resolves the
account through the credential's user handle, records the sign-in time
(`last_sign_in_at`, `previous_sign_in_at`) and creates a new session id.

### Deletion

"Delete my account" keeps the US-081 rule of a sign-in less than 10 minutes
old. When it is older, the client asks for the passkey again (a new sign-in),
then for confirmation. Deletion removes the account, its credential and its
user entity, and ends its sessions.

### Sessions

Unchanged: a session ends after 7 days without a request and lasts at most 30
days; signing out ends it.

## Data

### Migration V8

- Apply `AccountDeletionService`'s rules to every account at once: seats
  linked to an account get `account_id = NULL` and the "Deleted player" name,
  security events with an account are deleted, then every `account` row is
  deleted.
- Drop `email_change_token`, `sign_in_token` and `account_email`.
- Drop `account.provider`, `account.provider_subject` and
  `account.deletion_warned_at`, and the unique key on provider and subject.
- Create Spring's WebAuthn tables in MariaDB types:
  - `user_entities` (id, name, display_name), the WebAuthn user handle, with
    a unique name and Beelot's own `created_at`, so that sign-ups never
    finished can be purged after a day;
  - `user_credentials` (credential id, user entity id, public key, signature
    count, transports, flags, attestation data, created and last-used dates,
    label).
- A unique key on `user_credentials.user_entity_user_id`, so that an account
  has exactly one passkey. The account is linked through `user_entities.name`;
  `account` gets no new column.

V8 runs on startup, after `db-update-v2-to-v7.sql` has brought production to
version 7.

### Stored per player

Pseudonym or display name, creation date, last and previous sign-in dates, the
passkey's public key, signature counter and dates, matches, and security
events (with IP address, kept 12 months).

## Code

### Removed

- Email-link sign-in: `SignInEmail`, `SignInLinkSender`, `SignInRateLimiter`,
  `SignInRequestFilter`, `SignInTokenService`, `Tokens`,
  `EmailUserDetailsService`, `EmailAddresses`, `AccountEmail`,
  `AccountEmailRepository`.
- Email change: `EmailChangeController`, `EmailChangeService`,
  `EmailChangeEmail`.
- `InactivityWarningEmail`.
- Google and GitHub sign-in: the OAuth client registrations and the code that
  maps a provider user to an account.
- Dependencies `spring-boot-starter-mail` and
  `spring-boot-starter-security-oauth2-client`; the `spring.mail.*`,
  `beelot.mail.*`, `BEELOT_MAIL_*` and Google and GitHub client settings.
- Mailpit in `compose.yaml`.
- Their pages, scripts, EN/FR/NL messages and tests.

### Changed

- `SecurityConfiguration`: `webAuthn()` with the relying party id and allowed
  origin taken from `BEELOT_PUBLIC_URL`, the JDBC repositories, sessions,
  CSRF and the existing request limits. Sign-up and sign-in endpoints are
  rate limited per IP address and answer 429 above the limit.
- `SignInSuccessHandler`: records the sign-in times for a passkey sign-in.
- `Account`, `AccountService`, `AccountRepository`: no provider; the account
  is found by WebAuthn user handle.
- `AccountController` and `/api/account`: no `email` and no `provider`.
- `RetentionJob`: deletes accounts after 3 years without a sign-in, without a
  warning; no token purge. Security events are still kept 12 months.
- `SecurityEvents`: events for passkey sign-up, sign-in, failed sign-in,
  sign-out, session expiry, rate-limit refusal, export and deletion. The
  link-request and email-change events go.
- `DataExport`: pseudonym or display name, dates, the passkey's creation and
  last-use dates, sign-in history and matches; never the public key or the
  credential id.
- `AccountDeletionService`: also deletes the credential and user entity.
- The privacy policy: hosting is the only processor; no email address,
  Google or GitHub; the session and CSRF cookies are still the only cookies.
- `AccountRuntimeHints`: hints for webauthn4j and the WebAuthn repositories
  where the native image needs them.

## Documentation

- ADR-005 "Passkeys only" supersedes ADR-004. ADR-003 and ADR-004 stay as
  history with an updated status line.
- Backlog: US-084 rewritten to this design (one passkey, passkey only). US-075,
  US-077 and US-079 marked superseded by US-084. US-081 to US-083 updated where
  they mention email, Google or GitHub. The decision "Choose the SMTP provider
  for production" is struck through.
- README: SMTP, Mailpit, Google and GitHub setup removed; passkey sign-in and
  the `localhost` relying party for development described.

## Error handling

- A cancelled or failed WebAuthn ceremony shows a translated message and
  leaves the player signed out; nothing is created.
- A browser without WebAuthn support sees that passkeys are needed, and can
  still play as a guest.
- An unknown or deleted credential answers as a failed sign-in, without saying
  why.
- A signature counter that goes backwards fails the sign-in and records a
  security event.

## Testing

- Spring Security test support for registration and authentication, including
  the one-passkey rule, rate limits and the security events.
- V8 against MariaDB with Testcontainers: accounts deleted, matches kept with
  "Deleted player", email tables and columns gone, WebAuthn tables present.
- Data export and deletion with a passkey account.
- Headless Playwright with a virtual authenticator: sign-up, sign-out, sign-in,
  re-sign-in before deletion, deletion.
- Sign-up and sign-in checked in the native image (US-031).
- The full suite and the build pass, and no test refers to email or OAuth sign-in.

## Out of scope

Recovery of any kind, several passkeys per account, account linking, and
moving existing accounts to passkeys.
