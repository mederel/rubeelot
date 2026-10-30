# Confirm my email address (US-076) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expire Zitadel's email confirmation code after 24 hours, and delete through Zitadel's API every user left unconfirmed for 7 days who never signed in to Beelot.

**Architecture:** A thin `RestClient` client of Zitadel's user API v2 (`ZitadelUsers`, security package) feeds an hourly `@Scheduled` job (`UnconfirmedAccountCleanup`, account package) that skips users with an `account` row. `zitadel/provision.sh` sets the code expiry and creates the least-privileged `beelot-jobs` service account whose token Spring reads from `BEELOT_ZITADEL_API_TOKEN`.

**Tech Stack:** Java 21, Spring Boot 4.1 (Spring Framework 7 `RestClient`, Jackson 3 `tools.jackson`), Spring Data JPA, MariaDB in Testcontainers, `MockRestServiceServer`, bash + curl + jq, Zitadel v4.19.4 in Docker Compose, GraalVM native image.

**Spec:** `docs/superpowers/specs/2026-10-08-confirm-email-design.md`

## Global Constraints

- No new Gradle dependency. Build the client with `RestClient.builder()` (the project has no `RestClient.Builder` bean).
- Beelot never stores, logs or puts in an exception message a name, an email address, the API token or a Zitadel response body. Logs carry counts and user ids only.
- A user is deleted only if: human, `email.isVerified` false, `details.creationDate` before `now - beelot.account.unconfirmed-retention`, and no `account` row with `provider = 'zitadel'` and `provider_subject` = the user id.
- Properties: `beelot.zitadel.api-token=${BEELOT_ZITADEL_API_TOKEN:}`, `beelot.account.unconfirmed-retention=P7D`, `beelot.account.cleanup-interval=PT1H`; the job's initial delay is `PT1M`.
- No `@Conditional` on runtime configuration: the native image fixes conditions at build time. Beans always exist and check `ZitadelProperties.apiConfigured()` when they run.
- Records that Jackson binds must work in the native image: `@RegisterReflectionForBinding` on `ZitadelUsers`.
- Zitadel email code expiry: `"86400s"`. Service account: machine user `beelot-jobs`, role `ORG_USER_MANAGER` on its organization.
- Comments and Javadoc follow the existing style: short, plain English, story number in parentheses, e.g. `(US-076)`.
- Commit messages: conventional commits ending with `(US-076)` and the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Browser checks run headless.

## Review Focus

1. **Many users over several pages, with the cutoff falling mid-page.** Users after the cutoff on the same page must not be returned, and no further page should be requested. Pinned in Task 1 (`stopsAtTheFirstUserCreatedAfterTheCutoff`).
2. **A user without an `email` object or without `human`** (an IdP user with no address, or an odd response). The client must not throw a `NullPointerException` that aborts the whole run; it treats a missing `email` as "not unverified" and skips the user. Pinned in Task 1 (`skipsUsersWithoutAnEmail`).
3. **An empty candidate list.** The repository must not be queried with an empty `IN ()` (invalid SQL on MariaDB) and no delete happens. Pinned in Task 2 (`noCandidatesQueriesNothing`).
4. **A rejected or expired token (401).** Every run must log a warning without the token and delete nothing, and must not crash the scheduler thread. Pinned in Task 1 (`anErrorNeverShowsTheToken`) and Task 2 (`aFailedListingDeletesNothing`).
5. **An issuer with a trailing slash** (`http://localhost:8081/`). Requests must go to `/v2/users`, not `//v2/users`. Pinned in Task 1 (`theIssuerMayEndWithASlash`).

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/main/java/fr/beelot/application/security/ZitadelProperties.java` (modify) | Add `apiToken` and `apiConfigured()` |
| `src/main/java/fr/beelot/application/security/SecurityConfiguration.java` (modify) | Bind `beelot.zitadel.api-token`; expose `ZitadelUsers` and `Clock` beans |
| `src/main/java/fr/beelot/application/security/ZitadelUsers.java` (create) | List unconfirmed users, delete a user |
| `src/main/java/fr/beelot/application/security/ZitadelApiException.java` (create) | Failure of a Zitadel API call, without secrets |
| `src/main/java/fr/beelot/application/account/AccountRepository.java` (modify) | `findProviderSubjects` batch query |
| `src/main/java/fr/beelot/application/account/UnconfirmedAccountCleanup.java` (create) | The scheduled job |
| `src/main/resources/application.properties` (modify) | New properties |
| `zitadel/provision.sh` (modify) | Code expiry, `beelot-jobs`, token in `.env` |
| `README.md`, `docs/product-backlog.md` (modify) | Document the variable and the job; mark US-076 delivered |
| Tests: `security/ZitadelUsersTest.java`, `account/UnconfirmedAccountCleanupTest.java`, `account/AccountRepositoryIntegrationTest.java` (create); `ClientRegistrationsTest.java`, `RegisterAuthorizationRequestResolverTest.java` (modify for the new record component) | |

---

### Task 1: Zitadel user API client

**Files:**
- Modify: `src/main/java/fr/beelot/application/security/ZitadelProperties.java`
- Modify: `src/main/java/fr/beelot/application/security/SecurityConfiguration.java:49-54`
- Modify: `src/main/resources/application.properties` (Zitadel block)
- Create: `src/main/java/fr/beelot/application/security/ZitadelUsers.java`
- Create: `src/main/java/fr/beelot/application/security/ZitadelApiException.java`
- Test: `src/test/java/fr/beelot/application/security/ZitadelUsersTest.java`
- Modify tests constructing `ZitadelProperties`: `ClientRegistrationsTest.java`, `RegisterAuthorizationRequestResolverTest.java` (add `""` as the fourth argument)

**Interfaces:**
- Produces:
  - `public record ZitadelProperties(String issuer, String clientId, String clientSecret, String apiToken)` with `boolean configured()` (unchanged meaning) and `public boolean apiConfigured()` (issuer and apiToken set).
  - `public class ZitadelUsers`:
    - `public ZitadelUsers(ZitadelProperties zitadel)` → uses `RestClient.builder()`;
    - `ZitadelUsers(ZitadelProperties zitadel, RestClient.Builder builder)` (package-private, for tests);
    - `public List<String> unconfirmedCreatedBefore(Instant cutoff)`;
    - `public void delete(String userId)`.
  - `public class ZitadelApiException extends RuntimeException` with `public ZitadelApiException(String method, String path, int status)` (status 0 for an I/O failure); message `"Zitadel answered <status> to <method> <path>"`.
  - Beans in `SecurityConfiguration`: `ZitadelUsers zitadelUsers(ZitadelProperties)` and `Clock clock()` returning `Clock.systemUTC()`.

- [ ] **Step 1: Write the failing tests** in `ZitadelUsersTest`, binding `MockRestServiceServer.bindTo(builder).build()` to a `RestClient.builder()` and `new ZitadelUsers(new ZitadelProperties("https://zitadel.test", "", "", "api-token"), builder)`. Cutoff in every test: `Instant.parse("2026-10-01T00:00:00Z")`. Build user JSON with a helper `user(id, creationDate, Boolean verified)` (verified `null` → no `email` field).

```java
@Test void listsUnconfirmedHumansOldestFirst()
// expect POST https://zitadel.test/v2/users, header Authorization "Bearer api-token",
// jsonPath("$.query.offset")=0, "$.query.limit"=100, "$.query.asc"=true,
// "$.sortingColumn"="USER_FIELD_NAME_CREATION_DATE", "$.queries[0].typeQuery.type"="TYPE_HUMAN";
// respond with u1 (2026-09-01, false), u2 (2026-09-02, true), details.totalResult=2
// assert result == List.of("u1")

@Test void readsTheNextPage()
// page 1: 100 users created 2026-09-01, all unverified, totalResult=101; expect second request with offset 100
// page 2: u-last (2026-09-02, false); assert size 101 and last element "u-last"

@Test void stopsAtTheFirstUserCreatedAfterTheCutoff()
// one page, totalResult=500: u1 (2026-09-30, false), u2 (2026-10-01T00:00:00Z, false), u3 (2026-10-02, false)
// assert == List.of("u1"); server.verify() proves no second request

@Test void skipsUsersWithoutAnEmail()
// u1 (2026-09-01, null), u2 (2026-09-01, false); also a user object without "human" → assert == List.of("u2")

@Test void theIssuerMayEndWithASlash()
// properties issuer "https://zitadel.test/", expect requestTo("https://zitadel.test/v2/users")

@Test void deleteSendsTheUserId()
// expect DELETE https://zitadel.test/v2/users/u1 with the bearer header, respond 200 "{}"

@Test void deletingAMissingUserIsNotAnError()
// respond 404 → no exception

@Test void anErrorNeverShowsTheToken()
// list responds 401 with body {"message":"token api-token invalid for jane@example.com"}
// ZitadelApiException thrown; message == "Zitadel answered 401 to POST /v2/users";
// message contains neither "api-token" nor "jane"
// same for delete answering 500: message == "Zitadel answered 500 to DELETE /v2/users/u1"
```

- [ ] **Step 2: Run them to see them fail**

Run: `./gradlew test --tests 'fr.beelot.application.security.ZitadelUsersTest'`
Expected: compilation failure (`ZitadelUsers` does not exist).

- [ ] **Step 3: Implement**
  - `ZitadelProperties`: add `apiToken`, `apiConfigured()`; update the Javadoc to mention the API token of the scheduled jobs (US-076). Update the two tests and `SecurityConfiguration.zitadelProperties` (`@Value("${beelot.zitadel.api-token:}") String apiToken`).
  - `ZitadelUsers`: base URL = issuer without trailing slashes; default header `Authorization: Bearer <token>`. Request body as a `Map` or record. Private nested records with `@JsonIgnoreProperties(ignoreUnknown = true)` mapping only `result[].userId`, `result[].details.creationDate` (`Instant`), `result[].human.email.isVerified` (`Boolean`), and `details.totalResult` (`long`). Annotate the class `@RegisterReflectionForBinding({...those records...})`. Paging loop: offset += 100 while `offset < totalResult` and no user at or after the cutoff has been seen. Error handling with `.onStatus(...)` / `RestClientException` → `ZitadelApiException(method, path, status)` where path is the request path without the host; 404 on delete is swallowed.
  - `application.properties`, after `beelot.zitadel.client-secret`:

```properties
# The service account token of the scheduled jobs (US-076); zitadel/provision.sh writes it to .env.
beelot.zitadel.api-token=${BEELOT_ZITADEL_API_TOKEN:}
```

  - `SecurityConfiguration`: `@Bean ZitadelUsers zitadelUsers(ZitadelProperties zitadel)` and `@Bean Clock clock()`.

- [ ] **Step 4: Run the tests**

Run: `./gradlew test --tests 'fr.beelot.application.security.*'`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/main/java/fr/beelot/application/security src/main/resources/application.properties src/test/java/fr/beelot/application/security
git commit -m "feat(account): list and delete Zitadel users through its API (US-076)"
```

---

### Task 2: Cleanup job

**Files:**
- Modify: `src/main/java/fr/beelot/application/account/AccountRepository.java`
- Create: `src/main/java/fr/beelot/application/account/UnconfirmedAccountCleanup.java`
- Modify: `src/main/resources/application.properties` (account block)
- Test: `src/test/java/fr/beelot/application/account/UnconfirmedAccountCleanupTest.java`
- Test: `src/test/java/fr/beelot/application/account/AccountRepositoryIntegrationTest.java`

**Interfaces:**
- Consumes: `ZitadelUsers.unconfirmedCreatedBefore(Instant)`, `ZitadelUsers.delete(String)`, `ZitadelApiException`, `ZitadelProperties.apiConfigured()`, the `Clock` bean (Task 1).
- Produces:
  - `AccountRepository`: `@Query("select a.providerSubject from Account a where a.provider = :provider and a.providerSubject in :subjects") Set<String> findProviderSubjects(String provider, Collection<String> subjects)`.
  - `@Component public class UnconfirmedAccountCleanup` with constructor `(ZitadelProperties zitadel, ZitadelUsers users, AccountRepository accounts, Clock clock, @Value("${beelot.account.unconfirmed-retention:P7D}") Duration retention)` and `@Scheduled(fixedDelayString = "${beelot.account.cleanup-interval:PT1H}", initialDelayString = "PT1M") public int cleanUp()` returning the number deleted.

- [ ] **Step 1: Write the failing tests**

`UnconfirmedAccountCleanupTest` (plain unit test, Mockito mocks of `ZitadelUsers` and `AccountRepository`, `Clock.fixed(Instant.parse("2026-10-08T12:00:00Z"), UTC)`, retention `P7D`, properties with token `"api-token"`):

```java
@Test void deletesUnconfirmedUsersWhoNeverSignedIn()
// users.unconfirmedCreatedBefore(Instant.parse("2026-10-01T12:00:00Z")) → ["u1", "u2", "u3"]
// accounts.findProviderSubjects("zitadel", ["u1","u2","u3"]) → {"u2"}
// assert cleanUp() == 2; verify delete("u1"), delete("u3"); verify(users, never()).delete("u2")

@Test void aFailedDeleteDoesNotStopTheOthers()
// candidates ["u1","u2"], no rows; delete("u1") throws new ZitadelApiException("DELETE", "/v2/users/u1", 500)
// assert cleanUp() == 1; verify delete("u2")

@Test void aFailedListingDeletesNothing()
// unconfirmedCreatedBefore throws ZitadelApiException("POST", "/v2/users", 401)
// assert cleanUp() == 0 (no exception escapes); verify(users, never()).delete(any())

@Test void noCandidatesQueriesNothing()
// candidates [] → cleanUp() == 0; verifyNoInteractions(accounts)

@Test void withoutATokenZitadelIsNotCalled()
// properties with apiToken "" → cleanUp() == 0; verifyNoInteractions(users, accounts)
```

`AccountRepositoryIntegrationTest` (`@SpringBootTest` + `@Testcontainers` + `@ServiceConnection` MariaDB, as `NotConfiguredAccountTest`), saving accounts through `AccountService.signIn`:

```java
@Test void findsWhichUsersHaveSignedIn()
// signIn("zitadel", "z-1", ENGLISH); signIn("other", "z-2", ENGLISH)
// assert accounts.findProviderSubjects("zitadel", List.of("z-1", "z-2", "z-3")) == Set.of("z-1")
```

- [ ] **Step 2: Run them to see them fail**

Run: `./gradlew test --tests 'fr.beelot.application.account.UnconfirmedAccountCleanupTest' --tests 'fr.beelot.application.account.AccountRepositoryIntegrationTest'`
Expected: compilation failure.

- [ ] **Step 3: Implement** the repository query and `UnconfirmedAccountCleanup` as the Interfaces block states. Logging (SLF4J, `LoggerFactory.getLogger`): INFO `"Deleted {} unconfirmed Zitadel users"` when the count is above 0; WARN `"Could not delete unconfirmed Zitadel user {}: {}"` with the user id and the exception message; WARN `"Could not list unconfirmed Zitadel users: {}"` with the exception message. Catch only `ZitadelApiException`. Class Javadoc: deletes, through Zitadel, the users who never confirmed their address within the retention and never signed in to Beelot (US-076).

Add to `application.properties`, after the Zitadel block:

```properties
# Zitadel users who never confirmed their address and never signed in are deleted after this time (US-076).
beelot.account.unconfirmed-retention=P7D
beelot.account.cleanup-interval=PT1H
```

- [ ] **Step 4: Run the whole suite**

Run: `./gradlew test`
Expected: BUILD SUCCESSFUL; 253 earlier tests plus the new ones pass.

- [ ] **Step 5: Commit**

```bash
git add src/main/java/fr/beelot/application/account src/main/resources/application.properties src/test/java/fr/beelot/application/account
git commit -m "feat(account): delete accounts left unconfirmed for 7 days (US-076)"
```

---

### Task 3: Provisioning, documentation and end-to-end check

**Files:**
- Modify: `zitadel/provision.sh`
- Modify: `README.md` (Zitadel section, environment variables)
- Modify: `docs/product-backlog.md` (US-076 block)

**Interfaces:**
- Consumes: `BEELOT_ZITADEL_API_TOKEN` read by `beelot.zitadel.api-token` (Task 1); the job (Task 2).
- Produces: `.env` line `BEELOT_ZITADEL_API_TOKEN=<token>`.

- [ ] **Step 1: Extend `provision.sh`**, using its `api` and `log` helpers, before "Writing the client":
  - `log "Email confirmation codes: valid 24 hours"`: `GET /admin/v1/secretgenerators/SECRET_GENERATOR_TYPE_VERIFY_EMAIL_CODE`, then `PUT` the same path with its current `length` and `include*` fields and `expiry: "86400s"` (jq merge).
  - `log "Service account beelot-jobs"`: find it with `POST /management/v1/users/_search` and a `userNameQuery` (`TEXT_QUERY_METHOD_EQUALS`); create it if missing with `POST /management/v1/users/machine` (`userName: "beelot-jobs"`, `name: "Beelot jobs"`, `accessTokenType: "ACCESS_TOKEN_TYPE_BEARER"`); grant the role with `POST /management/v1/orgs/me/members` `{"userId": ..., "roles": ["ORG_USER_MANAGER"]}` (an "already exists" answer is fine, as the helper allows).
  - Token: keep `BEELOT_ZITADEL_API_TOKEN` from `$ENV_FILE` when present; otherwise `POST /management/v1/users/$id/pats` with `{"expirationDate": "2099-01-01T00:00:00Z"}` and read `.token`. Never echo the token.
  - Add `echo "BEELOT_ZITADEL_API_TOKEN=$api_token"` to the `.env` block. Update the header comment ("Optional variables" and what the script writes).

- [ ] **Step 2: Run it twice against the local stack**

Run: `docker compose up -d --wait && zitadel/provision.sh && zitadel/provision.sh && grep -c '^BEELOT_ZITADEL_API_TOKEN=' .env`
Expected: both runs end with `• Done`; the count is `1`; the token is the same after the second run (compare `sha256sum` of the line before and after).

- [ ] **Step 3: Check the expiry and the role**

Run: the script's `GET /admin/v1/secretgenerators/SECRET_GENERATOR_TYPE_VERIFY_EMAIL_CODE` with the setup token.
Expected: `"expiry": "86400s"`. Then `curl` `POST $ZITADEL_URL/v2/users` with the `beelot-jobs` token answers 200, and `PUT /admin/v1/policies/password/complexity` with it answers 403 (least privilege).

- [ ] **Step 4: End-to-end check in a headless browser** (Playwright MCP, headless), with the app started as `./gradlew bootRun --args='--beelot.account.unconfirmed-retention=PT1M --beelot.account.cleanup-interval=PT30S'`:
  1. Click "Create an account", register `unconfirmed@example.test`; the flow stops at Zitadel's code page. Close it without confirming.
  2. "Sign in" with that address and password → Zitadel asks for the code (no Beelot session: `GET /api/account` shows `signedIn: false`).
  3. "Resend code" → a second email arrives in Mailpit (`http://localhost:8025/api/v1/messages`). Do not use it.
  4. Register `confirmed@example.test`, confirm with the Mailpit code, land back in Beelot signed in. Reusing that code on the verify page is refused.
  5. Wait about 2 minutes. The log shows `Deleted 1 unconfirmed Zitadel users`; `POST /v2/users` (setup token) lists `confirmed@example.test` and no longer `unconfirmed@example.test`.

Expected: all five hold. Record any failure as a finding before continuing.

- [ ] **Step 5: Documentation**
  - README: add `BEELOT_ZITADEL_API_TOKEN` to the environment-variable list (token of the `beelot-jobs` service account, role `ORG_USER_MANAGER`, written by `provision.sh`; without it no account is cleaned up), and one paragraph on the job and its two properties.
  - Backlog US-076: add `- Delivered: design in \`docs/superpowers/specs/2026-10-08-confirm-email-design.md\`.` as US-075 does.

- [ ] **Step 6: Native build and full suite**

Run: `./gradlew test nativeCompile`
Expected: BUILD SUCCESSFUL. Start the native binary with the `.env` values and confirm it starts and logs no `ZitadelApiException` within the first run (initial delay 1 minute).

- [ ] **Step 7: Commit**

```bash
git add zitadel/provision.sh README.md docs/product-backlog.md
git commit -m "build(zitadel): expire confirmation codes after 24 hours and provision the jobs account (US-076)"
```
