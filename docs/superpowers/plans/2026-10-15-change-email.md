# Change my email address (US-079) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Email-link players change their address from Settings; the new address takes over once a link sent to it is confirmed, the old address is notified, and every session of the account ends.

**Architecture:** A new `email_change_token` table holds hashed, single-use, 15-minute tokens. `EmailChangeService` (package `account`) requests and confirms changes with `JdbcClient` and a `TransactionTemplate`, sending mail and ending sessions after the database work. `EmailChangeController` exposes `POST /api/account/email` and `POST /api/account/email/confirm`; confirmation signs the browser in by reusing `SignInSuccessHandler`. The vanilla-JS client gains an "Account" panel in Settings and a `/settings/email` confirmation view.

**Tech Stack:** Spring Boot 4 / Spring Security 7 (one-time-token login), JdbcClient, Flyway on MariaDB 11.4, JUnit 5 + Testcontainers + MockMvc, vanilla JS with `i18n.js` (French) and `i18n-nl.js` (Dutch).

**Spec:** `docs/superpowers/specs/2026-10-15-change-email-design.md`

## Global Constraints

- Tokens: 32 random bytes from `SecureRandom`, base64url without padding (43 chars); only the SHA-256 hex is stored; lifetime `beelot.sign-in.link-lifetime` (default `PT15M`); single use; purge 1 day after expiry, hourly.
- Never log or record a token or an address (logs: exception class name only; `security_event` rows: account id, type, outcome, IP).
- Recent sign-in = `AccountSessions.signedInRecently(session, clock)` (less than 10 minutes).
- Addresses are normalised with `EmailAddresses.normalise` before any comparison or storage.
- Rate limit: `SignInRateLimiter.tryAcquire(newEmail, ip)` — same budget as sign-in links.
- Every user-visible string exists in English (source), French (`i18n.js`) and Dutch (`i18n-nl.js`); emails in all three via `SignInEmail.language(request)`.
- Run `./gradlew test` before each commit that touches code; commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

- A player who requests twice and opens the *first* link: it must be refused (only the latest works) — test in Task 2.
- The browser that confirms is already signed in to the same account: `invalidateAll` ends that session too, and the response must still leave it signed in with a new session — test in Task 3.
- Login CSRF: a confirm POST without the CSRF token must be refused even for a guest, as `/login/ott` is — test in Task 3.
- The new address differs from the current one only in case or surrounding spaces: normalisation makes it `SameAddress` (400) — test in Task 2.
- The account is deleted while a change is pending: the token goes with it and the link answers 400 — test in Task 2.

---

### Task 1: Change-of-address emails

**Files:**
- Create: `application/account/EmailChangeEmail`
- Test: `tests/application/account/EmailChangeEmailTest`

**Interfaces:**
- Produces: `public record EmailChangeEmail(String subject, String body)` with
  - `static EmailChangeEmail link(Locale locale, String link)`
  - `static EmailChangeEmail taken(Locale locale)`
  - `static EmailChangeEmail changed(Locale locale, String newEmail, String contactOrBlank)`
  - `static String mask(String email)`

- [ ] **Step 1: Write the failing tests**

```
@Test void theLinkSitsOnItsOwnLineInEachLanguage() {
    for (String lang : List.of("en", "fr", "nl")) {
        EmailChangeEmail email = EmailChangeEmail.link(Locale.forLanguageTag(lang), "https://beelot.test/settings/email?token=abc");
        assertThat(email.body().lines()).contains("https://beelot.test/settings/email?token=abc");
    }
    assertThat(EmailChangeEmail.link(Locale.ENGLISH, "x").subject()).isEqualTo("Confirm your new Beelot address");
    assertThat(EmailChangeEmail.link(Locale.FRENCH, "x").subject()).isEqualTo("Confirmez votre nouvelle adresse Beelot");
    assertThat(EmailChangeEmail.link(Locale.forLanguageTag("nl"), "x").subject()).isEqualTo("Bevestig je nieuwe Beelot-adres");
}
@Test void anUnknownLanguageFallsBackToEnglish() { /* Locale.GERMAN and null → English subject */ }
@Test void theTakenNoticeHasNoLink() { /* body contains no "http" in each language; subject "This address already has a Beelot account" (en) */ }
@Test void theChangeNoticeShowsTheMaskedAddressAndTheContact() {
    EmailChangeEmail email = EmailChangeEmail.changed(Locale.ENGLISH, "new.player@example.com", "privacy@beelot.test");
    assertThat(email.subject()).isEqualTo("Your Beelot address was changed");
    assertThat(email.body()).contains("n***@example.com").contains("privacy@beelot.test").doesNotContain("new.player");
}
@Test void theChangeNoticeOmitsABlankContact() { /* changed(..., "") body contains no "write to" sentence (en) */ }
@Test void maskKeepsTheFirstLetterAndTheDomain() {
    assertThat(EmailChangeEmail.mask("a@b.fr")).isEqualTo("a***@b.fr");
    assertThat(EmailChangeEmail.mask("player@example.com")).isEqualTo("p***@example.com");
}
```

- [ ] **Step 2: Run** `./gradlew test --tests '*EmailChangeEmailTest'` — Expected: FAIL (class missing).

- [ ] **Step 3: Implement `EmailChangeEmail`** in the style of `SignInEmail` (switch on `locale.getLanguage()`, text blocks, English default). Copy, English (translate faithfully to French "vous" and Dutch "je", as `SignInEmail` does):
  - *link*: "Hello,\n\nUse this link to make this address the one you sign in to Beelot with.\n\n%s\n\nThe link works once, for 15 minutes. Until you open it, your current address keeps working. If you did not ask for this, ignore this email."
  - *taken* — subject "This address already has a Beelot account": "Hello,\n\nSomeone asked to move a Beelot account to this address, but it already has an account of its own. Nothing was changed. You can keep signing in with this address as usual.\n\nIf you did not ask for this, ignore this email."
  - *changed*: "Hello,\n\nYour Beelot account now signs in with %s. This address no longer works for signing in, and every session of the account has been signed out.\n\n[If you did not make this change, write to %s.]" — the bracketed sentence only when the contact is not blank.

- [ ] **Step 4: Run** the same command — Expected: PASS.

- [ ] **Step 5: Commit** `feat(account): add the change-of-address emails (US-079)`

---

### Task 2: Requesting and confirming a change

**Files:**
- Create: `src/main/resources/db/migration/V7__email_change.sql` (exact SQL in the spec, "Data")
- Create: `application/account/Tokens`
- Create: `application/account/EmailChangeService`
- Modify: `application/account/SignInTokenService` (use `Tokens`)
- Modify: `application/privacy/SecurityEvents` (`Type` gains `EMAIL_CHANGE_REQUESTED`, `EMAIL_CHANGED`)
- Modify: `application/account/AccountService` (add `email`)
- Test: `tests/application/account/EmailChangeServiceIntegrationTest`

**Interfaces:**
- Consumes: `EmailChangeEmail` (Task 1); `SignInRateLimiter.tryAcquire`; `AccountSessions.invalidateAll(UUID)`; `SecurityEvents.record`; `PrivacyProperties.contact()`; `MailSender`; `@Value("${beelot.public-url}")`, `@Value("${beelot.mail.from}")`, `@Value("${beelot.sign-in.link-lifetime:PT15M}")`.
- Produces:
  - `final class Tokens { static String newToken(); static String hash(String token); }` (package-private; moved from `SignInTokenService`, whose `hash` callers switch to `Tokens.hash`)
  - `public Optional<String> AccountService.email(UUID accountId)` — the account's address, empty for Google/GitHub
  - `@Service public class EmailChangeService` with
    - `public sealed interface RequestResult permits Sent, InvalidAddress, SameAddress, RateLimited, MailFailed` (records nested in the service, no fields)
    - `public RequestResult request(UUID accountId, String newEmail, Locale locale, String ip)`
    - `public Optional<Confirmed> confirm(String token, Locale locale, String ip)` with `public record Confirmed(UUID accountId, String newEmail)`
    - `@Scheduled(fixedDelayString = "PT1H", initialDelayString = "PT1M") int purgeExpired()`

- [ ] **Step 1: Write the failing integration tests** (Testcontainers MariaDB, `@SpringBootTest(properties = "beelot.sign-in.limit-per-ip=1000")`, `@MockitoBean MailSender mail`, mail captured with `ArgumentCaptor<SimpleMailMessage>`; accounts created with `AccountService.signInWithEmail(address, Locale.ENGLISH)`; link token extracted with `Pattern.compile("/settings/email\\?token=([A-Za-z0-9_-]{43})")`):
  - `aFreeAddressReceivesALinkAndNothingChangesYet` — `request` returns `Sent`; one mail to the new address containing the link; one `email_change_token` row whose `token_hash` ≠ the clear token; `account_email` still the old address; one `EMAIL_CHANGE_REQUESTED` event for the account.
  - `confirmingMovesTheAccountAndNotifiesTheOldAddress` — `confirm` returns `Confirmed(account, new)`; `account_email` has the new address; a mail to the old address with subject "Your Beelot address was changed"; `EMAIL_CHANGED` event; `email_change_token` empty.
  - `confirmingEndsEverySessionOfTheAccount` — register a `MockHttpSession` with `AccountSessions.signedIn(account, session)`; after `confirm`, `session.isInvalid()`.
  - `aTakenAddressReceivesANoticeAndNoToken` — returns `Sent`; one mail to the taken address with subject "This address already has a Beelot account"; no token row; nothing changes.
  - `theCurrentAddressIsRefused` — `request(account, "  OLD@Example.com ", …)` where the current is `old@example.com` → `SameAddress`; no mail.
  - `anInvalidAddressIsRefused` — `"not-an-address"` → `InvalidAddress`.
  - `theRateLimitApplies` — with `beelot.sign-in.limit-per-address=3`, the 4th request for the same new address → `RateLimited`.
  - `aSecondRequestReplacesTheFirstLink` — first link's `confirm` → empty; second link's → present.
  - `aTokenWorksOnce` and `anExpiredTokenIsRefused` (set `expires_at` in the past with JdbcClient).
  - `anAddressTakenSinceTheRequestIsRefused` — request, then `signInWithEmail(newAddress)` creates another account, then `confirm` → empty; `account_email` unchanged for the first account; no change notice sent.
  - `deletingTheAccountRemovesThePendingChange` — delete the `account` row; token row gone; `confirm` → empty.
  - `aMailFailureKeepsNoToken` — `doThrow(new MailSendException("down")).when(mail).send(any(SimpleMailMessage.class))` → `MailFailed`; no token row.
  - `purgeRemovesTokensExpiredForADay`.

- [ ] **Step 2: Run** `./gradlew test --tests '*EmailChangeServiceIntegrationTest'` — Expected: FAIL (classes missing).

- [ ] **Step 3: Add `V7__email_change.sql`, `Tokens`, the two event types and `AccountService.email`;** switch `SignInTokenService` to `Tokens`.

- [ ] **Step 4: Implement `EmailChangeService`.**
  - `request`: normalise (empty → `InvalidAddress`); equals `accountService.email(accountId)` → `SameAddress`; `!limiter.tryAcquire(new, ip)` → `RateLimited`; delete the account's tokens; if `account_email` has the address → send *taken*, else insert the token row (in a `TransactionTemplate`) and send *link* with `publicUrl + "/settings/email?token=" + token`; on `MailException` delete the inserted row, log the class name, return `MailFailed`; record `EMAIL_CHANGE_REQUESTED` with `Outcome.SUCCESS` on `Sent`, `REFUSED` on `RateLimited`.
  - `confirm`, inside one `TransactionTemplate`: select the row by `Tokens.hash(token)`; delete it, accept only if exactly one row was deleted and `expires_at` is after now; read the old address; if another account has the new address → empty; `UPDATE account_email SET email = ? WHERE account_id = ?` (a `DuplicateKeyException` → empty). After the transaction, on success: send *changed* to the old address (a `MailException` is logged by class name only), record `EMAIL_CHANGED`, `sessions.invalidateAll(accountId)`.
  - Mail sender, `from` and URL trimming copy `SignInLinkSender`.

- [ ] **Step 5: Run** the integration test, then `./gradlew test` — Expected: all PASS (including `SignInTokenServiceTest`).

- [ ] **Step 6: Commit** `feat(account): request and confirm a change of email address (US-079)`

---

### Task 3: Endpoints and signing in the confirming browser

**Files:**
- Create: `application/account/EmailChangeController`
- Modify: `application/account/AccountController` (`AccountView` gains `String email` after `name`, `null` for guests and OAuth)
- Modify: `application/security/SecurityConfiguration` (`LINK_SIGN_IN` also matches `POST /api/account/email/confirm`, so CSRF is always required on it)
- Modify: `application/HomeController` (forward `/settings/email` to `index.html`)
- Test: `tests/application/account/EmailChangeIntegrationTest`

**Interfaces:**
- Consumes: `EmailChangeService` (Task 2), `AccountService.current/email`, `SignInSuccessHandler`, `AccountSessions.signedInRecently`, `EmailUserDetailsService`, `SignInEmail.language(request)`.
- Produces:
  - `POST /api/account/email` (form field `email`): 401 guest; 403 `{}` for an OAuth account; 403 `{"reason":"reauthenticate"}` without a recent sign-in; 400 `{"message":"Enter a valid email address."}` / `{"message":"This is already your address."}`; 429 `{"message":"Too many links requested. Please try again later."}`; 503 `{"message":"The email could not be sent. Please try again later."}`; 204.
  - `POST /api/account/email/confirm` (form fields `token`, `language`): 400 `{"message":"This link no longer works."}`; 204 with the browser signed in as the new address.
  - `/api/account` JSON gains `"email"`.

- [ ] **Step 1: Write the failing MockMvc tests** (same setup as `AccountDeletionIntegrationTest`: `requestLink`, `signIn` helpers, mocked mail; the change link is taken from the mail to the new address):
  - `theAccountShowsTheAddressOfAnEmailPlayer` — `GET /api/account` → `$.email` = the address; guests have no `email`.
  - `aChangeMovesSignInToTheNewAddress` — sign in old@, `POST /api/account/email` → 204; confirm with `csrf()` on a fresh request (no session) → 204; the returned session's `GET /api/account` → `signedIn` true, `email` new@; the old session's `GET /api/account` → `signedIn` false and it is invalid; a sign-in link for new@ reaches the same account id; a sign-in link for old@ creates a different account.
  - `theConfirmingBrowserStaysSignedInWhenItWasAlreadySignedIn` — confirm on the same session that requested → 204, `GET /api/account` with the session returned by the confirm request → `signedIn` true, `email` new@, and its id differs from the original session id.
  - `confirmingNeedsCsrfEvenForAGuest` — confirm without `csrf()` and without a session → 403; token still pending.
  - `aSpentLinkAnswers400` — second confirm → 400 with the message.
  - `aStaleSignInMustReauthenticate` — `SIGNED_IN_AT` set 10 minutes back → 403 `reauthenticate`, no mail.
  - `guestsAndProviderAccountsCannotChange` — guest → 401; an OAuth session (`SecurityMockMvcRequestPostProcessors.oauth2Login()` with an account created by `AccountService.signIn("google", "sub", Locale.ENGLISH)`) → 403.
  - `theConfirmationViewIsServed` — `GET /settings/email?token=x` → forwarded to `/index.html`.

- [ ] **Step 2: Run** `./gradlew test --tests '*EmailChangeIntegrationTest'` — Expected: FAIL.

- [ ] **Step 3: Implement.** Map `RequestResult` to the statuses above. Confirm: on `Confirmed`, `HttpSession session = request.getSession(true); request.changeSessionId();` (the service already invalidated the account's sessions, so `getSession(true)` yields a fresh one when this browser was one of them); build `OneTimeTokenAuthentication` from `new EmailUserDetailsService().loadUserByUsername(newEmail)` with its authorities; set it on a new `SecurityContext` in `SecurityContextHolder` and save it with a `new HttpSessionSecurityContextRepository()`; then call `signInSuccessHandler.onAuthenticationSuccess(request, response, authentication)`, which records the account on the session, logs `SIGN_IN` and answers 204. Add `"/settings/email"` to `HomeController`'s mapped paths and the confirm path to `LINK_SIGN_IN`.

- [ ] **Step 4: Run** the test, then `./gradlew test` — Expected: all PASS.

- [ ] **Step 5: Commit** `feat(account): change the email address from the API (US-079)`

---

### Task 4: Settings panel, confirmation view and privacy policy

**Files:**
- Modify: `src/main/resources/static/index.html` (an `#account-email` settings panel before `#your-data`; a `data-view="email-change"` view; privacy policy lines 426 and 429)
- Modify: `src/main/resources/static/app.js` (`pathToView` gets `"/settings/email": "email-change"`; panel and view logic)
- Modify: `src/main/resources/static/i18n.js`, `src/main/resources/static/i18n-nl.js`, `src/main/resources/static/styles.css` (only if the panel needs it; reuse `settings-panel`, `your-data-actions`, `form-message`)
- Test: `tests/application/AccountUiTest`, `tests/application/PrivacyPageUiTest`

**Interfaces:**
- Consumes: the endpoints and `email` field of Task 3; existing `withCsrfToken`, `openSignInPanel(element)`, `afterSignInKey`, `continueAfterSignIn`, `loadAccount`, `t`.

- [ ] **Step 1: Write the failing UI tests** (string checks, as the existing ones):
  - `AccountUiTest.theAccountPanelIsForEmailPlayersOnly` — `index.html` contains `id="account-email"` with `hidden`, `id="email-change-confirm"`; `app.js` contains `account.email`, `"/api/account/email"`, `"/api/account/email/confirm"`, `"/settings/email"`.
  - `AccountUiTest.theEmailChangeTextsAreTranslated` — each new English string below has a key in `i18n.js` and `i18n-nl.js`.
  - `PrivacyPageUiTest` — the policy mentions "confirm a change of address" and lists "address changes" among security events, in all three languages.

- [ ] **Step 2: Run** `./gradlew test --tests '*UiTest'` — Expected: FAIL.

- [ ] **Step 3: Implement.**
  - Panel (`#account-email`, hidden unless `account.email`): heading "Account"; "You sign in with {0}." (the current address); a field labelled "New email address" (`type="email"`, `autocomplete="email"`); button "Send a confirmation link"; `role="status"` message; `#email-sign-in` container for re-sign-in.
  - Submit: `POST /api/account/email` with `new URLSearchParams({ email, language })`; 204 → "Check your new mailbox. Your current address keeps working until you open the link."; 403 `reauthenticate` → "For your safety, sign in again to change your address.", store `"/settings"` under `afterSignInKey`, `openSignInPanel(#email-sign-in)`; other errors → `t(body.message ?? "Something went wrong. Please try again.")`. `continueAfterSignIn` returns to `/settings` when that is the stored target.
  - Confirmation view: on entering it, keep the `token` in a variable and `history.replaceState(null, "", "/settings/email")`, as `prepareLinkSignIn` does. Button "Confirm my new address" posts `{ token, language }` with `withCsrfToken`; 204 → "Your address is now confirmed. Your other sessions have been signed out." and `loadAccount()`; 400 or a missing token → "This link no longer works. Ask for a new one in Settings."; otherwise "Something went wrong. Please try again."
  - Privacy policy: line 426 becomes "…used only to send you sign-in links, to confirm a change of address and to tell your old address about it, never shown to other players…"; line 429's list adds "address changes". Update the French and Dutch translations of both sentences.

- [ ] **Step 4: Run** `./gradlew test` — Expected: all PASS.

- [ ] **Step 5: Check in a headless browser** with the app running and Mailpit (`docker compose up -d`): sign in by link, change the address from Settings, open the Mailpit link, confirm; the old tab is signed out on reload; the old mailbox received the notice; switch to French and Dutch and check the panel texts. Run the browser headless.

- [ ] **Step 6: Commit** `feat(settings): change the email address from Settings (US-079)`

---

### Task 5: Backlog and wrap-up

**Files:**
- Modify: `docs/product-backlog.md` (US-079 gets "Delivered: design in `docs/superpowers/specs/2026-10-15-change-email-design.md`."; the phase-5 sentence at line ~1117 lists US-079 among delivered stories)

- [ ] **Step 1: Update the backlog.**
- [ ] **Step 2: Run** `./gradlew build` — Expected: BUILD SUCCESSFUL, all tests green; note the test count.
- [ ] **Step 3: Commit** `docs: record US-079 as delivered`
