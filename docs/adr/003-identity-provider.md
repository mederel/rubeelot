# ADR-003: Choose the identity provider for email accounts

- Status: Accepted: local storage, with sign-in by email link, Google or
  GitHub as set by ADR-004 (first proposed Zitadel Cloud); sign-in is by
  passkey only since ADR-005
- Date: 2026-10-07, revised 2026-10-14
- Decision makers: Product and engineering

## Context

Today players sign in only with Google or GitHub (US-057). Spring Security's
OAuth2 client creates a server-side session, and the `account` table in
MariaDB stores the provider, the provider's user id and a display name. The
application stores no password and sends no email.

Phase 5 of the roadmap (US-075 to US-083) adds accounts created with an email
address and a password. These accounts need email confirmation, password
reset, an optional second factor, deletion and data export. The stories
follow the GDPR (RGPD), and they apply the authentication controls of PCI DSS
v4.0 requirement 8 as a baseline. Beelot processes no payment cards, so PCI
DSS does not formally apply. The controls that weigh most in this choice are:

- passwords of at least 12 characters, checked against breached passwords;
- a 30-minute lock after 10 failed attempts;
- a 15-minute inactivity timeout and a 30-day maximum session;
- email links that work once: confirmation valid 24 hours, reset valid 1 hour;
- no reuse of the last 4 passwords;
- an optional second factor with recovery codes;
- deletion, export, and 12 months of security events.

The project has no budget for paid plans yet, and traffic is low and
irregular. Three options were compared, with their free plans as of
2026-10-07:

1. **Local storage:** Spring Security with accounts in MariaDB.
2. **Supabase Auth, Free plan.**
3. **Zitadel Cloud, Free plan**, with self-hosting as a fallback.

## Options

### 1. Local storage (Spring Security + MariaDB)

Spring Security handles registration, sign-in and reset. Password hashes,
tokens and second-factor secrets live in MariaDB, and Spring sends the emails
through SMTP.

- Every criterion can be met exactly as written: Argon2id or bcrypt, the
  breached-password check, the lock with automatic unlock, password history,
  separate expiries for each kind of link, and recovery codes.
- No external service, no quota and no inactivity clause. Personal data stays
  in our database, so the GDPR analysis has one processor fewer.
- **We build and maintain every security-sensitive flow:** registration,
  confirmation, reset, email change, second factor, the lock and the sign-in
  pages. That is the largest amount of code of the three options, all of it
  security-critical, and all of it must be translated into French and Dutch.
- **A database leak exposes password hashes.** The liability and the security
  reviews are ours.
- Argon2id needs Bouncy Castle, which must be checked in the native image
  (US-031). bcrypt (cost of 12 or more) is the fallback.

### 2. Supabase Auth, Free plan

Supabase provides email and password accounts, confirmation, reset, email
change, authenticator-app codes and security notification emails. It runs on
its own PostgreSQL project.

- **The project is paused after 1 week of inactivity.** While paused, nobody
  can sign in. For a game with irregular traffic, this risk alone rules out
  the free plan for production.
- **No backups** on the free plan, community support only, and no uptime
  guarantee.
- Missing on the free plan: the breached-password check, the inactivity
  timeout and the maximum session (Pro), and the lock after failed attempts
  (Team, through the password verification hook).
- Missing on every plan: password history and recovery codes.
- One expiry setting covers every email link, so a 24-hour confirmation link
  and a 1-hour reset link can't coexist.
- Passwords are limited to 72 characters, because they are hashed with
  bcrypt.
- The OpenID Connect provider ("OAuth 2.1 Server") is in public beta. A
  stable integration means Spring calls the Auth API and so handles
  passwords. Spring then has to add the lock, the breached-password check and
  the session limits itself, and those are most of the hard parts of option 1.
- A data processing agreement is available on every plan, and specific EU
  regions exist. Supabase is a US company.

### 3. Zitadel Cloud, Free plan

Zitadel is an OpenID Connect provider with hosted pages for sign-in,
registration, email verification, password reset and the second factor.

- **Its pricing and its cloud service description mention no pausing of
  inactive free instances.** The Terms of Service must confirm this.
- The free plan includes "all security features":
  - password complexity rules;
  - a lock after failed password and one-time-code attempts;
  - a separate expiry for each kind of email link;
  - authenticator apps, email and SMS codes, and passkeys;
  - hiding whether an account exists (the "ignore unknown usernames" setting);
  - custom SMTP, and privacy and terms links on the registration page.
- **Spring never sees a password.** It signs players in through the standard
  OpenID Connect flow with Spring Security's OAuth2 client, as it already does
  for Google and GitHub. Google and GitHub can move behind Zitadel (the free
  plan allows 3 identity providers).
- Limits of the free plan:
  - **100 daily active users**. What happens above that is not documented.
    Pro costs US$100 per month.
  - no custom domain: the sign-in pages are served from `*.zitadel.cloud`;
  - no uptime guarantee.
- Missing:
  - **automatic unlock**: only an administrator can unlock an account (GitHub
    issue #6502);
  - a breached-password check;
  - password history;
  - mature recovery codes (an API exists, but users report problems).
- Zitadel is a Swiss company, with EU and Swiss data regions. Its data
  processing agreement allows US subprocessors.
- Zitadel stores every change as an event. We must check that deleting a user
  erases the personal data in earlier events, as GDPR article 17 requires.
- Zitadel is open source (AGPL-3.0). It can be self-hosted with PostgreSQL
  in Docker Compose if the free plan stops fitting. Because Spring only speaks
  OpenID Connect, the move is a configuration change.

## Comparison

| Criterion | Local storage | Supabase Free | Zitadel Free |
| --- | --- | --- | --- |
| Inactivity clause | None | **Paused after 1 week** | None found (to confirm) |
| Quota | None | 50,000 monthly users | **100 daily users** |
| Passwords handled by Spring | Yes | Yes | **No** |
| Lock after failed attempts | Build | ✗ (Team plan) | ✅, manual unlock |
| Breached-password check | Build | ✗ (Pro plan) | ✗ |
| Inactivity timeout and maximum session | Build (Spring session) | Build (Pro in Supabase) | Build (Spring session) |
| Separate link expiries | Build | ✗ | ✅ |
| Password history | Build | ✗ | ✗ |
| Second factor | Build | Authenticator app | Authenticator app, passkeys, email and SMS codes |
| Recovery codes | Build | ✗ | Immature |
| Hosted, translated sign-in pages | Build | Build | ✅ (French and Dutch to check) |
| Backups | Ours (MariaDB) | ✗ | Provider's |
| Uptime guarantee | Ours | ✗ | ✗ (Pro) |
| Code to write and secure | Most | Medium | Least |

## Decision

**Store accounts locally, in Beelot's own MariaDB database, with Spring
Security (option 1).** No external identity provider holds Beelot's players.

This ADR first proposed Zitadel Cloud, and US-075 and US-076 were first built
on it. ADR-004 records why Zitadel was dropped, and it changes how option 1
signs players in: there are no passwords. Players sign in with a one-time
email link (Spring Security's one-time-token login) or with Google or GitHub
as direct OAuth providers. Most of the cost that weighed against local
storage came from passwords, so it goes away with them.

- The accounts live in tables migrated by Flyway:
  - `account`: one row per player, with the provider, the provider's user
    id, the display name and the sign-in dates;
  - `account_email`: the address of email-link players, used only to send
    sign-in links and account notices;
  - `sign_in_token`: pending sign-in links, stored only as a hash.
- Beelot asks Google and GitHub for the user id only, and keeps no name and
  no email address from them.
- Spring keeps the controls of the application session: the inactivity
  timeout, the maximum session length and the last sign-in time (values in
  ADR-004).
- Deletion and export (US-081, US-082) apply to these tables, with no
  external store to erase.

Rejected:

- **Zitadel Cloud**, the original proposal. Spring had to compensate for
  what it lacks (a job to unlock accounts, a job to delete unconfirmed
  accounts, a shorter Zitadel session). It is one more service to run, or a
  free plan capped at 100 daily active users, without an uptime guarantee and
  without a custom domain. It is one more processor, with US subprocessors
  and an event store whose erasure is unverified. Its checks before
  acceptance were never completed and are dropped.
- **Supabase's free plan**, for its weekly pausing, its lack of backups, and
  its missing security controls.

## Consequences

- No identity service to run, no quota, and no dependency for sign-in other
  than the SMTP provider.
- Personal data stays in our database, so the GDPR analysis has one
  processor fewer. The privacy policy (US-083) does not list Zitadel, and no
  data processing agreement with Zitadel is needed.
- The liability is ours: Beelot holds email addresses and must protect them,
  and we build and review the sign-in flows, the rate limits and the session
  limits. Backups of the accounts are ours (MariaDB).
- Beelot stores no password and no password hash (ADR-004). If passwords
  ever come back, the password-hash choice (Argon2id or bcrypt, and the
  native image check of US-031) must be made then.
- Zitadel and its configuration are removed from the project, and the
  development accounts created through it are deleted by a migration. The
  phase 5 story changes are listed in ADR-004.

## Sources

- Supabase: [pricing](https://supabase.com/pricing),
  [password security](https://supabase.com/docs/guides/auth/password-security),
  [sessions](https://supabase.com/docs/guides/auth/sessions),
  [rate limits](https://supabase.com/docs/guides/auth/rate-limits),
  [auth hooks](https://supabase.com/docs/guides/auth/auth-hooks),
  [email link expiry](https://supabase.com/docs/guides/auth/auth-email-passwordless),
  [OAuth 2.1 server beta](https://supabase.com/changelog/38022-oauth-2-1-server-capabilities-for-supabase-auth),
  [GDPR](https://supabase.com/docs/guides/security/gdpr-compliance).
- Zitadel: [pricing](https://zitadel.com/pricing),
  [default settings](https://zitadel.com/docs/guides/manage/console/default-settings),
  [cloud service description](https://zitadel.com/docs/legal/service-description/cloud-service-description),
  [data processing agreement](https://zitadel.com/docs/legal/data-processing-agreement),
  [recovery codes discussion](https://questions.zitadel.com/m/1293859410367741974).
