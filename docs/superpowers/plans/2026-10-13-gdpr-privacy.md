# GDPR: delete, export, privacy policy (US-081 to US-083) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Players can read the privacy policy, download their data and delete their account; Spring records security events and enforces retention periods.

**Architecture:** A new package `fr.beelot.application.privacy` holds the security event log, the account session registry, deletion, export, retention and the privacy endpoints. It reads and writes other tables through `JdbcClient`, so it needs no package-private repository of `account` or `history`; `account` calls `privacy` (events, sessions), never the reverse through repositories. The client gets a `/privacy` view and a "Your data" panel in Settings.

**Tech Stack:** Spring Boot 4.1.1 / Spring Security 7.1, Spring Data JPA + `JdbcClient`, Flyway, MariaDB 11.4 (Testcontainers), vanilla JS client with `i18n.js` (French) and `i18n-nl.js` (Dutch).

**Spec:** `docs/superpowers/specs/2026-10-13-gdpr-privacy-design.md`

## Global Constraints

- Never store a token, a token hash or a provider subject in `security_event` or in the export.
- Recording an event never blocks the action: catch, log at WARN, carry on.
- Event types: `LINK_REQUESTED`, `SIGN_IN`, `SIGN_OUT`, `SESSION_EXPIRED`, `RATE_LIMITED`, `EXPORT`, `DELETION`. Outcomes: `SUCCESS`, `FAILURE`, `REFUSED`.
- Recent sign-in for deletion: strictly less than 10 minutes.
- Deleted seats are named exactly `Deleted player`.
- Retention: events 12 months; inactive accounts 3 years; warning 30 days before; tokens 1 day (existing purge, unchanged).
- Export: at most one per account per hour; file name `beelot-data-YYYY-MM-DD.json`; `"format": "beelot-export-1"`.
- Env vars: `BEELOT_PRIVACY_CONTROLLER`, `BEELOT_PRIVACY_CONTACT`, `BEELOT_HOSTING_PROVIDER`, `BEELOT_SMTP_PROVIDER`, `BEELOT_BACKUP_RETENTION_DAYS`.
- All new UI text uses English as the i18n key, with French in `i18n.js` and Dutch in `i18n-nl.js`. French is the reference text of the policy.
- Times are UTC; inject `Clock` (the `Clock` bean in `SecurityConfiguration`) wherever "now" matters.
- Commit messages follow the repo: `feat(privacy): … (US-08x)`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Run the full suite with `./gradlew test` before each commit; `PrivateTableServiceTest` and `BotGameServiceTest` are known to be flaky — rerun once before treating them as failures.

## Review Focus

- A player deleted while another of their sessions is open on a second device: that session must be signed out (Task 5, `deletionEndsEverySession`).
- A solo bot game or table match that finishes after the account was deleted: recording must not fail on the missing account and must name the seat `Deleted player` (Task 5, `matchFinishedAfterDeletionIsRecordedAnonymously`).
- A sign-in exactly 10 minutes old: refused (Task 5, `signInTenMinutesOldMustReauthenticate`).
- The export must not leak another player's account id or any token (Task 7, `exportHoldsNoAccountIdsOfOthersAndNoTokens`).
- A warning email that fails to send must not start the 30-day clock (Task 6, `failedWarningIsRetried`).

## Rulings made while planning

- Sign-in and sign-in failure events are recorded in the success and failure handlers, not through Spring's `AuthenticationSuccessEvent` listeners: the handlers know the account id and fire for both OTT and OAuth without configuring an `AuthenticationEventPublisher`. The spec's intent (one table, every listed event) is unchanged.
- Solo bot games keep running after deletion; `MatchHistoryService` stores a seat whose account no longer exists as `Deleted player` with no account. This covers games that outlive the session without tracking them.

---

### Task 1: Security events

**Files:**
- Create: `src/main/resources/db/migration/V5__privacy.sql`
- Create: `application/privacy/SecurityEvents`
- Modify: `application/account/SignInRequestFilter`, `SignInSuccessHandler`
- Modify: `application/security/SecurityConfiguration`
- Test: `tests/application/privacy/SecurityEventsIntegrationTest`

**Interfaces:**
- Produces:
  - `enum SecurityEvents.Type { LINK_REQUESTED, SIGN_IN, SIGN_OUT, SESSION_EXPIRED, RATE_LIMITED, EXPORT, DELETION }`
  - `enum SecurityEvents.Outcome { SUCCESS, FAILURE, REFUSED }`
  - `public void record(Type type, Outcome outcome, UUID accountIdOrNull, String ipOrNull)` — never throws.
  - `public static final String ACCOUNT_ID = "beelot.accountId"` — the session attribute holding the signed-in account's `UUID`; also used by Tasks 2 and 5.
  - `public static final String ENDED = "beelot.ended"` — session attribute set to `true` before Beelot itself invalidates a session (sign-out, deletion), so that destruction is not counted as expiry.

- [ ] **Step 1: Write `V5__privacy.sql`** exactly as the spec's "Data" section (the `security_event` table, both indexes, and `ALTER TABLE account ADD COLUMN deletion_warned_at DATETIME(6);`).

- [ ] **Step 2: Write the failing tests** in `SecurityEventsIntegrationTest` (`@SpringBootTest(properties = "beelot.sign-in.limit-per-ip=1000")`, `@AutoConfigureMockMvc`, Testcontainers MariaDB `mariadb:11.4`, `@MockitoBean MailSender`, copying the link-capture helpers from `EmailSignInIntegrationTest`). Read rows with `JdbcClient` from `security_event`.
  - `linkRequestIsRecordedWithoutAccount` — POST `/api/sign-in/link` for `a@example.com` → one row `LINK_REQUESTED/SUCCESS`, `account_id` null, `ip_address = '127.0.0.1'`.
  - `rateLimitRefusalIsRecorded` — with `beelot.sign-in.limit-per-address` default 3, the 4th request for one address → a `RATE_LIMITED/REFUSED` row.
  - `emailSignInIsRecordedWithTheAccount` — open the link → `SIGN_IN/SUCCESS` with the new account's id; the session attribute `beelot.accountId` equals that id.
  - `usedLinkIsRecordedAsFailedSignIn` — post the same token twice → a `SIGN_IN/FAILURE` row with null account.
  - `signOutIsRecordedAndNotAsExpiry` — POST `/logout` → `SIGN_OUT/SUCCESS` with the account id and no `SESSION_EXPIRED` row.
  - `expiredSessionIsRecorded` — sign in, then `session.invalidate()` on the `MockHttpSession` after publishing `new HttpSessionDestroyedEvent(session)` through the `ApplicationContext` → `SESSION_EXPIRED/SUCCESS` with the account id.
  - `noTokenIsEverStored` — after a full sign-in, no column of any `security_event` row contains the token or its SHA-256 hex.
  - `recordingFailureDoesNotThrow` — unit test: `SecurityEvents` built on a `JdbcClient` whose `sql(..)` throws → `record(..)` returns normally.

- [ ] **Step 3: Run** `./gradlew test --tests '*SecurityEventsIntegrationTest'` — expected: FAIL (class missing).

- [ ] **Step 4: Implement `SecurityEvents`** (`@Component`, constructor `(JdbcClient jdbc, Clock clock)`): inserts one row per call inside try/catch; plus `@EventListener void sessionDestroyed(HttpSessionDestroyedEvent event)` recording `SESSION_EXPIRED/SUCCESS` with the session's `ACCOUNT_ID` when that attribute is set and `ENDED` is not. Register `HttpSessionEventPublisher` as a `@Bean` in `SecurityConfiguration` so the event fires.

- [ ] **Step 5: Wire the callers.**
  - `SignInRequestFilter(SignInRateLimiter, JsonMapper, SecurityEvents)`: on rate-limit refusal record `RATE_LIMITED/REFUSED`; before `chain.doFilter` record `LINK_REQUESTED/SUCCESS`. IP is `request.getRemoteAddr()`.
  - `SignInSuccessHandler`: after the account is found or created, set the session attribute `ACCOUNT_ID` and record `SIGN_IN/SUCCESS` with its id.
  - `SecurityConfiguration`: the OTT failure handler and a new OAuth failure handler (still redirecting to `/?signin=failed`) record `SIGN_IN/FAILURE` with a null account; a `LogoutHandler` added with `logout.addLogoutHandler(..)` sets `ENDED` on the session and records `SIGN_OUT/SUCCESS` with its `ACCOUNT_ID`.

- [ ] **Step 6: Run** `./gradlew test` — expected: all PASS (existing `SignInRequestFilterTest` gets the extra constructor argument, a mock).

- [ ] **Step 7: Commit** `feat(privacy): record security events without tokens (US-083)`.

---

### Task 2: Account sessions and the recent sign-in

**Files:**
- Create: `application/privacy/AccountSessions`
- Modify: `application/account/Account`, `SignInSuccessHandler`, `AccountController`
- Test: `tests/application/privacy/AccountSessionsTest`, `tests/application/account/AccountServiceTest`

**Interfaces:**
- Consumes: `SecurityEvents.ACCOUNT_ID`, `SecurityEvents.ENDED` (Task 1).
- Produces:
  - `AccountSessions` (`@Component`): `public void signedIn(UUID accountId, HttpSession session)` — stores the session and sets `SIGNED_IN_AT` to `clock.instant()`; `public void invalidateAll(UUID accountId)` — sets `ENDED` and invalidates each live session, ignoring `IllegalStateException` from an already-invalid one; `@EventListener` on `HttpSessionDestroyedEvent` removes the session.
  - `public static final String SIGNED_IN_AT = "beelot.signedInAt"` — an `Instant`.
  - `public static boolean signedInRecently(HttpSession session, Clock clock)` — true when `SIGNED_IN_AT` is set and less than 10 minutes before `clock.instant()`.
  - `Account.deletionWarnedAt()` (nullable `Instant`, column `deletion_warned_at`); `Account.signedIn(now)` now also clears it.

- [ ] **Step 1: Write the failing tests.**
  - `AccountSessionsTest` (plain unit test, `MockHttpSession`, fixed `Clock`):
    - `invalidateAllEndsEverySessionOfTheAccount` — two sessions for account A, one for B → after `invalidateAll(A)` both A sessions are invalid and marked `ENDED`, B's is valid.
    - `destroyedSessionIsForgotten` — publish-style call of the listener with `HttpSessionDestroyedEvent` → a later `invalidateAll` does not touch it.
    - `signInNineMinutesAgoIsRecent`, `signInTenMinutesOldMustReauthenticate` (exactly 10 minutes → false), `noSignInTimeIsNotRecent`.
  - `AccountServiceTest`: `signingInClearsTheDeletionWarning` — an account with `deletion_warned_at` set (set via `JdbcClient` or reflection, matching how the test already builds accounts) → `signIn(..)` leaves `deletionWarnedAt()` null.

- [ ] **Step 2: Run** `./gradlew test --tests '*AccountSessionsTest' --tests '*AccountServiceTest'` — expected: FAIL.

- [ ] **Step 3: Implement** `AccountSessions` (a `ConcurrentHashMap<UUID, Set<HttpSession>>` with concurrent sets), the `Account` field, and call `accountSessions.signedIn(account.id(), request.getSession())` in `SignInSuccessHandler` next to the Task 1 attribute.

- [ ] **Step 4: Safety net.** `AccountController.account(Authentication, HttpServletRequest)`: when `authentication` is an `OAuth2AuthenticationToken` or `OneTimeTokenAuthentication` but `accounts.current(..)` is empty, invalidate the session (set `ENDED` first) and answer as for a guest. Test in `AccountIntegrationTest` or a new test: `sessionOfAMissingAccountIsSignedOut` — sign in by email, delete the `account` row with `JdbcClient`, GET `/api/account` → `signedIn:false`, and the session is invalidated.

- [ ] **Step 5: Run** `./gradlew test` — expected: all PASS.

- [ ] **Step 6: Commit** `feat(privacy): track each account's sessions and when it signed in (US-081)`.

---

### Task 3: Policy configuration and endpoint

**Files:**
- Create: `application/privacy/PrivacyProperties`, `PrivacyController`
- Modify: `src/main/resources/application.properties`, `README.md`
- Test: `tests/application/privacy/PolicyInfoTest`

**Interfaces:**
- Produces:
  - `@ConfigurationProperties("beelot.privacy") public record PrivacyProperties(String controller, String contact, String hostingProvider, String smtpProvider, String backupRetentionDays)` — blank means not configured.
  - `PrivacyController` (`@RestController`), started here and extended in Tasks 5 and 7; `GET /api/privacy/policy-info` → `PolicyInfo(String controller, String contact, String hostingProvider, String smtpProvider, String backupRetentionDays)`, nulls for blank values.

- [ ] **Step 1: Write the failing tests** (`@SpringBootTest` with Testcontainers like other integration tests, or `@WebMvcTest` with `WebSliceTestConfiguration` if that slice already serves controllers in this repo):
  - `policyInfoComesFromTheEnvironment` — properties `beelot.privacy.controller=Beelot SAS`, `beelot.privacy.contact=privacy@example.com`, `beelot.privacy.hosting-provider=Hetzner`, `beelot.privacy.smtp-provider=Brevo`, `beelot.privacy.backup-retention-days=30` → JSON with those five values; reachable without signing in.
  - `unsetValuesAreNull` — no properties → every field null (absent or `null`), status 200.

- [ ] **Step 2: Run** — expected: FAIL.

- [ ] **Step 3: Implement.** In `application.properties` add:
  ```properties
  # Privacy policy values (US-083), from the environment where Beelot runs.
  beelot.privacy.controller=${BEELOT_PRIVACY_CONTROLLER:}
  beelot.privacy.contact=${BEELOT_PRIVACY_CONTACT:}
  beelot.privacy.hosting-provider=${BEELOT_HOSTING_PROVIDER:}
  beelot.privacy.smtp-provider=${BEELOT_SMTP_PROVIDER:}
  beelot.privacy.backup-retention-days=${BEELOT_BACKUP_RETENTION_DAYS:}
  ```
  Enable the record with `@EnableConfigurationProperties(PrivacyProperties.class)` on `PrivacyController`. Log one WARN at startup (`@EventListener(ApplicationReadyEvent.class)`) naming each unset env var.

- [ ] **Step 4: README.** Add a "Privacy" section: the five env vars and what each fills on the policy page; they may also be set in `.env` with the canonical `beelot.privacy.*` keys.

- [ ] **Step 5: Run** `./gradlew test` — expected: all PASS.

- [ ] **Step 6: Commit** `feat(privacy): serve the privacy policy values from the environment (US-083)`.

---

### Task 4: Privacy policy page

**Files:**
- Modify: `application/HomeController` (add `/privacy` to the route list)
- Modify: `src/main/resources/static/index.html`, `app.js`, `styles.css`, `i18n.js`, `i18n-nl.js`
- Test: `tests/application/HomeControllerTest`, `FrenchTranslationUiTest`, `DutchTranslationUiTest`

**Interfaces:**
- Consumes: `GET /api/privacy/policy-info` (Task 3).
- Produces: client route `/privacy` → view `data-view="privacy"`; element ids `privacy-controller`, `privacy-contact`, `privacy-hosting`, `privacy-smtp`, `privacy-backup-days`, filled by `app.js` (text "not configured" when null, translated).

- [ ] **Step 1: Write the failing tests.**
  - `HomeControllerTest`: `privacyPageIsServedByTheClient` — GET `/privacy` forwards to the client like `/settings`.
  - `FrenchTranslationUiTest`: `privacyPolicyIsTranslated` — `i18n.js` contains `"Privacy policy": "Politique de confidentialité"` and `"How we handle your data": "Comment nous traitons vos données"`.
  - `DutchTranslationUiTest`: `privacyPolicyIsTranslated` — `i18n-nl.js` contains `"Privacy policy": "Privacybeleid"` and `"How we handle your data": "Hoe we met je gegevens omgaan"`.
  - A resource test (in `HomeControllerTest` or a new `PrivacyPageUiTest`): `index.html` has `data-view="privacy"`, a home-screen link `href="/privacy"`, and under the sign-in form a link with text `How we handle your data`.

- [ ] **Step 2: Run** — expected: FAIL.

- [ ] **Step 3: Write the page.** Sections, in this order, as `<section><h2>`:
  1. Who is responsible — controller and contact (placeholders above).
  2. What we store and why — account (pseudonym, sign-in method, email address for email-link players, creation and last sign-in dates; legal basis: contract); match history (contract); security events with IP address (legitimate interest: security); sign-in tokens (contract).
  3. Who processes it — hosting provider, SMTP provider (email-link players), Google and GitHub (only for players who sign in with them; Beelot asks only for the user id).
  4. How long we keep it — sign-in links 1 day; accounts 3 years after the last sign-in, with a warning email 30 days before for email-link players; security events 12 months; backups `privacy-backup-days` days.
  5. Cookies — only the session cookie and the CSRF cookie (`XSRF-TOKEN`), strictly necessary, so no consent banner.
  6. Your rights — access and portability (Settings → Download my data), erasure (Settings → Delete my account), rectification, restriction and objection through the contact address; the right to complain to the CNIL (link `https://www.cnil.fr/fr/plaintes`).

  Write the French text first and translate it to English (the keys) and Dutch. Link the page from the home screen (footer) and under the sign-in form. Fill the placeholders on view entry from `/api/privacy/policy-info`.

- [ ] **Step 4: Run** `./gradlew test` — expected: all PASS. Then start the app (`docker compose up -d`, `./gradlew bootRun`) and check `/privacy` headless in en, fr and nl.

- [ ] **Step 5: Commit** `feat(privacy): add the privacy policy page in three languages (US-083)`.

---

### Task 5: Delete my account (server)

**Files:**
- Create: `application/privacy/AccountDeletionService`
- Modify: `application/privacy/PrivacyController`
- Modify: `application/privategame/PrivateTableService`, `game/PrivateTable`
- Modify: `application/history/MatchHistoryService`
- Test: `tests/application/privacy/AccountDeletionIntegrationTest`, `tests/application/privategame/PrivateTableServiceTest`, `tests/application/history/MatchHistoryServiceTest`

**Interfaces:**
- Consumes: `SecurityEvents` (Task 1), `AccountSessions` (Task 2), `AccountService.current(Authentication)`.
- Produces:
  - `public static final String DELETED_PLAYER = "Deleted player"` in `AccountDeletionService`.
  - `AccountDeletionService.delete(UUID accountId)` — used by Task 6.
  - `PrivateTableService.releaseAccount(UUID accountId, String seatName)` — frees the account's seats at tables waiting for players; at tables in play, its seats become `BOT_TAKEOVER` named `seatName`; forgets the account id for those tables.
  - `PrivateTable.handToBot(UUID playerId, String name)` (synchronized).
  - `DELETE /api/account` → 204; 401 signed out; 403 `{"reason":"reauthenticate"}`.

- [ ] **Step 1: Write the failing tests.**
  - `PrivateTableServiceTest`: `releasedAccountIsReplacedByABotInAMatch` — a started table with the account's player → seat is `BOT_TAKEOVER` named `Deleted player`, and the finished match passed to the recorder has a null account for that seat; `releasedAccountLeavesAWaitingTable` — a waiting table → seat freed.
  - `MatchHistoryServiceTest`: `matchFinishedAfterDeletionIsRecordedAnonymously` — a `FinishedMatch` seat whose account id is unknown → stored with null account and name `Deleted player`.
  - `AccountDeletionIntegrationTest` (Testcontainers, MockMvc, mocked `MailSender`, signs in by email link as in `EmailSignInIntegrationTest`):
    - `deletionRemovesTheAccountAndItsData` — after DELETE: no `account`, `account_email`, `sign_in_token` (for the address) or `security_event` rows for the account; one `DELETION/SUCCESS` row with null account.
    - `matchesStayWithTheSeatRenamed` — a recorded match with the account and another account → the match remains; the deleted seat has null account and name `Deleted player`; the other seat is unchanged.
    - `deletionEndsEverySession` — two sessions signed in to the same account → after DELETE from one, GET `/api/account` with the other says `signedIn:false`.
    - `signInTenMinutesOldMustReauthenticate` — set `beelot.signedInAt` to now minus 10 minutes → 403 with `reason = reauthenticate`, account still present.
    - `deletionNeedsCsrf` — DELETE without CSRF → 403, account still present.
    - `guestCannotDelete` — no session → 401.

- [ ] **Step 2: Run** — expected: FAIL.

- [ ] **Step 3: Implement `AccountDeletionService`** (`@Service`, constructor `(JdbcClient, TransactionTemplate, AccountSessions, PrivateTableService, SecurityEvents)`). `delete` runs, in one transaction: update `match_seat` (`account_id = NULL, name = 'Deleted player'`), delete the account's `security_event` rows, delete `sign_in_token` rows whose `email` is the account's address, delete `account_email`, delete `account`. After commit, each in its own try/catch with a WARN log: `privateTables.releaseAccount(id, DELETED_PLAYER)`, `accountSessions.invalidateAll(id)`, `events.record(DELETION, SUCCESS, null, ipOrNull)`. Give `delete` an overload `delete(UUID accountId, String ip)`; the retention job passes null.

- [ ] **Step 4: Implement the endpoint** in `PrivacyController`: resolve the account with `AccountService.current(authentication)` (401 if empty), check `AccountSessions.signedInRecently(session, clock)` (403 reauthenticate), then delete and return 204.

- [ ] **Step 5: Implement the game side.** `PrivateTable.handToBot` and `PrivateTableService.releaseAccount` (walk `accountIds`, then `leave` or `handToBot` under the table's lock, and drive bot turns if it is now a bot's turn). `MatchHistoryService.store`: when `accounts.displayName(seat.accountId())` is empty, store `null` and `Deleted player`.

- [ ] **Step 6: Run** `./gradlew test` — expected: all PASS.

- [ ] **Step 7: Commit** `feat(privacy): let players delete their account (US-081)`.

---

### Task 6: Retention job

**Files:**
- Create: `application/privacy/RetentionJob`, `InactivityWarningEmail`
- Modify: `src/main/resources/application.properties`
- Test: `tests/application/privacy/RetentionJobIntegrationTest`

**Interfaces:**
- Consumes: `AccountDeletionService.delete(UUID, String)` (Task 5).
- Produces: `RetentionJob.run()` — `@Scheduled(cron = "${beelot.privacy.retention-cron:0 30 3 * * *}", zone = "UTC")`; constructor `(JdbcClient, TransactionTemplate, AccountDeletionService, MailSender, Clock, @Value("${beelot.mail.from}") String from, @Value("${beelot.public-url}") String publicUrl)`.
- `InactivityWarningEmail.of(String publicUrl)` → `SignInEmail`-like record `(subject, body)`: subject `Beelot: your account will be deleted in 30 days / votre compte sera supprimé dans 30 jours / je account wordt over 30 dagen verwijderd`; the body says, in French, English and Dutch, that the account will be deleted in 30 days unless the player signs in at `publicUrl`.

- [ ] **Step 1: Write the failing tests** (Testcontainers; build `RetentionJob` in the test with `Clock.fixed(NOW, UTC)` where `NOW = 2030-01-01T00:00:00Z`; insert accounts with `JdbcClient`):
  - `oldEventsArePurged` — events at `NOW − 366 days` and `NOW − 364 days` → only the second remains.
  - `inactiveEmailAccountIsWarnedOnce` — email account last signed in at `NOW − (3 years − 30 days)` → one email sent to its address, `deletion_warned_at = NOW`; a second `run()` sends nothing.
  - `warnedAccountIsDeletedThirtyDaysLater` — last sign-in `NOW − 3 years`, warned at `NOW − 30 days` → deleted; warned at `NOW − 29 days` → kept.
  - `providerAccountIsDeletedWithoutWarning` — Google account last signed in `NOW − 3 years` → deleted, no email.
  - `failedWarningIsRetried` — `MailSender.send` throws → `deletion_warned_at` stays null and the account stays; no exception escapes `run()`.
  - `activeAccountIsUntouched` — last sign-in `NOW − 1 year` → no email, not deleted.

  "3 years" is `NOW.atZone(UTC).minusYears(3)`; "3 years − 30 days" is that plus 30 days.

- [ ] **Step 2: Run** — expected: FAIL.

- [ ] **Step 3: Implement.** Purge events in one statement. Then warn, then delete, each account in its own `TransactionTemplate` call wrapped in try/catch with a WARN log. Deletion candidates: `last_sign_in_at <= NOW − 3 years` and (no `account_email` row, or `deletion_warned_at <= NOW − 30 days`).

- [ ] **Step 4: Run** `./gradlew test` — expected: all PASS.

- [ ] **Step 5: Commit** `feat(privacy): enforce retention periods every night (US-083)`.

---

### Task 7: Download my data (server)

**Files:**
- Create: `application/privacy/DataExportService`, `DataExport`
- Modify: `application/privacy/PrivacyController`
- Test: `tests/application/privacy/DataExportIntegrationTest`

**Interfaces:**
- Consumes: `SecurityEvents` (Task 1), `AccountService.current`.
- Produces:
  - `DataExport` — nested records mirroring the spec's "Export format" JSON exactly (field names as there), registered for reflection with `@RegisterReflectionForBinding` on the service.
  - `DataExportService.export(UUID accountId, String ip)` → `Result` sealed: `Ready(DataExport)` or `TooSoon(long minutesLeft)`; records `EXPORT/SUCCESS` when ready.
  - `GET /api/account/export` → 200 `application/json` with `Content-Disposition: attachment; filename="beelot-data-YYYY-MM-DD.json"` (UTC date); 401 signed out; 429 with `Retry-After` (seconds) and `{"minutesLeft": n}`.

- [ ] **Step 1: Write the failing tests** (Testcontainers, MockMvc, sign in by email link):
  - `exportHoldsAccountEventsAndMatches` — after one recorded match (inserted with `JdbcClient` into `match_record`, `match_seat`, `match_round`) → `format = beelot-export-1`, `account.signIn.method = email`, `account.signIn.email` the address, at least one `SIGN_IN` event, one match with `yourTeam`, `won`, 4 seats and its rounds.
  - `exportHoldsNoAccountIdsOfOthersAndNoTokens` — the body contains neither the other seat's account id nor any `sign_in_token.token_hash` value nor the provider subject.
  - `providerAccountExportsOnlyTheMethod` — a Google account (`oauth2Login()` request post-processor) → `account.signIn` is `{"method":"google"}`.
  - `secondExportWithinAnHourIsRefused` — second GET → 429, `Retry-After` present.
  - `guestCannotExport` — 401.
  - `attachmentIsNamedByDate` — `Content-Disposition` matches `attachment; filename="beelot-data-\d{4}-\d{2}-\d{2}.json"`.

- [ ] **Step 2: Run** — expected: FAIL.

- [ ] **Step 3: Implement** with `JdbcClient` queries; events and matches ordered newest first.

- [ ] **Step 4: Run** `./gradlew test` — expected: all PASS.

- [ ] **Step 5: Commit** `feat(privacy): let players download their data (US-082)`.

---

### Task 8: "Your data" in Settings

**Files:**
- Modify: `src/main/resources/static/index.html`, `app.js`, `styles.css`, `i18n.js`, `i18n-nl.js`
- Test: `tests/application/AccountUiTest`, `FrenchTranslationUiTest`, `DutchTranslationUiTest`

**Interfaces:**
- Consumes: `GET /api/account` (`signedIn`), `GET /api/account/export` (Task 7), `DELETE /api/account` (Task 5), the existing sign-in flows.
- Produces: in the settings view, a panel `id="your-data"` shown only when signed in, with `id="export-data"` (link to `/api/account/export`), `id="export-message"`, `id="delete-account"`, a confirmation block `id="delete-confirm"` with input `id="delete-confirm-word"` and button `id="delete-confirm-button"`, and `id="delete-message"`.

- [ ] **Step 1: Write the failing tests** (resource checks, as the existing UI tests do):
  - `AccountUiTest`: `settingsOfferYourData` — `index.html` contains the ids above and the texts `Download my data`, `Delete my account`; `app.js` calls `fetch("/api/account", { method: "DELETE"` with the CSRF header the client already sends on other state-changing requests.
  - Translation tests: French `"Download my data": "Télécharger mes données"`, `"Delete my account": "Supprimer mon compte"`, `"DELETE": "SUPPRIMER"`; Dutch `"Download my data": "Mijn gegevens downloaden"`, `"Delete my account": "Mijn account verwijderen"`, `"DELETE": "VERWIJDEREN"`.

- [ ] **Step 2: Run** — expected: FAIL.

- [ ] **Step 3: Implement.**
  - Export: fetch `/api/account/export`; on 200 save the blob under the name from `Content-Disposition`; on 429 show `You can download your data again in {n} minutes.`
  - Delete: "Delete my account" reveals the confirmation, which says: `Your account, your email address and your sign-in history will be deleted. Finished matches stay in other players' history, shown as "Deleted player".` and asks to type the translated `DELETE`; the button stays disabled until the input matches. On 204: go to `/`, refresh the account bar, show `Your account has been deleted.` On 403 `reauthenticate`: show `For your safety, sign in again to delete your account.`, store `beelot.afterSignIn = "/settings?confirm-delete"` in `localStorage` (try/catch), and open the sign-in panel. After any successful sign-in, if that key is set, remove it and navigate there; `?confirm-delete` opens the confirmation directly.

- [ ] **Step 4: Run** `./gradlew test` — expected: all PASS. Start the app with Mailpit and run headless (never a visible browser): sign in by email link, export, wait out or reset the 10 minutes by restarting the session, delete with reauthentication, check the home screen says the account was deleted; repeat the policy page check in nl.

- [ ] **Step 5: Commit** `feat(privacy): add download and delete to Settings (US-081, US-082)`.

---

### Task 9: Documents and native build

**Files:**
- Create: `docs/privacy/data-breach-procedure.md`
- Modify: `README.md`, `docs/product-backlog.md`

- [ ] **Step 1: Breach procedure.** Who decides (the controller named by `BEELOT_PRIVACY_CONTROLLER`); first steps (contain, preserve logs); assessing the risk; notifying the CNIL within 72 hours of becoming aware (`https://notifications.cnil.fr/notifications/index`), with what is known and follow-ups; informing players without undue delay when the risk to them is high; keeping a breach log (date, facts, effects, measures) even when nothing is notified.

- [ ] **Step 2: README.** Under "Privacy": sign a DPA (GDPR article 28) with the hosting and the SMTP provider before production; backups must be overwritten within `BEELOT_BACKUP_RETENTION_DAYS` days, which the policy states; link the breach procedure.

- [ ] **Step 3: Backlog.** Mark US-081, US-082 and US-083 delivered with a link to the spec; replace "email change" in US-083's event list with nothing (Beelot cannot change an address); move "Name the data controller and the privacy contact" to say it is set through the env vars.

- [ ] **Step 4: Native build smoke test.** `./gradlew nativeCompile`, run the binary against the compose MariaDB and Mailpit, sign in by email link, GET `/api/account/export` and `/api/privacy/policy-info` → 200 with the expected fields (reflection hints work).

- [ ] **Step 5: Commit** `docs: document privacy duties and record US-081 to US-083 as delivered`.
