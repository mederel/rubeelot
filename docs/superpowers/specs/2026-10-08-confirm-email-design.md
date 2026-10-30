# Confirm my email address (US-076) — design

## Goal

As a new player, I want to confirm my email address, so that the game knows
the address is mine and can reach me to reset my password.

US-075 already makes Zitadel send a confirmation code at registration and
refuse to finish signing in until the address is confirmed
(`EMAIL_VERIFICATION=true` on the login container). This design adds the
24-hour expiry of that code, and the scheduled job that deletes accounts left
unconfirmed for 7 days. It is Beelot's first use of Zitadel's API from Spring;
the unlock of US-077 and the retention jobs of US-083 will reuse the client.

## Acceptance criteria and how they are met

| Criterion | Met by |
| --- | --- |
| Zitadel emails a confirmation link or code after registration | Zitadel (US-075), checked end to end |
| It works once | Zitadel: a verified code cannot be reused, checked end to end |
| It expires after 24 hours | `provision.sh` sets the email verification code generator to `86400s` |
| An expired link can be replaced from Zitadel's pages | Zitadel's "resend code" on the verify page, checked end to end |
| Signing in to an unconfirmed account asks to confirm first | Zitadel (US-075, `EMAIL_VERIFICATION=true`), checked end to end |
| A Spring job deletes accounts unconfirmed for 7 days, through Zitadel's API | `UnconfirmedAccountCleanup` and `ZitadelUsers` (below) |

## Decisions

- **Which users the job deletes.** A Zitadel user is deleted only when all
  three hold:
  1. it is a human user whose email address is not verified;
  2. it was created at least 7 days ago;
  3. Beelot has no `account` row for it (`provider = 'zitadel'`,
     `provider_subject` = the user id), so it has never signed in to Beelot;
  4. it holds no role in Zitadel (no membership, checked with
     `POST /management/v1/users/{id}/memberships/_search`).

  The third condition protects every player who has played, including a
  Google or GitHub user whose address Zitadel might mark as unverified. The
  fourth protects the Zitadel administrators, who never sign in to Beelot and
  so have no `account` row; if the check fails, the user is kept. Machine
  users are excluded by the type filter.
- **Spring calls Zitadel's REST API with `RestClient`.** No SDK and no new
  dependency: the job needs two calls. Small records map the JSON responses,
  registered for reflection in the native image.
- **Zitadel cannot filter on "email verified".** `POST /v2/users` has no such
  query, so the job lists human users sorted by creation date, oldest first,
  stops at the first user created after the cutoff, and filters the rest
  itself.
- **The job never keeps a name or an email address.** The list response
  contains them, but the records map only the user id, the creation date and
  the verified flag; the other fields are dropped while parsing. Logs show
  counts and user ids only.
- **A dedicated service account with the least privilege.** `provision.sh`
  creates the machine user `beelot-jobs` with the `ORG_USER_MANAGER` role on
  the organization and a personal access token. Spring reads the token from
  `BEELOT_ZITADEL_API_TOKEN`. It is not `beelot-setup`, which is an IAM owner.
- **The job is off without its token.** Like sign-in, which is offered only
  once the issuer and client are set, the job runs only when the issuer and
  the API token are both set. The bean always exists and checks this at each
  run, because the native image fixes conditional beans at build time, not
  from runtime variables (the sign-in client of US-075 is decided the same
  way). Development without Zitadel and `./gradlew test` are unaffected.
- **One instance.** Beelot runs as a single instance, so the job needs no
  distributed lock. Deleting the same user twice is harmless anyway (404 is
  treated as done).

## Components

### `ZitadelProperties` (changed)

Gains `apiToken` (`beelot.zitadel.api-token=${BEELOT_ZITADEL_API_TOKEN:}`)
and `apiConfigured()`: issuer and API token set. `configured()` (sign-in)
is unchanged.

### `ZitadelUsers` (new, `fr.beelot.application.security`)

A thin client of Zitadel's user API, built from a `RestClient.Builder` with
the issuer as base URL and `Authorization: Bearer <api token>`.

- `List<String> unconfirmedCreatedBefore(Instant cutoff)`: pages through
  `POST /v2/users` with
  `{"query": {"offset": n, "limit": 100, "asc": true},
    "sortingColumn": "USER_FIELD_NAME_CREATION_DATE",
    "queries": [{"typeQuery": {"type": "TYPE_HUMAN"}}]}`.
  It returns the ids of users with `human.email.isVerified` false and
  `details.creationDate` before the cutoff, and stops reading pages at the
  first user created at or after the cutoff, or at the last page.
- `void delete(String userId)`: `DELETE /v2/users/{userId}`. A 404 means the
  user is already gone and is not an error.
- Any other error response or I/O failure is thrown as
  `ZitadelApiException`, whose message holds the method, the path and the
  status, never the token or the response body.

### `UnconfirmedAccountCleanup` (new, `fr.beelot.application.account`)

A Spring component. Each run first checks `apiConfigured()` and does
nothing, without calling Zitadel, when it is false.

- `@Scheduled(fixedDelayString = "${beelot.account.cleanup-interval:PT1H}",
  initialDelayString = "PT1M")` runs `cleanUp()`.
- `cleanUp()` computes `cutoff = clock.instant() - retention`
  (`beelot.account.unconfirmed-retention`, default `P7D`), asks
  `ZitadelUsers` for candidates, drops those with an `account` row (one
  `AccountRepository` query for the whole batch:
  `findProviderSubjectsIn("zitadel", ids)`), and deletes the rest one by one.
- Logging, at INFO: the number deleted when it is not zero. At WARN: a
  failed delete (user id and status), then the job continues with the next
  user; a failed listing, then the run stops and the next run retries.
- It takes a `Clock`. No `Clock` bean exists yet: the application adds
  `Clock.systemUTC()` as one, so tests can set the time.

### `zitadel/provision.sh` (changed)

- Sets the email verification code: `PUT
  /admin/v1/secretgenerators/SECRET_GENERATOR_TYPE_VERIFY_EMAIL_CODE` with
  `expiry: "86400s"`, keeping Zitadel's length and character settings.
- Creates the machine user `beelot-jobs` if missing, grants it
  `ORG_USER_MANAGER` on its organization, and creates a personal access
  token only when `.env` has none for it. Zitadel shows a token only once,
  so an existing `BEELOT_ZITADEL_API_TOKEN` is kept, as the client secret is.
- Writes `BEELOT_ZITADEL_API_TOKEN` to `.env` with the other
  `BEELOT_ZITADEL_` variables.

### Configuration

`application.properties`:

```properties
beelot.zitadel.api-token=${BEELOT_ZITADEL_API_TOKEN:}
# Zitadel users who never confirmed their address and never signed in are deleted after this time (US-076).
beelot.account.unconfirmed-retention=P7D
beelot.account.cleanup-interval=PT1H
```

The README documents `BEELOT_ZITADEL_API_TOKEN`, the `beelot-jobs` service
account it belongs to, and the job.

## Error handling

| Situation | Behaviour |
| --- | --- |
| Token or issuer missing | Each run does nothing; sign-in works as before |
| Zitadel unreachable or answers 5xx while listing | WARN, run stops, next run retries |
| Token rejected (401/403) | WARN with the status, run stops; every run warns until fixed |
| Delete answers 404 | Treated as deleted |
| Delete fails otherwise | WARN with the user id, continue with the next user |
| Database unavailable | The exception ends the run; Spring's scheduler logs it, next run retries |

## Testing

- `ZitadelUsersTest`, with `MockRestServiceServer`:
  - the request body, the bearer token and the paging;
  - it stops at the first user created after the cutoff;
  - verified users are not returned;
  - delete sends the right request; a 404 is accepted;
  - a 500 or 401 throws `ZitadelApiException` without the token in its
    message.
- `UnconfirmedAccountCleanupTest`, with a fake `ZitadelUsers`, a fixed
  `Clock` and MariaDB in Testcontainers:
  - a candidate with an `account` row is not deleted;
  - the others are deleted;
  - a failed delete does not stop the next ones;
  - a failed listing deletes nothing.
- Without `BEELOT_ZITADEL_API_TOKEN`, a run makes no call to Zitadel.
- End to end, in a headless browser against `docker compose`:
  1. register; sign-in stops at the confirmation page;
  2. "resend code" sends a new code to Mailpit, and the new code confirms
     the address; the previous code and the used one are then refused;
  3. the code generator shows an expiry of 24 hours (`GET
     /admin/v1/secretgenerators/...`);
  4. with `beelot.account.unconfirmed-retention=PT1M` and
     `beelot.account.cleanup-interval=PT30S`, an unconfirmed user is deleted
     from Zitadel, and a confirmed player who signed in is kept.
- `./gradlew test` and the native build stay green.

## Out of scope

- Unlocking accounts (US-077) and the retention jobs of US-083. They will
  reuse `ZitadelUsers`.
- Emailing the player before deleting an unconfirmed account: the backlog
  does not ask for it, and Beelot does not hold the address.
- Recording a security event for the deletion: Spring's security events
  arrive with US-083.
