# Passkeys only (US-084) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Players sign in with exactly one passkey; email-link sign-in, Google, GitHub and SMTP are removed and every existing account is deleted.

**Architecture:** Spring Security's `webAuthn()` handles passkey sign-in, with its JDBC repositories on MariaDB and one `WebAuthnRelyingPartyOperations` bean shared with Beelot's own sign-up endpoints. Spring's registration endpoints need a signed-in user, so sign-up is a Beelot controller that calls the same bean for a not-yet-created account, then signs the browser in. The WebAuthn user entity's `name` is the account id, so `WebAuthnAuthentication.getName()` finds the account; no extra column is needed.

**Tech Stack:** Spring Boot 4.1.1, Spring Security 7.1.1 (`spring-security-webauthn`, webauthn4j-core 0.31.9), MariaDB with Flyway, Testcontainers, `com.webauthn4j:webauthn4j-test` as a software authenticator in tests, vanilla JS client, Playwright (headless) for browser checks.

**Spec:** `docs/superpowers/specs/2026-10-19-passkeys-only-design.md`

## Global Constraints

- Exactly one passkey per account, created at sign-up; never added, replaced or removed later.
- No recovery of any kind; sign-up warns before the passkey is created: losing the passkey loses the account, with no recovery.
- Registration requires a resident key (discoverable credential) and user verification.
- WebAuthn user entity: `name` = account id (UUID string), `displayName` = the pseudonym at sign-up; the user handle is random (`Bytes.random()`).
- Relying party id = host of `BEELOT_PUBLIC_URL` (`beelot.public-url`); allowed origin = that URL's scheme, host and port (port omitted when it is the scheme's default).
- Sessions unchanged: 7 days idle, 30 days at most; a sign-in changes the session id and rotates the CSRF token.
- No email address, no Google, no GitHub, no SMTP anywhere in code, configuration, pages, messages or tests once done.
- Every user-visible string in English, French and Dutch.
- Data export never contains the public key or the credential id.
- Security events never contain a credential id or challenge.
- Logs and responses for a failed sign-in never say why it failed.

## Review Focus

- The player cancels the browser's passkey dialog during sign-up: no account exists, and "Create an account" works again (Task 2 test `cancelledSignUpCreatesNoAccount`).
- The sign-up finish request is replayed: the second one is refused with 400 and creates nothing (Task 2 test `finishTwiceIsRefused`).
- A signed-in player calls Spring's own `/webauthn/register/options`: refused, so no second passkey (Task 3 test `registeringASecondPasskeyIsRefused`).
- A deleted player's authenticator still offers the old passkey: sign-in fails with the generic 401 (Task 5 test `deletedAccountPasskeyFails`).
- The re-sign-in before deletion offers only the signed-in account's passkey, so a shared device can't switch to, and delete, another account (Task 3 test `reSignInOffersOnlyTheAccountsPasskey`).

## File map

- `src/main/resources/db/migration/V8__passkeys_only.sql`: the clean break and the WebAuthn tables (Task 1).
- `account/RelyingParty`: rp id and origin from the public URL (Task 2).
- `security/WebAuthnConfiguration`: relying party operations, JDBC repositories, the `UserDetailsService` and the WebAuthn JSON mapper (Tasks 2, 3).
- `account/PasskeySignUpController`: `POST /api/sign-up/options`, `POST /api/sign-up` (Task 2).
- `account/SignInSuccessHandler`: one place that starts a signed-in session, for sign-up and sign-in (Tasks 2, 3).
- `account/PasskeyRateLimiter`, `account/PasskeyRequestFilter`: per-IP limits, renamed from `SignInRateLimiter` and `SignInRequestFilter` (Task 4).
- Privacy classes in `privacy/` (Task 5); client files in `static/` (Task 6); docs (Task 7).
- Test helper `tests/application/account/SoftwarePasskey` (Task 2), used by Tasks 2 to 5.

Paths below under `account/`, `security/`, `privacy/` are relative to `application/` (and `tests/application/` for tests).

---

### Task 1: Remove email, OAuth and SMTP, and migrate to V8

The branch has no sign-in method between this task and Task 3, but tests sign in with `PasskeySignIn`; the build and suite stay green.

**Files:**
- Create: `src/main/resources/db/migration/V8__passkeys_only.sql`
- Create: `account/PasskeysMigrationTest` (replaces `EmailAccountsMigrationTest`, same style: Flyway only, no Spring context)
- Delete (main): `SignInEmail`, `SignInLinkSender`, `SignInTokenService`, `Tokens`, `EmailUserDetailsService`, `EmailAddresses`, `AccountEmail`, `AccountEmailRepository`, `EmailChangeController`, `EmailChangeService`, `EmailChangeEmail` (all in `account/`), `privacy/InactivityWarningEmail`
- Delete (test): `EmailAccountsMigrationTest`, `EmailAddressesTest`, `EmailChange*Test` (4), `EmailSignInIntegrationTest`, `SignInEmailTest`, `SignInLinkSenderTest`, `SignInTokenServiceTest`, `security/ClientRegistrationsTest`, `account/NotConfiguredAccountTest`
- Keep for Task 4 (do not delete): `SignInRateLimiter`, `SignInRequestFilter` and their tests; unwire them from `SecurityConfiguration` now.
- Create (test): `account/PasskeySignIn`
- Modify: `account/Account`, `AccountRepository`, `AccountService`, `AccountController`, `SignInSuccessHandler`, `security/SecurityConfiguration`, `privacy/AccountDeletionService`, `privacy/RetentionJob`, `privacy/SecurityEvents`, `privacy/DataExport*`, `privacy/PrivacyController`, `privacy/PrivacyProperties`, `HomeController` (drop `/settings/email`, `/sign-in`), `build.gradle` (also add `implementation 'org.springframework.security:spring-security-webauthn'`), `application.properties`, `compose.yaml`, `WebSliceTestConfiguration`, and every test that creates accounts with `accounts.signIn("google", …)` or authenticates with `oauth2Login()`/`OneTimeTokenAuthentication`

**Interfaces:**
- Produces:
  - `Account(String displayName, Instant now)` (package-private), plus `Account(UUID id, String displayName, Instant now)` for sign-up; no `provider()`, `providerSubject()`, `deletionWarnedAt()`.
  - `AccountService`: `public Account create(Locale locale)` (new account, random pseudonym; tests use it in place of `signIn("google", …)`), `public Optional<Account> signIn(UUID accountId)` (records `lastSignInAt`/`previousSignInAt`), `public Optional<Account> current(Authentication)` (the account named by a `WebAuthnAuthentication` whose name is a UUID; empty for anything else, and a non-UUID name is not an exception), `displayName(UUID)`, `currentId(Authentication)`. `signInWithEmail` and `email` go.
  - `/api/account` view: `record AccountView(boolean signedIn, String name, String lastSignInAt)`.
  - Test helper `PasskeySignIn.as(UUID accountId): RequestPostProcessor`: puts a `WebAuthnAuthentication` (user entity named with the account id, `ROLE_USER`) in the request's security context, and sets `SecurityEvents.ACCOUNT_ID` and `AccountSessions.SIGNED_IN_AT` (now) in its session.
  - `SecurityEvents.Type { SIGN_IN, SIGN_OUT, SESSION_EXPIRED, RATE_LIMITED, EXPORT, DELETION }`.
  - `PrivacyProperties(String controller, String contact, String hostingProvider, String backupRetentionDays)`; `PolicyInfo` likewise loses `smtpProvider`.

- [ ] **Step 1: Write the failing migration test** `PasskeysMigrationTest.v8DeletesEveryAccountAndKeepsMatches`: migrate to target `"7"`; insert two accounts (one `google`, one `email` with an `account_email`, a `sign_in_token` and an `email_change_token`), a match whose seat 0 is the email account (name `Ann`) and seat 1 a bot, security events for each account plus one anonymous `LINK_REQUESTED` and one anonymous `DELETION`. Migrate fully. Assert:

```
assertEquals(0, count(s, "SELECT COUNT(*) FROM account"));
assertEquals(1, count(s, "SELECT COUNT(*) FROM match_record"));
assertEquals(1, count(s, "SELECT COUNT(*) FROM match_seat WHERE position = 0 AND account_id IS NULL AND name = 'Deleted player'"));
assertEquals(1, count(s, "SELECT COUNT(*) FROM security_event"));            // only the anonymous DELETION
for (String table : List.of("account_email", "sign_in_token", "email_change_token"))
    assertEquals(0, count(s, "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = '" + table + "'"));
for (String column : List.of("provider", "provider_subject", "deletion_warned_at"))
    assertEquals(0, count(s, "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = 'account' AND column_name = '" + column + "'"));
assertEquals(2, count(s, "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name IN ('user_entities', 'user_credentials')"));
```

Second test `v8AllowsOnePasskeyPerUser`: after migrating, insert one `user_entities` row and two `user_credentials` rows with the same `user_entity_user_id`; the second insert throws `SQLIntegrityConstraintViolationException`.

- [ ] **Step 2: Run it to verify it fails**

Run: `./gradlew test --tests '*PasskeysMigrationTest'`
Expected: FAIL, the accounts are still there.

- [ ] **Step 3: Write `V8__passkeys_only.sql`**

In order: seats of any account get `account_id = NULL, name = 'Deleted player'` (the value of `FinishedMatch.DELETED_PLAYER`); delete security events with an account, and anonymous events of type `LINK_REQUESTED`, `EMAIL_CHANGE_REQUESTED`, `EMAIL_CHANGED`; drop `email_change_token`, `sign_in_token`, `account_email`; delete every account; drop the unique key `account_provider_subject_unique` and the columns `provider`, `provider_subject`, `deletion_warned_at`. Then Spring's two tables (from `org/springframework/security/user-*-schema.sql` in `spring-security-web`), in MariaDB types sized for InnoDB keys:

```sql
CREATE TABLE user_entities (
    id           VARCHAR(100) CHARACTER SET ascii NOT NULL PRIMARY KEY,  -- user handle, base64url
    name         VARCHAR(100) NOT NULL,                                  -- the account id
    display_name VARCHAR(200),
    created_at   DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),     -- Beelot's: purges abandoned sign-ups
    CONSTRAINT user_entities_name_unique UNIQUE (name)
);
CREATE TABLE user_credentials (
    credential_id                VARCHAR(1400) CHARACTER SET ascii NOT NULL PRIMARY KEY,
    user_entity_user_id          VARCHAR(100) CHARACTER SET ascii NOT NULL,
    public_key                   BLOB NOT NULL,
    signature_count              BIGINT,
    uv_initialized               BOOLEAN,
    backup_eligible              BOOLEAN NOT NULL,
    authenticator_transports     VARCHAR(1000),
    public_key_credential_type   VARCHAR(100),
    backup_state                 BOOLEAN NOT NULL,
    attestation_object           MEDIUMBLOB,
    attestation_client_data_json BLOB,
    created                      DATETIME(6),
    last_used                    DATETIME(6),
    label                        VARCHAR(1000) NOT NULL,
    CONSTRAINT user_credentials_one_per_user UNIQUE (user_entity_user_id),
    CONSTRAINT user_credentials_user_fk FOREIGN KEY (user_entity_user_id) REFERENCES user_entities (id) ON DELETE CASCADE
);
```

Head comment: US-084, the clean break, and that `db-update-v2-to-v7.sql` must have brought production to version 7 first.

- [ ] **Step 4: Run the migration test to verify it passes**

Run: `./gradlew test --tests '*PasskeysMigrationTest'`
Expected: PASS (2 tests).

- [ ] **Step 5: Remove the email, OAuth and SMTP code and settings**

Delete the files listed above. In `build.gradle` drop `spring-boot-starter-mail` and `spring-boot-starter-security-oauth2-client`. In `application.properties` drop the Google/GitHub block, `spring.mail.*`, `beelot.mail.from`, `beelot.sign-in.link-lifetime`, `beelot.sign-in.limit-per-address`, `beelot.privacy.smtp-provider`; keep `beelot.public-url` with the comment "the relying party of passkeys (US-084)" and keep `spring.config.import`. Drop Mailpit from `compose.yaml` and its header line. `SecurityConfiguration` keeps CSRF, headers, `SessionLifetimeFilter` and logout; it loses `oneTimeTokenLogin`, `oauth2Login`, the client registration bean and `Hints`, and its CSRF matcher becomes `!SAFE_METHODS.contains(method) && session exists` until Task 2. `SignInSuccessHandler` keeps only its private recording logic (it is rebuilt in Task 2). `AccountDeletionService` loses the `account_email` and `sign_in_token` statements. `RetentionJob` loses the warning: it deletes accounts with `last_sign_in_at <= now − 3 years` and purges events; constructor `RetentionJob(JdbcClient, AccountDeletionService, Clock)`. `DataExport.Account` loses `signIn` (Task 5 adds the passkey). Update the class comments that mention email, links, Google or GitHub.

- [ ] **Step 6: Fix the remaining tests**

Replace `accounts.signIn("google", "301", Locale.ENGLISH)` with `accounts.create(Locale.ENGLISH)`, and `oauth2Login()` or one-time-token sign-ins with `PasskeySignIn.as(account.id())`. `AccountServiceTest` gains `currentFindsAPasskeyAccount` and `currentIgnoresANonUuidName`. `RetentionJobIntegrationTest`: drop the warning cases; keep `deletesAccountsInactiveFor3Years` and `keepsAccountsSignedInWithin3Years`.

- [ ] **Step 7: Verify no reference is left and the suite passes**

Run: `grep -rniE "oauth2|smtp|mailpit|sign_in_token|account_email|EmailChange|signInWithEmail" src build.gradle compose.yaml` → no output.
Run: `./gradlew build`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 8: Commit**

```bash
git add -A src build.gradle compose.yaml
git commit -m "feat(account): remove email, Google and GitHub sign-in, migrate to V8 (US-084)"
```

---

### Task 2: Relying party and passkey sign-up

**Files:**
- Modify: `build.gradle` (add `testImplementation 'com.webauthn4j:webauthn4j-test:0.31.9.RELEASE'`)
- Create: `account/RelyingParty`, `security/WebAuthnConfiguration`, `account/PasskeySignUpController`
- Modify: `account/AccountService`, `account/SignInSuccessHandler`, `security/SecurityConfiguration`, `privacy/SecurityEvents`
- Create (test): `account/SoftwarePasskey`, `account/RelyingPartyTest`, `account/PasskeySignUpIntegrationTest`

**Interfaces:**
- Consumes: `AccountService.create`/`signIn` (Task 1), the V8 tables.
- Produces:
  - `record RelyingParty(String id, String origin)` with `static RelyingParty of(String publicUrl)`.
  - Beans in `WebAuthnConfiguration`: `PublicKeyCredentialUserEntityRepository` (`JdbcPublicKeyCredentialUserEntityRepository`), `UserCredentialRepository` (`JdbcUserCredentialRepository`), `WebAuthnRelyingPartyOperations` (`Webauthn4JRelyingPartyOperations` with rp entity name `Beelot`, the rp id, `Set.of(origin)`, and `setCustomizeCreationOptions` requiring `ResidentKeyRequirement.REQUIRED` and `UserVerificationRequirement.REQUIRED`), and `@Bean("webAuthnJson") JsonMapper` built with `new WebauthnJacksonModule()`.
  - `AccountService.register(PublicKeyCredentialCreationOptions options, RelyingPartyPublicKey key): Account`, `@Transactional`: `relyingParty.registerCredential(new ImmutableRelyingPartyRegistrationRequest(options, key))`, then saves `new Account(UUID.fromString(options.getUser().getName()), options.getUser().getDisplayName(), now)`. Either both are stored or neither.
  - `SignInSuccessHandler.startSession(HttpServletRequest, HttpServletResponse, WebAuthnAuthentication, Account)`: `request.changeSessionId()` (creating the session if needed), CSRF rotation with `CsrfAuthenticationStrategy(csrfTokens)`, `HttpSessionSecurityContextRepository.saveContext`, `ACCOUNT_ID` attribute, `AccountSessions.signedIn`, `SIGN_IN SUCCESS` event, status 204. Move this logic from the deleted `EmailChangeController`'s pattern.
  - `SecurityEvents.Type` gains `SIGN_UP` (first in the enum).
  - Endpoints: `POST /api/sign-up/options` (form param `language`) → 200, creation options JSON; `POST /api/sign-up` (body: `RelyingPartyPublicKey` JSON, `{"credential": {...}, "label": "Beelot"}`) → 204 signed in, or 400.
  - Test helper `SoftwarePasskey` (wraps webauthn4j-test `ClientPlatform` for origin `http://localhost`): `String register(String creationOptionsJson)` returns the `RelyingPartyPublicKey` JSON for `/api/sign-up`; `String signIn(String requestOptionsJson)` returns the assertion JSON for `/login/webauthn` (Spring's format: `id`, `rawId`, `response.{authenticatorData, clientDataJSON, signature, userHandle}`, `credType`, `clientExtensionResults`, `authenticatorAttachment`, all base64url). One instance holds one key, so a test can sign in twice with it.

- [ ] **Step 1: Write the failing tests**

`RelyingPartyTest.relyingPartyFromPublicUrl`:

```
assertEquals(new RelyingParty("localhost", "http://localhost:8080"), RelyingParty.of("http://localhost:8080"));
assertEquals(new RelyingParty("beelot.fr", "https://beelot.fr"), RelyingParty.of("https://beelot.fr/"));
assertEquals(new RelyingParty("beelot.fr", "https://beelot.fr"), RelyingParty.of("https://beelot.fr:443"));
assertThrows(IllegalArgumentException.class, () -> RelyingParty.of("beelot.fr"));
```

`PasskeySignUpIntegrationTest` (`@SpringBootTest(properties = "beelot.public-url=http://localhost")`, MockMvc, MariaDB container like the other integration tests), each request with `csrf()` and a shared `MockHttpSession`:
- `signUpCreatesOneAccountAndSignsIn`: options → `$.authenticatorSelection.residentKey == "required"`, `$.authenticatorSelection.userVerification == "required"`, `$.rp.id == "localhost"`, `$.user.displayName` is a pseudonym; finish with `passkey.register(options)` → 204; then `GET /api/account` → `$.signedIn == true`, `$.name` equals that `displayName`; one `account` row, one `user_credentials` row, one `SIGN_UP SUCCESS` and one `SIGN_IN SUCCESS` event for the account.
- `cancelledSignUpCreatesNoAccount`: options, then no finish; a new options call in the same session → 200 and a different `$.challenge`; `account` count 0.
- `finishTwiceIsRefused`: finish once (204), replay the same body → 400; `account` and `user_credentials` counts stay 1.
- `finishWithoutOptionsIsRefused`: finish on a fresh session → 400, `SIGN_UP FAILURE` recorded with no account.
- `wrongOriginIsRefused`: a `SoftwarePasskey` for origin `http://evil.example` → 400, no account.
- `signUpNeedsTheCsrfToken`: options without `csrf()` → 403.
- `signUpChangesTheSessionId`: the session id before finish differs from the one after.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*RelyingPartyTest' --tests '*PasskeySignUpIntegrationTest'`
Expected: FAIL to compile (`RelyingParty`, `SoftwarePasskey` missing).

- [ ] **Step 3: Implement `RelyingParty`, `WebAuthnConfiguration`, `AccountService.register`, `SignInSuccessHandler.startSession` and `PasskeySignUpController`**

Options endpoint: new `UUID`, pseudonym from `PseudonymGenerator` in the requested language (move `SignInEmail.language(request)` logic into `PseudonymGenerator.language(HttpServletRequest)`); save `ImmutablePublicKeyCredentialUserEntity.builder().name(id.toString()).id(Bytes.random()).displayName(pseudonym).build()` first, so Spring's `findUserEntityOrCreateAndSave` finds it; call `createPublicKeyCredentialCreationOptions(new ImmutablePublicKeyCredentialCreationOptionsRequest(UsernamePasswordAuthenticationToken.authenticated(id.toString(), null, List.of())))`; keep the options in `HttpSessionPublicKeyCredentialCreationOptionsRepository`; write them with `webAuthnJson`. Finish endpoint: load the options and remove them from the session before verifying (single use); none → 400; read the body with `webAuthnJson`; any exception from `register` → 400 and `SIGN_UP FAILURE`; success → `SIGN_UP SUCCESS`, then `startSession` with `new WebAuthnAuthentication(options.getUser(), AuthorityUtils.createAuthorityList("ROLE_USER"))`. In `SecurityConfiguration`, CSRF is also required on `POST /api/sign-up/options` and `POST /api/sign-up` without a session.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `./gradlew test --tests '*RelyingPartyTest' --tests '*PasskeySignUpIntegrationTest'`
Expected: PASS (1 + 7 tests).

- [ ] **Step 5: Commit**

```bash
git add -A build.gradle src
git commit -m "feat(account): sign up with a passkey (US-084)"
```

---

### Task 3: Passkey sign-in

**Files:**
- Modify: `security/SecurityConfiguration`, `security/WebAuthnConfiguration`, `account/SignInSuccessHandler`, `account/AccountController`
- Create (test): `account/PasskeySignInIntegrationTest`

**Interfaces:**
- Consumes: Task 2's beans, `startSession`, `SoftwarePasskey`.
- Produces: `POST /webauthn/authenticate/options` → request options; `POST /login/webauthn` → 204 signed in, or 401 with an empty body. `WebAuthnConfiguration` also defines `UserDetailsService` that returns `User.withUsername(name).password("").authorities("ROLE_USER")` when `name` is the id of an existing account, and throws `UsernameNotFoundException` otherwise.

- [ ] **Step 1: Write the failing tests** in `PasskeySignInIntegrationTest` (sign up first with `SoftwarePasskey`, then sign out):
- `signInWithThePasskey`: options → `$.allowCredentials` empty (discoverable), `$.userVerification == "required"`; `/login/webauthn` with `passkey.signIn(options)` → 204; `/api/account` → signed in as the same account; `previous_sign_in_at` is now the sign-up time; session id changed; a `SIGN_IN SUCCESS` event.
- `unknownPasskeyFails`: a second `SoftwarePasskey` that never signed up → 401, empty body, `SIGN_IN FAILURE` with no account.
- `counterGoingBackwardsFails`: `UPDATE user_credentials SET signature_count = 1000000`; sign-in → 401 and `SIGN_IN FAILURE`.
- `registeringASecondPasskeyIsRefused`: signed in, `POST /webauthn/register/options` with `csrf()` → 403; `POST /webauthn/register` → 403; `DELETE /webauthn/register/x` → 403; still one `user_credentials` row.
- `reSignInOffersOnlyTheAccountsPasskey`: signed in, options → `$.allowCredentials.length() == 1` and its `id` equals the stored `credential_id`.
- `signInNeedsTheCsrfToken`: options without `csrf()` → 403.
- `springDefaultPagesAreOff`: `GET /login` and `GET /webauthn/register` do not return Spring's generated HTML (the SPA forward or 404/403, as today for `/login`).

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*PasskeySignInIntegrationTest'`
Expected: FAIL, `/webauthn/authenticate/options` is not mapped.

- [ ] **Step 3: Configure `webAuthn()`**

In `SecurityConfiguration`: `.webAuthn(webAuthn -> webAuthn.rpName("Beelot").rpId(rp.id()).allowedOrigins(rp.origin()).disableDefaultRegistrationPage(true).withObjectPostProcessor(...))`, the post-processor setting on `WebAuthnAuthenticationFilter` a success handler that calls `SignInSuccessHandler.onAuthenticationSuccess` (records the sign-in times via `AccountService.signIn`, session attributes, `SIGN_IN SUCCESS`, 204; the filter has already changed the session id) and a failure handler recording `SIGN_IN FAILURE` with no account and answering 401. `authorizeHttpRequests`: `/webauthn/register/**` and `/webauthn/register` `denyAll()` before `anyRequest().permitAll()`. CSRF is required on the two WebAuthn POSTs without a session. `AccountController.signedIn(...)` checks `instanceof WebAuthnAuthentication`.

- [ ] **Step 4: Run the full suite**

Run: `./gradlew test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(account): sign in with a passkey (US-084)"
```

---

### Task 4: Rate limits on sign-up and sign-in

**Files:**
- Rename: `account/SignInRateLimiter` → `account/PasskeyRateLimiter`, `account/SignInRequestFilter` → `account/PasskeyRequestFilter`, and their tests (`git mv`)
- Modify: `security/SecurityConfiguration`, `application.properties`, `WebSliceTestConfiguration`

**Interfaces:**
- Produces: `PasskeyRateLimiter(Clock, @Value("${beelot.passkey.limit-per-ip}") int limitPerIp)` with `boolean tryAcquire(String ip)`, one fixed 1-hour window per IP, same bounded map as today. `PasskeyRequestFilter(PasskeyRateLimiter, SecurityEvents)` matches `POST /api/sign-up/options` and `POST /webauthn/authenticate/options`; above the limit it answers 429 with `Retry-After: 3600` and records `RATE_LIMITED REFUSED` with the IP and no account. Property `beelot.passkey.limit-per-ip=30`, comment "ceremonies started per client IP per hour (US-084)".

- [ ] **Step 1: Adapt the renamed tests** `PasskeyRateLimiterTest`: `allowsUpToTheLimitPerIp` (30 then refused), `anotherIpIsNotAffected`, `windowResetsAfterAnHour` (clock + 1 h → allowed), `refusedRequestsDoNotCount`, `boundsTrackedIps` (kept from today). `PasskeyRequestFilterTest`: `refusesAboveTheLimitWith429`, `recordsTheRefusal`, `otherPathsPassThrough` (`POST /login/webauthn` and `POST /api/sign-up` are never limited, so a ceremony started is always finished).

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*PasskeyRateLimiterTest' --tests '*PasskeyRequestFilterTest'`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**, dropping the per-address map and the email parsing; add the filter before `CsrfFilter` in `SecurityConfiguration`, so a refused request costs nothing more. Drop `beelot.sign-in.limit-per-ip`.

- [ ] **Step 4: Run the full suite**

Run: `./gradlew test`
Expected: PASS (integration tests that sign up repeatedly set `beelot.passkey.limit-per-ip=1000`).

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(account): limit passkey ceremonies per IP address (US-084)"
```

---

### Task 5: Privacy with passkeys: deletion, export, retention, events

**Files:**
- Modify: `privacy/AccountDeletionService`, `privacy/DataExport`, `privacy/DataExportService`, `privacy/RetentionJob`, `privacy/SecurityEvents` (class comment only)
- Test: `privacy/AccountDeletionIntegrationTest`, `privacy/DataExportIntegrationTest`, `privacy/RetentionJobIntegrationTest`, `account/PasskeySignInIntegrationTest`

**Interfaces:**
- Produces: `DataExport.FORMAT = "beelot-export-2"`; `DataExport.Account(String id, String displayName, String createdAt, String lastSignInAt, String previousSignInAt, Passkey passkey)`; `record Passkey(String createdAt, String lastUsedAt)`.

- [ ] **Step 1: Write the failing tests**
- `AccountDeletionIntegrationTest.deletionRemovesThePasskey`: an account signed up with `SoftwarePasskey`, deleted → no `user_entities` row with its name, no `user_credentials` row.
- `PasskeySignInIntegrationTest.deletedAccountPasskeyFails`: sign up, delete (`DELETE /api/account` within 10 minutes), sign in with the same `SoftwarePasskey` → 401, empty body.
- `PasskeySignInIntegrationTest.deletionAfterTenMinutesNeedsAFreshSignIn`: clock + 11 min → `DELETE /api/account` → 403 `{"reason":"reauthenticate"}`; sign in again with the passkey → `DELETE` → 204.
- `DataExportIntegrationTest.exportShowsThePasskeyDatesOnly`: `$.format == "beelot-export-2"`, `$.account.passkey.createdAt` and `lastUsedAt` set; the raw JSON contains neither the stored `credential_id` nor `public_key` in base64 or base64url, nor the word `email`.
- `RetentionJobIntegrationTest.purgesOrphanUserEntities`: a `user_entities` row with no account and `created_at` 2 days ago is deleted; one created 1 hour ago is kept; one with an account is kept.
- `RetentionJobIntegrationTest.deletesAccountsInactiveFor3YearsWithTheirPasskey`.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*AccountDeletionIntegrationTest' --tests '*DataExportIntegrationTest' --tests '*RetentionJobIntegrationTest' --tests '*PasskeySignInIntegrationTest'`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

Deletion, inside its transaction: `DELETE FROM user_entities WHERE name = ?` (credentials go by cascade). Export: `created` and `last_used` read with `getTimestamp(...).toInstant()` (Spring writes them as `Timestamp`, not as UTC local times like Beelot's own columns). Retention: `DELETE FROM user_entities e WHERE e.created_at < now − 1 day AND NOT EXISTS (SELECT 1 FROM account a WHERE a.id = e.name)` after the account deletions, logged like the event purge. `SecurityEvents` comment lists sign-ups, sign-ins, failed sign-ins, sign-outs, expiries, rate-limit refusals, exports and deletions, and says no credential id or challenge is ever recorded.

- [ ] **Step 4: Run the tests to verify they pass**, same command. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(privacy): delete, export and retain passkey accounts (US-084)"
```

---

### Task 6: Client: passkey sign-up, sign-in and deletion

**Files:**
- Modify: `src/main/resources/static/index.html`, `app.js`, `i18n.js`, `i18n-nl.js`
- Test: `AccountUiTest`, `PrivacyPageUiTest`, `FrenchTranslationUiTest`, `DutchTranslationUiTest`

**Interfaces:**
- Consumes: Task 2 and Task 3 endpoints; `/api/account` → `{signedIn, name, lastSignInAt}`; Spring's JSON formats as in `org/springframework/security/spring-security-webauthn.js` inside `spring-security-web-7.1.1.jar` (copy its base64url encode/decode and body shapes, not the file).

Copy (English source strings, translated in both `i18n` files):
- `"Sign in with a passkey"`, `"Create an account"`
- Warning, shown before the passkey is created: `"Your passkey is the only way to sign in. If you lose it, you lose your account: there is no recovery."` and the confirm button `"I understand, create my passkey"`
- `"Sign-in did not complete. Please try again."` (kept), `"Your browser does not support passkeys. You can still play as a guest."`
- Too many attempts: `"Too many attempts. Please try again later."`

- [ ] **Step 1: Update the UI tests to the new client**

`AccountUiTest`: `theHomeScreenOffersPasskeys` (`id="passkey-sign-in"`, `id="passkey-sign-up"`, `id="passkey-warning"` in `index.html`; `app.js` contains `"/api/sign-up/options"`, `"/api/sign-up"`, `"/webauthn/authenticate/options"`, `"/login/webauthn"`, `navigator.credentials.create`, `navigator.credentials.get`); `noEmailOrProviderLeft` (`index.html`, `app.js`, `i18n.js`, `i18n-nl.js` contain none of `sign-in-email`, `/api/sign-in/link`, `/login/ott`, `email-change`, `account.providers`, `Continue with {0}`, `inbox`); `deletionSignsInAgainWithThePasskey` (`app.js` handles `"reauthenticate"` by calling the passkey sign-in, not `openSignInPanel`). `PrivacyPageUiTest`: no `privacy-smtp`, no `Google`, no `GitHub`, no `email address`; mentions passkeys. Translation tests: every new string present in FR and NL.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*UiTest'`
Expected: FAIL.

- [ ] **Step 3: Implement the client**

Sign-in panel: two buttons and the hidden warning; "Create an account" reveals the warning, its confirm button runs the sign-up ceremony. `window.PublicKeyCredential` missing → the panel shows only the unsupported message. A `NotAllowedError`/`AbortError` (cancelled) or a 400/401 shows "Sign-in did not complete…"; 429 shows "Too many attempts…". After a 204 the client reloads `/api/account`. Deletion: on 403 `reauthenticate`, run the passkey sign-in, then re-show the confirmation with the reloaded account name; remove `afterSignInKey` handling that only served links. Remove the `sign-in` and `email-change` views, `prepareLinkSignIn`, `prepareEmailChange` and their paths, and the Settings address section. Privacy page: hosting is the only processor; Beelot stores no email address; a passkey's public key, signature counter and dates are stored; the session and CSRF cookies are still the only cookies.

- [ ] **Step 4: Run the UI tests to verify they pass**

Run: `./gradlew test --tests '*UiTest'`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(client): sign up and sign in with a passkey (US-084)"
```

---

### Task 7: Documentation

**Files:**
- Create: `docs/adr/005-passkeys-only.md`
- Modify: `docs/adr/003-identity-provider.md`, `docs/adr/004-email-link-sign-in.md` (status lines), `docs/product-backlog.md`, `README.md`, `docs/privacy/data-breach-procedure.md` (if it mentions email addresses or SMTP), `AGENTS.md` only if it states sign-in facts

- [ ] **Step 1: Write ADR-005** in the style of ADR-004: context (email needs an SMTP processor; providers add processors), decision (the spec's Decisions section, short), consequences (no recovery, one passkey, lost device without sync = new account, all accounts deleted by V8). Status "Accepted; supersedes ADR-004". ADR-004 status: "Superseded by ADR-005"; ADR-003 status line mentions ADR-005.

- [ ] **Step 2: Update the backlog**: US-084 rewritten to the spec (one passkey, passkey only, no recovery); US-075, US-077 and US-079 marked "Superseded by US-084"; US-081 to US-083 lose email, Google and GitHub mentions; the decision "Choose the SMTP provider for production" struck through with "not needed since US-084".

- [ ] **Step 3: Update the README**: remove SMTP, Mailpit, Google and GitHub setup and their variables; describe passkey sign-in, that development uses rp id `localhost` from the default `BEELOT_PUBLIC_URL`, and that production needs `BEELOT_PUBLIC_URL` set to the public HTTPS address before the first sign-up (changing the host later makes every passkey unusable).

- [ ] **Step 4: Check**

Run: `grep -rniE "smtp|mailpit|google|github|sign-in link|email link" README.md docs/adr/005-passkeys-only.md` → only historical mentions explained as removed.

- [ ] **Step 5: Commit**

```bash
git add README.md docs
git commit -m "docs: passkeys only (ADR-005, US-084)"
```

---

### Task 8: End-to-end checks

No new code unless a check fails; a failure goes back to the task that owns the code.

- [ ] **Step 1: Full build**

Run: `./gradlew clean build`
Expected: BUILD SUCCESSFUL; `grep -rliE "oauth|smtp|sign-in link|signInWithEmail" src/test` → no output.

- [ ] **Step 2: Headless browser** (Playwright, headless): start MariaDB (`docker compose up -d --wait`) and the app (`./gradlew bootRun`); in the page, add a CDP virtual authenticator (`WebAuthn.enable`, `WebAuthn.addVirtualAuthenticator` with `protocol: "ctap2"`, `transport: "internal"`, `hasResidentKey: true`, `hasUserVerification: true`, `isUserVerified: true`). Check in order: the warning shows before the passkey is created; sign-up → signed in with a pseudonym; sign out; "Sign in with a passkey" → same pseudonym; Settings → delete after waiting is not practical, so delete at once (recent sign-in) → signed out, and signing in again fails with the generic message. Repeat the sign-in in French and Dutch for the translated texts. With `isUserVerified: false`, the ceremony fails with the generic message.

- [ ] **Step 3: Native image** (US-031): `./gradlew nativeCompile`, run `build/native/nativeCompile/beelot` against the local MariaDB, and repeat sign-up and sign-in from Step 2. A reflection or serialization error is fixed with hints in `AccountRuntimeHints` (Spring ships `PublicKeyCredentialUserEntityRuntimeHints` and `UserCredentialRuntimeHints`; add only what is missing), then commit `fix(account): native hints for passkeys (US-084)`.

- [ ] **Step 4: Report** the commands run, the browser checks and the native result.
