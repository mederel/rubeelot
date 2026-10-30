# ADR-004: Sign in with an email link in Spring, and with Google and GitHub directly

- Status: Superseded by ADR-005 (passkeys only); it had replaced the Zitadel proposal of ADR-003
- Date: 2026-10-10
- Decision makers: Product and engineering

## Context

ADR-003 proposed Zitadel Cloud for email accounts. US-075 and US-076 were
built on it: Spring signs players in with OpenID Connect, Google and GitHub
sit behind Zitadel, `zitadel/provision.sh` applies the settings, and a Spring
job deletes unconfirmed accounts through Zitadel's API. ADR-003 was never
accepted: its checks on the terms of service and on erasure from the event
history are still open.

Building US-077 showed three problems:

- **Workarounds pile up.** Spring compensates for what Zitadel lacks: a job
  to unlock accounts, with the lock time guessed from the user's last change;
  a job to delete unconfirmed accounts, with a role check so that it spares
  administrators; Zitadel's own sign-in session to shorten, so that Spring's
  15-minute timeout means something. Each needs a service account, an API
  token and end-to-end checks.
- **Operations.** Zitadel is one more service to run, with its own PostgreSQL
  database, or an external free plan capped at 100 daily active users,
  without an uptime guarantee and without a custom domain.
- **Privacy.** Zitadel is one more processor, with US subprocessors, a data
  processing agreement to sign, and an event store whose erasure is
  unverified.

ADR-003 rejected storing accounts in Spring because the project would build
and secure every password flow: registration, confirmation, reset, change,
lock, second factor, and password hashes that a leak would expose. Most of
that work exists only because of passwords.

## Options

1. **Keep Zitadel**, Cloud or self-hosted. The workarounds stay; self-hosting
   adds operations.
2. **Passwords in Spring** (ADR-003, option 1). Every control can be met as
   written, but it is the largest amount of security-critical code, and
   Beelot holds password hashes.
3. **Email links in Spring, with Google and GitHub as direct OAuth
   providers.** No password exists.
4. **Google and GitHub only.** The least work, but players without either
   account cannot keep a history, which is the goal of phase 5.

## Decision

Proposed: **option 3.**

### Email link sign-in

- The player enters an email address. Spring emails a one-time link; opening
  it signs the player in. Spring Security provides the flow
  (`oneTimeTokenLogin()`); Beelot supplies the email sender
  (a `OneTimeTokenGenerationSuccessHandler`) and a token store in MariaDB, so
  that links survive a restart.
- A link works once and expires after 15 minutes. Beelot's token store keeps
  only a hash of the token; Spring's own JDBC store keeps it in clear, so it
  is not used.
- The link opens a page where the player confirms with a button, which sends
  the token by POST. Mail scanners that open links in emails therefore cannot
  use up the token.
- Registration and sign-in are the same flow: the first link opened creates
  the account. Opening the link proves the address, so no separate
  confirmation and no cleanup of unconfirmed accounts are needed. Requested
  links that are never opened create no account; expired tokens are purged.
- The answer to a request is always "check your inbox", so the form never
  reveals whether an address has an account. An existing account gets a
  sign-in link; a new address gets a link that creates the account.
- Link requests are rate limited per address and per IP address, so that the
  form cannot be used to flood an inbox or the SMTP provider.
- The email is sent in the player's language (English, French or Dutch).

### Google and GitHub

- They return as direct OAuth providers of Spring Security, as in US-057.
- Beelot asks for the provider's user id only: the `openid` scope for Google,
  no scope for GitHub. It keeps no name and no email address from them.
- A player who signs in with Google and with an email link gets two separate
  accounts. Linking them would need the provider's email address; it is left
  out.

### Data

- Beelot now stores the email address of email-link players, in a table of
  its own linked to the `account` row. It is used only to send sign-in links
  and account notices. It is never shown to other players, logged or sent
  elsewhere.
- Players keep a random pseudonym (US-075) until they choose a display name
  (US-072).
- Deleting the account deletes the address (US-081); the export includes it
  (US-082).

### Sessions

- The 15-minute inactivity timeout came from PCI DSS 8.2.8, which does not
  apply to Beelot (no payment cards). With email links, it would send players
  back to their inbox after every break. The session ends after **7 days
  without a request**, and lasts **at most 30 days**.
- A sign-in changes the session id; signing out ends the session.

### Security baseline

- Without passwords, PCI DSS requirement 8 is no longer the baseline. The
  baseline is NIST SP 800-63B at assurance level 1, plus the shared rules of
  phase 5: HTTPS with HSTS, `Secure`, `HttpOnly`, `SameSite=Lax` cookies, and
  no token in logs or in URLs sent to third parties.
- An email link is as strong as the player's mailbox. That fits a card game
  that holds no payment data, and Google and GitHub offer their own second
  factors.

## Consequences

- **Removed:** Zitadel in `compose.yaml`, `zitadel/provision.sh`, the
  `BEELOT_ZITADEL_*` variables, `ZitadelUsers`, `ZitadelProperties`,
  `ZitadelLogoutSuccessHandler`, `RegisterAuthorizationRequestResolver`, and
  the unconfirmed-account cleanup job. The pseudonyms, the `account` table and
  the match history stay.
- **Added:** an SMTP sender (Spring Boot mail) with its variables, the
  token store, the link email in three languages, the sign-in form, the rate
  limits, and the Google and GitHub registrations from environment variables.
- **Phase 5 stories:**
  - **US-075** becomes "Create an account with my email address": request a
    link; the first link opened creates the account.
  - **US-076** (confirm my email) merges into US-075: the link confirms the
    address.
  - **US-077** becomes "Sign in with an email link, Google or GitHub": the
    link rules, the rate limits, the session limits, sign-out and the last
    sign-in time. The lock and unlock are dropped: there is no password to
    guess, and a link cannot be guessed within its lifetime.
  - **US-078** (reset a forgotten password) is dropped.
  - **US-079** becomes "Change my email address": the new address is used
    once a link sent to it is opened, and the old address is notified.
  - **US-080** (second factor) is dropped for now. Passkeys can come back as
    a separate story.
  - **US-081 to US-083** stay. The privacy policy lists the hosting and the
    SMTP provider as processors, and no longer Zitadel.
- **Privacy:** one processor fewer and no event store to erase, but Beelot
  now holds email addresses and is responsible for protecting them.
- **Operations:** no identity service to run, no quota and no uptime
  dependency for sign-in other than the SMTP provider. If the SMTP provider is
  down, email players cannot sign in; Google, GitHub and guest play still
  work.
- Production needs an SMTP provider. Its choice is still open (see the
  backlog decisions).

## Sources

- Spring Security, [One-Time Token Login](https://docs.spring.io/spring-security/reference/servlet/authentication/onetimetoken.html).
- NIST SP 800-63B, [Digital Identity Guidelines: Authentication](https://pages.nist.gov/800-63-4/sp800-63b.html).
- ADR-003 for the comparison of identity providers.
