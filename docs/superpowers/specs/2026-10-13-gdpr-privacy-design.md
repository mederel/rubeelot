# Delete my account, download my data, know how my data is handled (US-081, US-082, US-083) — design

## Goal

Players control the personal data Beelot keeps about them, as the GDPR
requires, before the leaderboard (US-069) shows their names to everyone:

- US-083: a privacy policy says what is kept, why, for how long and by whom,
  and Spring enforces those retention periods and records security events.
- US-081: a signed-in player deletes their account (article 17).
- US-082: a signed-in player downloads their data as JSON (articles 15
  and 20).

The three stories share one design because the security events of US-083 are
the sign-in history of the export, and the account deletion of US-081 is also
what the retention job runs on inactive accounts. They are built in the order
US-083, US-081, US-082.

## Acceptance criteria and how they are met

| Criterion | Met by |
| --- | --- |
| Settings offers "Delete my account"; it asks the player to sign in again and to confirm | "Your data" panel, recent sign-in rule, typed confirmation |
| The account, email address, pending tokens and sessions are deleted | `AccountDeletionService`, `AccountSessions` |
| Finished matches stay; the player's seats show "Deleted player" with no account | `AccountDeletionService` |
| Backups are overwritten within the stated retention period | Documented; `BEELOT_BACKUP_RETENTION_DAYS` on the policy |
| Settings offers "Download my data" as JSON: account details, sign-in history, matches | `DataExportService`, `GET /api/account/export` |
| The export is for the signed-in owner only, once per hour | Session-bound endpoint, `EXPORT` event check |
| A privacy policy page, linked from the home screen and the sign-in form | `/privacy` view |
| The policy names the controller, contact, data, purposes, legal bases, processors, retention, rights, CNIL | Policy text in English, French and Dutch, values from env vars |
| DPAs are signed with the hosting and SMTP providers | Documented in the README (not code) |
| Retention enforced by a scheduled job: tokens 1 day, inactive accounts 3 years with a warning, events 12 months | Existing token purge; `RetentionJob` |
| Spring records security events without tokens, with account, time, IP and outcome | `SecurityEvents`, `security_event` table |
| Only the session and CSRF cookies; listed in the policy | Policy "Cookies" section |
| The data breach procedure is documented in `docs/` | `docs/privacy/data-breach-procedure.md` |

## Decisions

- **Signing in again means a recent sign-in.** Deletion is allowed only when
  the session's last successful sign-in (email link, Google or GitHub) is less
  than 10 minutes old. Otherwise the server answers
  `403 {"reason":"reauthenticate"}`, and the client sends the player through
  their usual sign-in and back to the confirmation step. The rule is the same
  for every sign-in method.
- **Live matches go to bots.** On deletion, a seat the player holds in a
  match being played switches to `BOT_TAKEOVER` and is renamed "Deleted
  player", so the others can finish; the player leaves any table that has not
  started. Solo bot games (`BotGameService`) end with the session.
- **"Email change" is not an event.** Beelot cannot change an email address;
  the backlog wording is corrected.
- **Our own events table.** Spring Security's events and our own calls feed
  one `security_event` table, which the export reads and the retention job
  purges. Actuator's audit store and logs-only were rejected: the first is a
  free-form map in memory, the second cannot feed the export.
- **The deletion event is anonymous.** `DELETION` is recorded with no account
  id after the account is gone: it proves that a deletion happened, not who
  was deleted.
- **Recording never blocks.** A failed event insert is logged and the request
  carries on.
- **The policy is in three languages.** English, French and Dutch through the
  existing client i18n; French is the reference text. Operator-specific values
  come from environment variables, not from the text.
- **DPAs and backups are documented, not coded.**

## Data

`V5__privacy.sql`:

```sql
-- Security events (US-083): never a token. account_id has no foreign key, so
-- the history never blocks a deletion; an account's events go with it.
CREATE TABLE security_event (
    id          BIGINT      NOT NULL AUTO_INCREMENT PRIMARY KEY,
    account_id  UUID,
    type        VARCHAR(32) NOT NULL,
    outcome     VARCHAR(16) NOT NULL,
    ip_address  VARCHAR(45),
    occurred_at DATETIME(6) NOT NULL
);
CREATE INDEX security_event_account_idx ON security_event (account_id, occurred_at);
CREATE INDEX security_event_occurred_idx ON security_event (occurred_at);

-- When the inactivity warning was sent (US-083); a sign-in clears it.
ALTER TABLE account ADD COLUMN deletion_warned_at DATETIME(6);
```

Event types: `LINK_REQUESTED`, `SIGN_IN`, `SIGN_OUT`, `SESSION_EXPIRED`,
`RATE_LIMITED`, `EXPORT`, `DELETION`. Outcomes: `SUCCESS`, `FAILURE`,
`REFUSED`. A link request has no account id (the form never says whether an
address has an account). `account.last_sign_in_at` already exists and drives
the 3-year rule.

## Components

All new classes live in `fr.beelot.application.privacy` unless stated.

- **`SecurityEvents`** — `record(type, outcome, accountId, request)` and
  listeners for Spring Security's `AuthenticationSuccessEvent`,
  `AbstractAuthenticationFailureEvent`, `LogoutSuccessEvent` and
  `HttpSessionDestroyedEvent` (expiry only, not sign-out or deletion).
  `SignInRequestFilter` calls it for link requests and rate-limit refusals.
  The IP is `request.getRemoteAddr()`, as used by the rate limiter.
- **`AccountSessions`** — account id → live `HttpSession`s, filled when a
  sign-in succeeds and emptied by `HttpSessionDestroyedEvent`. The same
  success hook stores the sign-in time in the session (`beelot.signedInAt`)
  and clears `deletion_warned_at`. `invalidateAll(accountId)` ends every
  session of the account.
- **`AccountDeletionService.delete(accountId)`** — one transaction:
  1. `match_seat`: `account_id = NULL`, `name = 'Deleted player'` for the
     account's seats;
  2. delete the account's `security_event` rows;
  3. delete `sign_in_token` rows for its email address, then `account_email`,
     then `account`.

  After commit: `PrivateTableService` gives the player's live seats to bots
  and frees seats at waiting tables; `AccountSessions.invalidateAll`; record an
  anonymous `DELETION` event. A failure after commit is logged and does not
  undo the deletion.
- **`AccountService.current()`** (existing) treats a session whose account no
  longer exists as signed out and invalidates it, as a safety net for a
  missed session.
- **`DataExportService.export(accountId)`** — builds the export below.
  Allowed when the account has no `EXPORT` event in the last hour; otherwise
  it reports the minutes left. Each export records an `EXPORT` event.
- **`RetentionJob`** — `@Scheduled` nightly (`beelot.privacy.retention-cron`,
  default `0 30 3 * * *`), with an injected `Clock`. Steps 2 and 3 handle
  each account in its own transaction:
  1. delete `security_event` rows older than 12 months;
  2. warn email-link accounts whose `last_sign_in_at` is older than 3 years
     minus 30 days and `deletion_warned_at` is null: send the email in English, French and
     Dutch in one message (no language is stored per account), then set `deletion_warned_at`. A send failure leaves it null so
     the next night retries;
  3. delete, through `AccountDeletionService`, accounts whose
     `last_sign_in_at` is older than 3 years and that either have no email
     address or were warned at least 30 days ago.

  The hourly expired-token purge in `SignInTokenService` stays as it is.
- **`PrivacyController`**:
  - `GET /api/privacy/policy-info` (public) → `{controller, contact,
    hostingProvider, smtpProvider, backupRetentionDays}`;
  - `GET /api/account/export` → `application/json` attachment
    `beelot-data-YYYY-MM-DD.json`; 401 if signed out, 429 with
    `Retry-After` past the hourly limit;
  - `DELETE /api/account` → 204; 401 if signed out, 403 `reauthenticate`
    when the sign-in is 10 minutes old or more; CSRF required.

### Configuration

| Property | Env var | Default |
| --- | --- | --- |
| `beelot.privacy.controller` | `BEELOT_PRIVACY_CONTROLLER` | none |
| `beelot.privacy.contact` | `BEELOT_PRIVACY_CONTACT` | none |
| `beelot.privacy.hosting-provider` | `BEELOT_HOSTING_PROVIDER` | none |
| `beelot.privacy.smtp-provider` | `BEELOT_SMTP_PROVIDER` | none |
| `beelot.privacy.backup-retention-days` | `BEELOT_BACKUP_RETENTION_DAYS` | none |

`application.properties` binds each property to its env var
(`${BEELOT_PRIVACY_CONTROLLER:}`), so `.env` and real environment variables
both work. An unset value is logged as a warning at startup and shown as "not
configured" on the policy.

## Export format (version 1)

```json
{
  "format": "beelot-export-1",
  "exportedAt": "2026-10-13T10:00:00Z",
  "account": {
    "id": "…",
    "displayName": "…",
    "createdAt": "…",
    "lastSignInAt": "…",
    "signIn": { "method": "email", "email": "…" }
  },
  "securityEvents": [
    { "type": "SIGN_IN", "outcome": "SUCCESS", "ipAddress": "…", "occurredAt": "…" }
  ],
  "matches": [
    {
      "endedAt": "…", "mode": "…", "variant": "…", "difficulty": "…",
      "yourTeam": "NORTH_SOUTH", "northSouthScore": 1012, "eastWestScore": 640, "won": true,
      "seats": [ { "position": 0, "name": "…", "team": "NORTH_SOUTH", "bot": false } ],
      "rounds": [ { "declaringTeam": "…", "trump": "…", "contractValue": null, "coinched": false,
                    "contractMade": true, "capotTeam": null, "beloteTeam": null,
                    "northSouthPoints": 92, "eastWestPoints": 70 } ]
    }
  ]
}
```

For Google and GitHub, `signIn` is `{ "method": "google" }` or
`{ "method": "github" }`; the provider subject is not exported. Other
players' names appear as in the match history; account ids never do. The
records get native reflection hints.

## Client

- **Settings → "Your data"** (signed in only):
  - "Download my data" links to `/api/account/export`; a 429 shows "You can
    download your data again in N minutes".
  - "Delete my account" opens a confirmation that says what is deleted and
    that past matches stay as "Deleted player". The player types the
    translated word `DELETE`. On `reauthenticate` the page explains why,
    starts the usual sign-in, and the return lands on
    `/settings?confirm-delete`. After deletion: home page, guest,
    "Your account has been deleted".
- **`/privacy` view** — added to `HomeController`'s routes; linked from the
  home screen footer and under the sign-in form ("How we handle your data").
  Sections: who is responsible; data stored, purposes and legal bases
  (contract for the account and matches, legitimate interest for security
  events); processors (hosting, SMTP, Google and GitHub for their players);
  retention (tokens 1 day, inactive accounts 3 years with a 30-day warning,
  security events 12 months, backups N days); cookies (session and CSRF only,
  strictly necessary); rights (access, portability, erasure through Settings;
  rectification and objection through the contact) and the right to complain
  to the CNIL. Values are filled from `/api/privacy/policy-info`.
- All text goes through `i18n.js` (French) and `i18n-nl.js` (Dutch), English
  as the key, like the rest of the app.

## Docs

- `docs/privacy/data-breach-procedure.md`: who decides (the controller), the
  assessment, notifying the CNIL within 72 hours, informing players when the
  risk is high, the breach log.
- README: the privacy env vars, the DPAs to sign, backup retention.
- Backlog: US-081 to US-083 marked delivered with a link to this spec;
  "email change" removed from the event list.

## Errors

- Deletion transaction fails: nothing is deleted; the player sees an error.
- Bot takeover or session invalidation fails after commit: logged; the
  `current()` check still signs the session out.
- Retention job: one account's failure does not stop the others; a failed
  warning email is retried the next night, and deletion counts 30 days from
  the warning actually sent.
- Event recording fails: logged, the request carries on.

## Testing

Testcontainers MariaDB, as for US-075.

- `AccountDeletionServiceTest`: rows gone; seats renamed with no account;
  other players' matches kept; live seat taken by a bot; sessions invalidated.
- `PrivacyControllerTest`: reauthenticate under and over 10 minutes; CSRF;
  export shape and no token in it; hourly limit; owner only; policy info.
- `RetentionJobTest` with a fixed `Clock`: warning at 3 years minus 30 days,
  deletion 30 days after, immediate deletion without an email address, events
  purged after 12 months, a sign-in clears the warning, a failed email leaves
  the account unwarned.
- `SecurityEventsTest`: each type recorded with the IP and no token.
- Client: the French and Dutch translation UI tests cover the new keys; a
  headless browser run through settings, export, delete and the privacy page.
- A native build smoke test for the export records.
