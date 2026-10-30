# Email Accounts Through Zitadel (US-075) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Players create an account and sign in through Zitadel, and Beelot
knows them only by their Zitadel user id and a random pseudonym.

**Architecture:** Spring Security's OAuth2 client signs in against one
OpenID Connect registration, `zitadel`, built in code from three properties,
with the `openid` scope only. `AccountService` keys accounts on
`("zitadel", sub)` and names them with a `PseudonymGenerator`. A local
Zitadel stack runs in Docker Compose and is configured by
`zitadel/provision.sh`.

**Tech Stack:** Java 21, Spring Boot 4 (Security OAuth2 client, Data JPA,
Flyway), MariaDB 11.4, Testcontainers, vanilla JS client, Docker Compose,
Zitadel v4.19.4, Traefik, Mailpit, bash + curl + jq.

**Spec:** `docs/superpowers/specs/2026-10-07-email-accounts-zitadel-design.md`

## Global Constraints

- Only the Zitadel user id (`sub`) crosses from Zitadel to Beelot: scope
  `openid` only; no name, email or other claim is requested, stored, logged
  or displayed.
- Registration id `zitadel`; account rows are `provider = "zitadel"`,
  `provider_subject = <sub>`.
- Properties `beelot.zitadel.issuer`, `beelot.zitadel.client-id`,
  `beelot.zitadel.client-secret` (env `BEELOT_ZITADEL_ISSUER`,
  `BEELOT_ZITADEL_CLIENT_ID`, `BEELOT_ZITADEL_CLIENT_SECRET`). No issuer → no
  registration, sign-in hidden.
- Zitadel endpoints from the issuer: `/oauth/v2/authorize`,
  `/oauth/v2/token`, `/oauth/v2/keys`, `/oidc/v1/end_session`; no user info
  URI; user name attribute `sub`. Nothing is fetched from Zitadel at startup.
- Pseudonym: adjective + animal + number 10–99, English, French or Dutch,
  at most 30 characters, generated once, never overwritten.
- Sign-in URL `/oauth2/authorization/zitadel`; register URL
  `/oauth2/authorization/zitadel?register` (adds `prompt=create`).
- Failed sign-in goes to `/?signin=failed`.
- `POST /logout` → `200 {"logoutUrl": …}`; `/` when there is no ID token.
- Local Zitadel on `http://localhost:8081`, Mailpit inbox on
  `http://localhost:8025`, Zitadel v4.19.4, login container
  `EMAIL_VERIFICATION=true`.
- Password minimum 12 characters, no required character classes; lockout 10
  password and 10 OTP attempts; languages en, fr, nl.
- New texts translated into French and Dutch (`i18n.js`, `i18n-nl.js`).

## Review Focus

- Zitadel unreachable when Beelot starts: the server must start and still
  offer the sign-in links (Task 3: the test issuer `https://zitadel.test`
  never resolves).
- A returning player whose browser language changed keeps their pseudonym
  (Task 2 test).
- `POST /logout` without an OIDC session (guest, or a session without an ID
  token) answers `{"logoutUrl": "/"}` rather than failing (Task 3 test).
- An unsupported or missing language (`de`, no `Accept-Language`) gets an
  English pseudonym (Task 1 test).
- The pseudonym replaces the default "Player" name in the table forms, as
  the provider name did before (Task 4 manual check).

---

### Task 1: Pseudonym generator

**Files:**
- Create: `src/main/java/fr/beelot/application/account/PseudonymGenerator.java`
- Test: `src/test/java/fr/beelot/application/account/PseudonymGeneratorTest.java`

**Interfaces:**
- Produces: `@Component public class PseudonymGenerator` with
  - `@Autowired PseudonymGenerator()` delegating to `RandomGenerator.getDefault()`;
  - `PseudonymGenerator(RandomGenerator random)` (package-private, for tests);
  - `public String generate(Locale locale)`;
  - `static List<String> all(Locale language)` (package-private): every
    combination without the number, for the tests.

Word lists: 16 adjectives × 16 animals per language, capitalised.
- English: `"<Adjective> <Animal> <NN>"`, e.g. "Swift Otter 42".
- French: `"<Animal> <Adjective> <NN>"`, e.g. "Loutre Agile 42"; only
  adjectives with the same masculine and feminine form (agile, rapide, calme,
  habile, brave, sage, tenace, fidèle, …).
- Dutch: `"<Adjective> <Animal> <NN>"`, e.g. "Snelle Otter 42"; only
  *de*-words as animals, adjectives in their inflected `-e` form.
- Language: `fr` → French, `nl` → Dutch, anything else or `null` → English.

- [ ] **Step 1: Write the failing tests**

```java
@Test void namesInTheRequestedLanguage() {
    var fixed = new PseudonymGenerator(new SplittableRandom(7));
    assertTrue(fixed.generate(Locale.FRENCH).matches("\\p{Lu}\\S* \\p{Lu}\\S* \\d\\d"));
    assertTrue(PseudonymGenerator.all(Locale.FRENCH).contains(withoutNumber(fixed.generate(Locale.FRENCH))));
    assertTrue(PseudonymGenerator.all(Locale.of("nl")).contains(withoutNumber(
            new PseudonymGenerator(new SplittableRandom(7)).generate(Locale.of("nl", "BE")))));
}
@Test void fallsBackOnEnglish() {
    assertTrue(PseudonymGenerator.all(Locale.ENGLISH).contains(withoutNumber(
            new PseudonymGenerator(new SplittableRandom(7)).generate(Locale.GERMAN))));
    assertTrue(PseudonymGenerator.all(Locale.ENGLISH).contains(withoutNumber(
            new PseudonymGenerator(new SplittableRandom(7)).generate(null))));
}
@Test void everyPseudonymFitsThirtyCharacters() {
    for (Locale language : List.of(Locale.ENGLISH, Locale.FRENCH, Locale.of("nl")))
        for (String name : PseudonymGenerator.all(language)) assertTrue((name + " 99").length() <= 30, name);
}
@Test void numbersRunFromTenToNinetyNine() {
    var generator = new PseudonymGenerator(new SplittableRandom(1));
    for (int i = 0; i < 1000; i++) {
        int n = Integer.parseInt(generator.generate(Locale.ENGLISH).replaceAll(".* ", ""));
        assertTrue(n >= 10 && n <= 99);
    }
}
// withoutNumber(s) = s.substring(0, s.lastIndexOf(' '))
```

- [ ] **Step 2: Run to verify they fail**

Run: `./gradlew test --tests '*PseudonymGeneratorTest'`
Expected: compilation failure, `PseudonymGenerator` does not exist.

- [ ] **Step 3: Implement `PseudonymGenerator`** with the word lists above.

- [ ] **Step 4: Run to verify they pass**

Run: `./gradlew test --tests '*PseudonymGeneratorTest'`
Expected: 4 tests PASS.

- [ ] **Step 5: Commit** — `feat(account): generate random pseudonyms (US-075)`

---

### Task 2: Accounts keyed on the Zitadel user id, with a pseudonym

**Files:**
- Modify: `src/main/java/fr/beelot/application/account/AccountService.java`
- Modify: `src/main/java/fr/beelot/application/account/Account.java`
  (`signedIn(Instant now)` no longer takes a name)
- Create: `src/main/resources/db/migration/V3__reset_accounts_for_zitadel.sql`
- Modify: `src/test/java/fr/beelot/application/account/AccountServiceTest.java`
- Modify: `src/test/java/fr/beelot/application/history/MatchHistoryIntegrationTest.java`
  (callers of `signIn`; name assertions read `account.displayName()`)

**Interfaces:**
- Consumes: `PseudonymGenerator.generate(Locale)` (Task 1).
- Produces: `public Account signIn(String provider, String subject, Locale locale)`
  — finds the account or creates it named `pseudonyms.generate(locale)`; an
  existing account only gets `lastSignInAt` updated. Constructor
  `AccountService(AccountRepository, PseudonymGenerator)`. Removed:
  `static String displayName(Map<String, Object>)` and `MAX_NAME_LENGTH`
  unless still used elsewhere. `current`, `currentId`, `displayName(UUID)`
  keep their signatures.

- [ ] **Step 1: Write the failing tests** in `AccountServiceTest`, with a
  Mockito `AccountRepository` (`save` returns its argument) and a
  `PseudonymGenerator` on `new SplittableRandom(3)`; replace the four
  profile-name tests.

```java
@Test void theFirstSignInCreatesAnAccountWithAPseudonym() {
    when(repository.findByProviderAndProviderSubject("zitadel", "u-1")).thenReturn(Optional.empty());
    Account account = service.signIn("zitadel", "u-1", Locale.FRENCH);
    assertTrue(PseudonymGenerator.all(Locale.FRENCH).contains(withoutNumber(account.displayName())));
    verify(repository).save(account);
}
@Test void aLaterSignInKeepsThePseudonymWhateverTheLanguage() {
    Account existing = /* created by a first signIn("zitadel", "u-1", Locale.ENGLISH) */;
    when(repository.findByProviderAndProviderSubject("zitadel", "u-1")).thenReturn(Optional.of(existing));
    String name = existing.displayName();
    Instant before = existing.lastSignInAt();
    assertSame(existing, service.signIn("zitadel", "u-1", Locale.of("nl")));
    assertEquals(name, existing.displayName());
    assertFalse(existing.lastSignInAt().isBefore(before));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `./gradlew test --tests '*AccountServiceTest'`
Expected: compilation failure on `signIn(String, String, Locale)`.

- [ ] **Step 3: Implement** the new `signIn`, update `Account.signedIn`, and
  update `SignInSuccessHandler` to call
  `accounts.signIn(token.getAuthorizedClientRegistrationId(), token.getName(), request.getLocale())`.

- [ ] **Step 4: Write `V3__reset_accounts_for_zitadel.sql`**: delete the
  `match_record` rows having a `match_seat` with a non-null `account_id`
  (seats and rounds cascade), set any remaining `match_seat.account_id` to
  null, delete every `account` row. A header comment says why: no real
  accounts existed before Zitadel (US-075).

- [ ] **Step 5: Update `MatchHistoryIntegrationTest`** callers to
  `accounts.signIn("zitadel", "<id>", Locale.ENGLISH)` and its seat-name
  assertions to the created account's `displayName()`. Leave its
  `@SpringBootTest` properties for Task 3.

- [ ] **Step 6: Run the account and history tests**

Run: `./gradlew test --tests '*AccountServiceTest' --tests '*MatchHistory*'`
Expected: PASS (Flyway applies V3 on the Testcontainers MariaDB).

- [ ] **Step 7: Commit** — `feat(account): name accounts with a pseudonym, keyed on the user id (US-075)`

---

### Task 3: Zitadel sign-in, registration and sign-out

**Files:**
- Create: `src/main/java/fr/beelot/application/security/ZitadelProperties.java`
- Create: `src/main/java/fr/beelot/application/security/RegisterAuthorizationRequestResolver.java`
- Create: `src/main/java/fr/beelot/application/security/ZitadelLogoutSuccessHandler.java`
- Modify: `src/main/java/fr/beelot/application/security/SecurityConfiguration.java`
- Modify: `src/main/java/fr/beelot/application/account/AccountController.java`
- Modify: `src/main/resources/application.properties`
- Modify: `.gitignore` (add `.env`)
- Test: `src/test/java/fr/beelot/application/security/RegisterAuthorizationRequestResolverTest.java`
- Modify: `src/test/java/fr/beelot/application/account/AccountIntegrationTest.java`
- Modify: `src/test/java/fr/beelot/application/history/MatchHistoryIntegrationTest.java`
  (properties and `oauth2Login()` → `oidcLogin()` on `zitadel`)

**Interfaces:**
- Consumes: `AccountService.signIn(String, String, Locale)` (Task 2).
- Produces:
  - `@ConfigurationProperties("beelot.zitadel") record ZitadelProperties(String issuer, String clientId, String clientSecret)`
    with `boolean configured()` (issuer, client id and secret all non-blank).
    Add binding hints for it in `SecurityConfiguration.Hints`, replacing the
    `OAuth2ClientProperties` ones.
  - `static final String REGISTRATION_ID = "zitadel"` in `SecurityConfiguration`.
  - `ClientRegistrationRepository` bean: `registrationId -> null` when not
    configured, else the one registration of the Global Constraints, with
    `providerConfigurationMetadata(Map.of("end_session_endpoint", issuer + "/oidc/v1/end_session"))`,
    `issuerUri(issuer)`, `clientAuthenticationMethod(CLIENT_SECRET_BASIC)`,
    redirect URI `{baseUrl}/login/oauth2/code/{registrationId}`.
  - `RegisterAuthorizationRequestResolver implements OAuth2AuthorizationRequestResolver`,
    wrapping `DefaultOAuth2AuthorizationRequestResolver(repository, "/oauth2/authorization")`;
    when the request has a `register` parameter it adds `prompt=create` to
    the additional parameters.
  - `ZitadelLogoutSuccessHandler implements LogoutSuccessHandler`: writes
    `{"logoutUrl": "<end_session_endpoint>?id_token_hint=<id token>&client_id=<id>&post_logout_redirect_uri=<context root URL>/"}`
    (URL-encoded) when the authentication's principal is an `OidcUser`,
    else `{"logoutUrl": "/"}`; status 200, `application/json`.
  - `AccountController` view: `record AccountView(boolean signedIn, String name, String signInUrl, String registerUrl)`
    serialised without nulls (`@JsonInclude(NON_NULL)`).
- `SecurityConfiguration` chain: `oauth2Login(login -> login.loginPage("/")
  .authorizationEndpoint(e -> e.authorizationRequestResolver(resolver))
  .successHandler(signInSuccessHandler).failureUrl("/?signin=failed"))`,
  `logout(… .logoutSuccessHandler(zitadelLogoutSuccessHandler))`; CSRF rule
  unchanged.
- `application.properties`: `spring.config.import=optional:file:.env[.properties]`;
  `beelot.zitadel.issuer=${BEELOT_ZITADEL_ISSUER:}` (same for client id and
  secret); replace the Google/GitHub comment with one on Zitadel.

- [ ] **Step 1: Write the failing resolver tests**

```java
@Test void theRegisterLinkOpensZitadelsRegistrationForm() {
    var request = new MockHttpServletRequest("GET", "/oauth2/authorization/zitadel");
    request.setServletPath("/oauth2/authorization/zitadel");
    request.setParameter("register", "");
    assertEquals("create", resolver.resolve(request).getAdditionalParameters().get("prompt"));
}
@Test void theSignInLinkDoesNot() {
    var request = new MockHttpServletRequest("GET", "/oauth2/authorization/zitadel");
    request.setServletPath("/oauth2/authorization/zitadel");
    assertNull(resolver.resolve(request).getAdditionalParameters().get("prompt"));
}
// resolver built on an InMemoryClientRegistrationRepository holding a zitadel
// registration with issuer https://zitadel.test
```

- [ ] **Step 2: Rewrite `AccountIntegrationTest`** with properties
  `beelot.zitadel.issuer=https://zitadel.test`, `client-id=beelot-test`,
  `client-secret=test-secret`. Keep the three CSRF/guest tests (the sign-out
  one now expects `status().isOk()`). Replace the others:

```java
@Test void aGuestIsOfferedToSignInOrCreateAnAccount() {
    // GET /api/account → signedIn false, signInUrl "/oauth2/authorization/zitadel",
    // registerUrl "/oauth2/authorization/zitadel?register", no "name"
}
@Test void signingInStartsZitadelsAuthorisationFlowWithTheOpenidScopeOnly() {
    // GET /oauth2/authorization/zitadel → 3xx, redirect starts with
    // "https://zitadel.test/oauth/v2/authorize?", contains "scope=openid&" or
    // ends with "scope=openid", and has no "prompt="
}
@Test void creatingAnAccountOpensTheRegistrationForm() {
    // GET /oauth2/authorization/zitadel?register → redirect contains "prompt=create"
}
@Test void theFirstSignInCreatesTheAccountUnderAPseudonym() {
    // signInSuccessHandler.onAuthenticationSuccess(request with Accept-Language "fr", …,
    //   OAuth2AuthenticationToken(oidcUser("u-1"), authorities, "zitadel")) twice
    // → one account ("zitadel", "u-1"); its name is in PseudonymGenerator.all(FRENCH);
    // GET /api/account with oidcLogin().idToken(t -> t.subject("u-1")).clientRegistration(zitadel)
    //   → signedIn true, name = that pseudonym, no signInUrl
}
@Test void signingOutAlsoEndsTheZitadelSession() {
    // POST /logout with oidcLogin() (id token value "id-token-1", subject "u-2") and csrf()
    // → 200, $.logoutUrl starts with "https://zitadel.test/oidc/v1/end_session?",
    //   contains "id_token_hint=id-token-1", "client_id=beelot-test",
    //   "post_logout_redirect_uri=http%3A%2F%2Flocalhost%2F"
}
@Test void aSessionWithoutAnIdTokenSignsOutLocally() {
    // POST /logout with a session and the CSRF token, no authentication → 200, $.logoutUrl "/"
}
```

  Add `NotConfiguredAccountTest` (`@SpringBootTest` without the Zitadel
  properties, same MariaDB container setup): `GET /api/account` →
  `signedIn` false and no `signInUrl`/`registerUrl`.

- [ ] **Step 3: Run to verify they fail**

Run: `./gradlew test --tests '*RegisterAuthorizationRequestResolverTest' --tests '*AccountIntegrationTest' --tests '*NotConfiguredAccountTest'`
Expected: compilation failures, then failures on the new JSON shape.

- [ ] **Step 4: Implement** the three classes, the configuration, the
  controller view and the properties, and switch `MatchHistoryIntegrationTest`
  to the Zitadel properties and `oidcLogin()` on the `zitadel` registration.

- [ ] **Step 5: Run the whole suite**

Run: `./gradlew test`
Expected: all tests PASS (about 245).

- [ ] **Step 6: Commit** — `feat(account): sign in and register through Zitadel (US-075)`

---

### Task 4: Home screen sign-in buttons

**Files:**
- Modify: `src/main/resources/static/index.html` (lines 25–35, 361–365)
- Modify: `src/main/resources/static/app.js` (`loadAccount`, sign-out handler, `?signin=failed`)
- Modify: `src/main/resources/static/i18n.js`, `src/main/resources/static/i18n-nl.js`

**Interfaces:**
- Consumes: `GET /api/account` → `{signedIn, name?, signInUrl?, registerUrl?}`;
  `POST /logout` → `{logoutUrl}` (Task 3).

- [ ] **Step 1: Update the markup and `loadAccount`**: the guest area (and
  `#stats-providers`, renamed `#stats-sign-in`; `#account-providers` renamed
  `#account-sign-in`) shows two links, "Sign in" (`signInUrl`) and "Create an
  account" (`registerUrl`), hidden when `signInUrl` is absent. The signed-in
  area and the "Player" pre-fill of `#owner-name, #join-name, #public-name`
  stay as they are.
- [ ] **Step 2: Sign-out** reads `logoutUrl` from the `POST /logout` JSON
  and navigates to it (`/` if the request fails).
- [ ] **Step 3: Failed sign-in**: when the page loads with `?signin=failed`,
  show "Sign-in did not complete. Please try again." in the account bar and
  remove the parameter with `history.replaceState`.
- [ ] **Step 4: Translations**: remove "Sign in with {0}"; add "Sign in"
  ("Se connecter" / "Inloggen"), "Create an account" ("Créer un compte" /
  "Account aanmaken"), and the failure message in French and Dutch.
- [ ] **Step 5: Check** with `./gradlew test` (the i18n completeness tests,
  if any, pass) and by loading `/` with `./gradlew bootRun` and a dummy
  `BEELOT_ZITADEL_*` configuration: both buttons appear; without it, none.
- [ ] **Step 6: Commit** — `feat(ui): offer sign-in and account creation through Zitadel (US-075)`

---

### Task 5: Local Zitadel stack and provisioning script

**Files:**
- Modify: `compose.yaml`
- Create: `zitadel/provision.sh` (executable)
- Modify: `.gitignore` (`zitadel/.bootstrap/`)
- Modify: `README.md` ("Sign-in" section, lines 56–90)

**Interfaces:**
- Produces: `.env` with `BEELOT_ZITADEL_ISSUER=http://localhost:8081`,
  `BEELOT_ZITADEL_CLIENT_ID`, `BEELOT_ZITADEL_CLIENT_SECRET` (read by Task 3's
  `spring.config.import`).

- [ ] **Step 1: Add the services** to `compose.yaml`, adapted from Zitadel's
  `deploy/compose/docker-compose.yml` at v4.19.4: `zitadel-db`
  (`postgres:17-alpine`), `zitadel-api`
  (`ghcr.io/zitadel/zitadel:v4.19.4`, `start-from-init`, external domain
  `localhost`, external port `8081`, not secure, login v2 required with its
  URLs on `http://localhost:8081/ui/v2/login/`, first-instance login client
  PAT, and a first-instance machine user `beelot-setup` with a PAT at
  `/zitadel/bootstrap/beelot-setup.pat`), `zitadel-login`
  (`ghcr.io/zitadel/zitadel-login:v4.19.4`, `EMAIL_VERIFICATION: "true"`),
  `zitadel-proxy` (Traefik, the official labels, published `8081:80`), and
  `mailpit` (`axllent/mailpit`, inbox published on `8025`). Development
  secrets inline with a "local only" comment. Keep `mariadb` unchanged.
- [ ] **Step 2: Start it**

Run: `docker compose up -d --wait`
Expected: every service healthy; `curl -s http://localhost:8081/.well-known/openid-configuration | jq -r .issuer`
prints `http://localhost:8081`.

- [ ] **Step 3: Write `zitadel/provision.sh`** (`set -euo pipefail`),
  following the spec's "Provisioning script" steps 1–6, through Zitadel's
  v1 Admin and Management REST APIs (check paths against the running
  version's API reference). Token: `ZITADEL_TOKEN` if set, else
  `docker compose cp zitadel-api:/zitadel/bootstrap/beelot-setup.pat -`
  (the image has no shell). Every step looks up before creating, so a second
  run changes nothing. Values from the Global Constraints. Branding primary
  colour from `styles.css`'s accent colour.
- [ ] **Step 4: Run it twice**

Run: `zitadel/provision.sh && zitadel/provision.sh && grep -c BEELOT_ZITADEL_ .env`
Expected: both runs exit 0, the second reports nothing created; `3`.

- [ ] **Step 5: README**: replace the Google/GitHub section with
  "Accounts (Zitadel)": `docker compose up -d --wait`, `zitadel/provision.sh`,
  `./gradlew bootRun`, the environment variables, the Mailpit inbox, the
  Zitadel console (`http://localhost:8081/ui/console`, admin credentials
  from `compose.yaml`), Google/GitHub credentials through the script with
  callback `http://localhost:8081/idps/callback`, and what an account stores
  (user id, pseudonym, dates; no email or name).
- [ ] **Step 6: Commit** — `build(zitadel): run Zitadel locally and provision it (US-075)`

---

### Task 6: End-to-end check, native build and docs

**Files:**
- Modify: `docs/product-backlog.md` (US-075 criteria; US-077 note that
  Google/GitHub moved behind Zitadel in US-075)
- Modify: `docs/adr/003-identity-provider.md` ("user id only" rule; check
  that Zitadel Cloud's hosted login enforces email verification)

- [ ] **Step 1: Manual check in a headless browser** (Playwright MCP),
  against `docker compose` + `./gradlew bootRun`:
  1. "Create an account" opens Zitadel's registration form.
  2. Register with a password of 11 characters: refused; 12: accepted.
  3. Before confirming, signing in stops at the verification page.
  4. Confirm with the code from Mailpit (`http://localhost:8025`); Beelot's
     home screen shows "Signed in as <pseudonym>", and the "Quick game" name
     field is pre-filled with it.
  5. Sign out, then "Sign in" asks for credentials again.
  6. `docker compose exec mariadb mariadb -ubeelot -pbeelot beelot -e 'select * from account'`
     shows only `zitadel`, the user id, the pseudonym and dates; grep the
     bootRun log for the test email address and names: no match.
- [ ] **Step 2: Native build**

Run: `./gradlew nativeCompile`
Expected: BUILD SUCCESSFUL; the binary starts and `/api/account` answers
with the Zitadel links when `BEELOT_ZITADEL_*` are set.

- [ ] **Step 3: Update the backlog and ADR-003** as listed above.
- [ ] **Step 4: Commit** — `docs: record US-075 as delivered through Zitadel`
