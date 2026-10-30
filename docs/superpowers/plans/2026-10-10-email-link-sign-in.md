# Email Link Sign-In Implementation Plan (US-075, ADR-004)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Players sign in with a one-time link emailed by Spring, or with Google or GitHub directly; Zitadel is removed.

**Architecture:** Spring Security's `oneTimeTokenLogin()` runs the link flow with a Beelot token store (hashed tokens in MariaDB) and a Beelot sender (plain-text email through `JavaMailSender`). A filter in front of token generation validates the address and applies rate limits. Google and GitHub return as `spring.security.oauth2.client` registrations bound at run time. The client gains a sign-in panel and a `/sign-in` view that posts the token.

**Tech Stack:** Java 21, Spring Boot 4.1.1, Spring Security 7.1.1, Spring Data JPA, `JdbcClient`, Flyway on MariaDB 11.4, `spring-boot-starter-mail`, Mailpit, vanilla JS client, JUnit 5, MockMvc, Testcontainers.

**Spec:** `docs/superpowers/specs/2026-10-10-email-link-sign-in-design.md` (and ADR-004).

## Global Constraints

- Work on branch `feat/us-075-email-link` from `master`.
- Spring Security 7.1.1 names: `OneTimeTokenLoginConfigurer.tokenGeneratingUrl`, `tokenGenerationSuccessHandler`, `tokenService`, `loginProcessingUrl`, `showDefaultSubmitPage`, `authenticationSuccessHandler`, `authenticationFailureHandler`. A successful link sign-in yields `OneTimeTokenAuthentication`; the token presented to `OneTimeTokenService.consume` is a `OneTimeTokenAuthenticationToken`. The generation filter reads the form parameter `username`; the login filter reads `token`.
- Token: 32 bytes from `SecureRandom`, base64url without padding (43 chars); stored as lowercase hex SHA-256 (64 chars). Lifetime `beelot.sign-in.link-lifetime`, default `PT15M`. Purge rows expired for more than 1 day, hourly.
- Addresses: trimmed, lower-cased with `Locale.ROOT`, at most 254 characters, exactly one `@`, a non-empty local part, and a domain containing a dot that neither starts nor ends with one.
- Rate limits: `beelot.sign-in.limit-per-address=3` per `PT15M`; `beelot.sign-in.limit-per-ip=10` per `PT1H`.
- The link is `{beelot.public-url}/sign-in?token={token}`, with any trailing `/` of the public URL removed. Never built from the request.
- Email language: the form parameter `language` (`en`, `fr`, `nl`) when present, else `PseudonymGenerator.language(Accept-Language)`. (The client's language is chosen in Settings, so the client sends it; this refines the spec's "from `Accept-Language`".)
- Never log an email address, a token or a link. Log lines hold counts and statuses only.
- Responses: link request 204; invalid address 400 `{"message":"Enter a valid email address."}`; limit 429 `{"message":"Too many links requested. Please try again later."}`; SMTP failure 503 `{"message":"The email could not be sent. Please try again later."}`; bad token 401; link sign-in success 204.
- `account.provider` values: `google`, `github`, `email`. An email account's `provider_subject` is its own id as a string.
- New user-facing texts get French (`i18n.js`) and Dutch (`i18n-nl.js`) entries.
- After each task: `./gradlew test` green (Docker must be running for Testcontainers).

## Review Focus

- **The same address typed differently** (`" Jane@Example.COM "` then `"jane@example.com"`) must reach one account and count against one rate limit. Tests in Tasks 2 and 4.
- **A token used twice**, for example a double click or two tabs, must sign in once and then answer 401. Tests in Tasks 3 and 6.
- **A forged `Host` header** on the link request must not change the link in the email. Test in Task 5.
- **A mail scanner fetching `GET /sign-in?token=…`** must leave the token usable. Test in Task 6.
- **An SMTP outage** must answer 503 for every address, and must neither log the address nor leave an account behind. Test in Task 5.

---

### Task 1: Google and GitHub sign in directly; Zitadel is removed

**Files:**
- Modify: `src/main/java/fr/beelot/application/security/SecurityConfiguration.java`
- Modify: `src/main/java/fr/beelot/application/account/AccountController.java`
- Modify: `src/main/resources/application.properties`, `compose.yaml`, `.gitignore`, `src/main/resources/static/app.js`, `src/main/resources/static/index.html` (only the account bar)
- Delete: `zitadel/`, `ZitadelProperties`, `ZitadelUsers`, `ZitadelApiException`, `ZitadelLogoutSuccessHandler`, `RegisterAuthorizationRequestResolver`, `UnconfirmedAccountCleanup`, and the tests `ZitadelUsersTest`, `RegisterAuthorizationRequestResolverTest`, `UnconfirmedAccountCleanupTest`
- Test: `src/test/java/fr/beelot/application/security/ClientRegistrationsTest.java` (rewrite), `account/AccountIntegrationTest.java`, `account/NotConfiguredAccountTest.java`, `history/MatchHistoryIntegrationTest.java`, `AccountUiTest.java`

**Interfaces:**
- Produces: the `ClientRegistrationRepository` bean, bound from `spring.security.oauth2.client` at run time (US-057's version: `git show 0e789ba^:src/main/java/fr/beelot/application/security/SecurityConfiguration.java`, including its `Hints` registrar). GitHub's registration is rebuilt with an empty scope set.
- Produces: `GET /api/account` returns `AccountView(boolean signedIn, String name, Boolean emailSignIn, List<Provider> providers)`, with `@JsonInclude(NON_NULL)`, where `Provider(String id, String name, String signInUrl)` covers each configured provider in the order `google`, `github`. A guest gets `providers` and, from Task 6, `emailSignIn: true`. A signed-in player gets `signedIn` and `name` only.
- Produces: `POST /logout` answers 204 (`HttpStatusReturningLogoutSuccessHandler(HttpStatus.NO_CONTENT)`).
- Keeps: the `Clock` bean, the CSRF rule, `SignInSuccessHandler` for `oauth2Login`, and `failureUrl("/?signin=failed")`.

- [ ] **Step 1: Write the failing tests**
  - `ClientRegistrationsTest`, a plain unit test that builds the repository from a `MockEnvironment`:
    - `googleAsksForTheOpenidScopeOnly`: with Google's id and secret set, plus `spring.security.oauth2.client.registration.google.scope=openid`, the registration's scopes are `Set.of("openid")`.
    - `githubAsksForNoScope`: with GitHub's id and secret set, the scopes are empty.
    - `aProviderWithoutCredentialsIsNotOffered`: with nothing set, `findByRegistrationId("google")` is null.
  - `AccountIntegrationTest`:
    - Its properties become `spring.security.oauth2.client.registration.google.client-id=g`, `…google.client-secret=s`, `…github.client-id=h`, `…github.client-secret=t`.
    - `aGuestIsOfferedGoogleAndGithub`: `$.providers[0].id == "google"`, `$.providers[0].signInUrl == "/oauth2/authorization/google"`, `$.providers[1].id == "github"`.
    - `signingInWithGoogleAsksForTheOpenidScopeOnly`: the redirect from `/oauth2/authorization/google` matches `[?&]scope=openid(&|$)`.
    - Existing tests change provider `zitadel` to `google`. The sign-out test expects 204.
    - Delete the Zitadel-specific tests: registration form, end-session URL.
  - `NotConfiguredAccountTest`: a guest gets `$.providers` empty.
  - `AccountUiTest`: assert `app.js` contains `account.providers` and no longer contains `registerUrl` or `logoutUrl`.

- [ ] **Step 2: Run the tests**

  Run: `./gradlew test --tests '*ClientRegistrationsTest' --tests '*AccountIntegrationTest' --tests '*AccountUiTest'`. Expected: FAIL.

- [ ] **Step 3: Implement**
  - Restore the run-time `ClientRegistrationRepository`. After mapping, replace the `github` registration with `ClientRegistration.withClientRegistration(r).scope(Set.of()).build()`. An empty property cannot override Boot's `read:user` default.
  - Remove the Zitadel beans, properties and classes listed above.
  - `application.properties`:
    - Drop the `beelot.zitadel.*` and `beelot.account.*` lines.
    - Add `spring.security.oauth2.client.registration.google.scope=openid`.
    - Add a comment naming `SPRING_SECURITY_OAUTH2_CLIENT_REGISTRATION_{GOOGLE,GITHUB}_CLIENT_{ID,SECRET}`.
    - Keep `spring.config.import=optional:file:.env[.properties]`.
  - `compose.yaml`: keep only `mariadb` and `mailpit`. Expose Mailpit as `"1025:1025"` (SMTP) and `"8025:8025"` (inbox), drop its `zitadel` network, and update the header comment.
  - Client:
    - `loadAccount()` renders one link per provider in `#account-sign-in` and `#stats-sign-in`, with the text `t("Continue with {0}", provider.name)`.
    - Sign-out posts `/logout`, then `window.location.assign("/")`.
    - Remove "Create an account".
    - Add French "Continuer avec {0}" and Dutch "Doorgaan met {0}".

- [ ] **Step 4: Run all tests**

  Run: `./gradlew test`. Expected: PASS, with no reference to Zitadel left in `src/` (`grep -ri zitadel src/main src/test` prints only `V3__reset_accounts_for_zitadel.sql`).

- [ ] **Step 5: Commit**

  `refactor(account): sign in with Google and GitHub directly and remove Zitadel (US-075, ADR-004)`

### Task 2: Email accounts

**Files:**
- Create: `src/main/resources/db/migration/V4__email_accounts.sql`, `account/AccountEmail.java`, `account/AccountEmailRepository.java`
- Modify: `account/Account.java`, `account/AccountService.java`
- Test: `account/AccountServiceTest.java`, `account/AccountRepositoryIntegrationTest.java`

**Interfaces:**
- Produces: `AccountService.signInWithEmail(String email, Locale locale) -> Account`, which expects an already normalised address.
- Produces: `AccountService.current(Authentication)`, which also resolves a `OneTimeTokenAuthentication` by `authentication.getName()` through `AccountEmailRepository.findByEmail(String) -> Optional<AccountEmail>`.
- Produces: `AccountEmail(UUID accountId, String email)`, an entity on table `account_email` with `@Id account_id`.
- Produces: `Account.withEmail(String displayName, Instant now)`, a static factory with `provider = "email"` and `providerSubject = id.toString()`.
- Produces: `EmailAddresses.normalise(String raw) -> Optional<String>` in `fr.beelot.application.account`, a final utility class that applies the Global Constraints rules.

- [ ] **Step 1: Write the failing tests**
  - `EmailAddressesTest`:
    - `" Jane@Example.COM "` → `"jane@example.com"`.
    - Each of `""`, `"jane"`, `"a@b@c.fr"`, `"@example.com"`, `"jane@example"`, `"jane@.example.com"`, `"jane@example.com."`, and a 255-character address → empty.
  - `AccountServiceTest`:
    - `theFirstLinkSignInCreatesAnEmailAccount`: provider `email`, subject equals `id().toString()`, a French pseudonym for `Locale.FRENCH`, and one `AccountEmail` saved with the address.
    - `aLaterLinkSignInFindsTheSameAccount`: with `findByEmail` returning the stored row, the same account comes back, no new account is saved, and `lastSignInAt` is updated.
  - `AccountRepositoryIntegrationTest`:
    - `theMigrationDeletesZitadelAccounts`, in its own class `EmailAccountsMigrationTest`, with a separate MariaDB Testcontainer and no Spring context:
      - run `Flyway.configure().dataSource(url, user, password).locations("classpath:db/migration").target("3").load().migrate()`;
      - insert a `zitadel` account, a `google` account and a match seating both;
      - migrate to the latest version;
      - the `zitadel` account and that match are gone, and the `google` account remains.
    - `anEmailBelongsToOneAccount`: a duplicate `account_email.email` raises `DataIntegrityViolationException`.

- [ ] **Step 2: Run the tests**

  Run: `./gradlew test --tests '*EmailAddressesTest' --tests '*AccountServiceTest' --tests '*AccountRepositoryIntegrationTest'`. Expected: FAIL to compile.

- [ ] **Step 3: Implement**

  `V4__email_accounts.sql`:

  ```sql
  -- Email-link accounts (US-075, ADR-004): the address lives only here, and is used only to send sign-in links.
  CREATE TABLE account_email (
      account_id UUID         NOT NULL PRIMARY KEY REFERENCES account (id) ON DELETE CASCADE,
      email      VARCHAR(254) NOT NULL,
      CONSTRAINT account_email_unique UNIQUE (email)
  );

  -- Pending sign-in links; only the SHA-256 hash of each token is stored. Times are in UTC.
  CREATE TABLE sign_in_token (
      token_hash CHAR(64)     NOT NULL PRIMARY KEY,
      email      VARCHAR(254) NOT NULL,
      expires_at DATETIME(6)  NOT NULL
  );

  CREATE INDEX sign_in_token_expires_idx ON sign_in_token (expires_at);

  -- Zitadel accounts existed only in development databases; they go with Zitadel, like the matches recorded for them.
  DELETE FROM match_record
  WHERE id IN (SELECT s.match_id FROM match_seat s JOIN account a ON a.id = s.account_id WHERE a.provider = 'zitadel');
  UPDATE match_seat SET account_id = NULL
  WHERE account_id IN (SELECT id FROM account WHERE provider = 'zitadel');
  DELETE FROM account WHERE provider = 'zitadel';
  ```

  Then add `Account.withEmail`, the `AccountEmail` entity and its repository, `AccountService.signInWithEmail` (`@Transactional`) and the `current` branch, and `EmailAddresses`.

- [ ] **Step 4: Run all tests**

  Run: `./gradlew test`. Expected: PASS.

- [ ] **Step 5: Commit**

  `feat(account): store email-link accounts with their address apart (US-075)`

### Task 3: Hashed one-time tokens

**Files:**
- Create: `account/SignInTokenService.java`
- Test: `account/SignInTokenServiceTest.java` (`@SpringBootTest` with a MariaDB Testcontainer, like `AccountRepositoryIntegrationTest`)

**Interfaces:**
- Produces: `SignInTokenService implements OneTimeTokenService`, a `@Component` built from `(JdbcClient jdbc, Clock clock, @Value("${beelot.sign-in.link-lifetime:PT15M}") Duration lifetime)`.
  - `generate(GenerateOneTimeTokenRequest)` returns a `DefaultOneTimeToken(clearToken, username, expiresAt)`. It ignores the request's own `expiresIn`.
  - `consume(OneTimeTokenAuthenticationToken)` returns a `OneTimeToken`, or null.
  - `@Scheduled(fixedDelayString = "PT1H", initialDelayString = "PT1M") int purgeExpired()` returns the number of rows deleted.
- Produces: `static String hash(String token)`, package-private: lowercase hex SHA-256.

- [ ] **Step 1: Write the failing tests** (the clock is a fixed `Clock` that the test can move forward)
  - `storesOnlyTheHash`: after `generate`, `SELECT token_hash, email FROM sign_in_token` has one row; its hash equals `hash(token.getTokenValue())` and differs from the token. The token matches `[A-Za-z0-9_-]{43}`.
  - `aTokenWorksOnce`: the first `consume` returns the email and the second returns null.
  - `anExpiredTokenIsRefused`: advance the clock by 15 min + 1 s; `consume` returns null and the row is gone.
  - `anUnknownTokenIsRefused`: `consume` of `"nope"` returns null.
  - `purgeDeletesOnlyRowsExpiredForADay`: of two rows, the one expired 25 h ago is deleted and the one expired 1 h ago is kept; the method returns 1.

- [ ] **Step 2: Run them**

  Run: `./gradlew test --tests '*SignInTokenServiceTest'`. Expected: FAIL to compile.

- [ ] **Step 3: Implement**

  `consume` runs in one `@Transactional` method: `SELECT email, expires_at … WHERE token_hash = ?`, then `DELETE … WHERE token_hash = ?`. It returns the token only when the delete count is 1 and `expires_at` is after `clock.instant()`. A concurrent consume therefore sees a delete count of 0.

- [ ] **Step 4: Run them**

  Expected: PASS.

- [ ] **Step 5: Commit**

  `feat(account): store sign-in tokens hashed, single-use and short-lived (US-075)`

### Task 4: Validate and limit link requests

**Files:**
- Create: `account/SignInRateLimiter.java`, `account/SignInRequestFilter.java`
- Test: `account/SignInRateLimiterTest.java`, `account/SignInRequestFilterTest.java`

**Interfaces:**
- Consumes: `EmailAddresses.normalise` (Task 2).
- Produces: `SignInRateLimiter`, a `@Component` built from `(Clock clock, @Value limit-per-address, @Value limit-per-ip)`. Its windows are fixed at `PT15M` per address and `PT1H` per IP. `boolean tryAcquire(String email, String ip)` returns false when either limit is reached, and then consumes from neither. At most 50 000 entries per map; at the cap, expired entries are dropped first, as in `RequestRateLimitFilter`.
- Produces: `SignInRequestFilter extends OncePerRequestFilter`, a plain class (not a `@Component`, so Boot does not register it twice) built from `(SignInRateLimiter limiter, JsonMapper json)`. It matches only `POST /api/sign-in/link`.
  - An invalid `username` gets 400 with the JSON message, and the chain stops.
  - A limit reached gets 429, and the chain stops.
  - Otherwise it passes on a request wrapper whose `getParameter("username")` returns the normalised address.

- [ ] **Step 1: Write the failing tests**
  - `SignInRateLimiterTest`:
    - the 4th request for one address within 15 min is refused; it is accepted again after 15 min;
    - the 11th request from one IP within 1 h is refused, across different addresses;
    - a refused request does not consume the other counter.
  - `SignInRequestFilterTest` (MockMvc-free, with `MockHttpServletRequest` and `MockFilterChain`):
    - `" Jane@Example.COM "` reaches the chain as `"jane@example.com"`;
    - `"jane"` gets 400 `{"message":"Enter a valid email address."}`, and the chain is not called;
    - over the limit, 429 `{"message":"Too many links requested. Please try again later."}`;
    - `GET /api/sign-in/link` and `POST /api/other` pass untouched.

- [ ] **Step 2: Run them**

  Expected: FAIL to compile.

- [ ] **Step 3: Implement** as specified above.

- [ ] **Step 4: Run them**

  Expected: PASS.

- [ ] **Step 5: Commit**

  `feat(account): validate sign-in link requests and limit them per address and IP (US-075)`

### Task 5: Email the link

**Files:**
- Modify: `build.gradle` (add `implementation 'org.springframework.boot:spring-boot-starter-mail'`), `application.properties`
- Create: `account/SignInEmail.java`, `account/SignInLinkSender.java`
- Test: `account/SignInEmailTest.java`, `account/SignInLinkSenderTest.java`

**Interfaces:**
- Produces: `record SignInEmail(String subject, String body)` with `static SignInEmail of(Locale locale, String link)`. `fr` and `nl` get their texts and anything else gets English.
- Produces: `static Locale SignInEmail.language(HttpServletRequest request)`, following the Global Constraints rule.
- Produces: `SignInLinkSender implements OneTimeTokenGenerationSuccessHandler`, a `@Component` built from `(JavaMailSender mail, JsonMapper json, @Value("${beelot.public-url}") String publicUrl, @Value("${beelot.mail.from}") String from)`.
  - It sends a `SimpleMailMessage` to `token.getUsername()` and answers 204.
  - On a `MailException`, it logs `WARN "Could not send a sign-in link: {}"` with the exception's class name only, and answers 503 with the JSON message.

Exact copy (the link is on its own line):

| Language | Subject | Body |
| --- | --- | --- |
| en | Your Beelot sign-in link | `Hello,\n\nUse this link to sign in to Beelot. If you have no account yet, it creates one.\n\n{link}\n\nThe link works once, for 15 minutes. If you did not ask for it, ignore this email: nobody can sign in without it.\n` |
| fr | Votre lien de connexion à Beelot | `Bonjour,\n\nUtilisez ce lien pour vous connecter à Beelot. Si vous n’avez pas encore de compte, il en crée un.\n\n{link}\n\nLe lien fonctionne une seule fois, pendant 15 minutes. Si vous ne l’avez pas demandé, ignorez cet e-mail : personne ne peut se connecter sans lui.\n` |
| nl | Je aanmeldlink voor Beelot | `Hallo,\n\nGebruik deze link om je aan te melden bij Beelot. Als je nog geen account hebt, wordt er een aangemaakt.\n\n{link}\n\nDe link werkt één keer, 15 minuten lang. Heb je hem niet aangevraagd? Negeer deze e-mail dan: zonder de link kan niemand zich aanmelden.\n` |

Configuration added to `application.properties`: the spec's block (`beelot.public-url`, `spring.mail.*`, `beelot.mail.from`, `beelot.sign-in.link-lifetime`), plus `spring.mail.properties.mail.smtp.connectiontimeout=5000`, `…timeout=10000` and `…writetimeout=10000`.

- [ ] **Step 1: Write the failing tests**
  - `SignInEmailTest`:
    - each language's subject;
    - the body contains the link on a line of its own;
    - `language` picks `fr` from `language=fr`, then from `Accept-Language: nl-BE` when the parameter is missing, and gives English for `language=de`.
  - `SignInLinkSenderTest`, with a Mockito `JavaMailSender` and an `ArgumentCaptor<SimpleMailMessage>`:
    - `theLinkUsesThePublicUrlWhateverTheHost`: the request has `Host: evil.example`, and `publicUrl` is `"https://beelot.fr/"`; the body contains `https://beelot.fr/sign-in?token=TOKEN` and not `evil`.
    - `sendsFromTheConfiguredAddressToThePlayer`.
    - `anSmtpFailureAnswers503WithoutTheAddress`: `send` throws `MailSendException("jane@example.com refused")`; the status is 503, and the captured log output (`OutputCaptureExtension`) does not contain `jane`.

- [ ] **Step 2: Run them**

  Expected: FAIL to compile.

- [ ] **Step 3: Implement** as specified above.

- [ ] **Step 4: Run them**

  Expected: PASS. Then run `./gradlew test`, which is still green: the context starts with the mail properties pointing at localhost.

- [ ] **Step 5: Commit**

  `feat(account): email the sign-in link in the player's language (US-075)`

### Task 6: Wire the link sign-in

**Files:**
- Create: `account/EmailUserDetailsService.java`
- Modify: `security/SecurityConfiguration.java`, `account/SignInSuccessHandler.java`, `account/AccountController.java`
- Test: `account/EmailSignInIntegrationTest.java` (`@SpringBootTest`, MockMvc, MariaDB Testcontainer, `@MockitoBean JavaMailSender`)

**Interfaces:**
- Consumes: `SignInTokenService`, `SignInLinkSender`, `SignInRequestFilter` and `SignInRateLimiter`, `AccountService.signInWithEmail`, and `SignInEmail.language` (as the pseudonym's locale too).
- Produces: in the filter chain:
  - `.oneTimeTokenLogin(...)` with the spec's settings, using `authenticationSuccessHandler(signInSuccessHandler)` and `authenticationFailureHandler((req, res, e) -> res.setStatus(401))`;
  - `.addFilterBefore(new SignInRequestFilter(limiter, json), GenerateOneTimeTokenFilter.class)`;
  - the CSRF matcher also always matches `POST /api/sign-in/link` and `POST /login/ott`;
  - `.headers(h -> h.referrerPolicy(r -> r.policy(ReferrerPolicy.NO_REFERRER)))`.
- Produces: `EmailUserDetailsService implements UserDetailsService`, a `@Component` whose `loadUserByUsername(email)` returns `User.withUsername(email).password("").authorities("EMAIL_USER").build()`. It is passed to the one-time-token configuration only; `oauth2Login` does not use it.
- Produces: `SignInSuccessHandler`. A `OneTimeTokenAuthentication` calls `signInWithEmail(authentication.getName(), SignInEmail.language(request))` and answers 204. OAuth works as before, with the locale from `SignInEmail.language(request)`.
- Produces: `/api/account` gives a guest `emailSignIn: true`.

- [ ] **Step 1: Write the failing tests** (`EmailSignInIntegrationTest`)
  - `aLinkRequestNeedsTheCsrfToken`: without the token, 403 and no email.
  - `knownAndUnknownAddressesGetTheSameAnswerAndEmail`:
    - request a link for a new address, sign in with it, sign out;
    - request again, and request for another new address;
    - all answers are 204 with an empty body, and the two captured emails have identical subjects and bodies once the token is masked.
  - `theLinkSignsThePlayerInOnce`:
    - take the token from the captured email and POST it to `/login/ott` with `token` and CSRF: 204;
    - `/api/account` with that session gives `signedIn: true` and a pseudonym;
    - the same token again gives 401;
    - `account_email` has one row.
  - `aScannerOpeningTheLinkDoesNotUseIt`: `GET /sign-in?token=T` returns 200 (it forwards to `index.html`; the route is added in this task), then the POST with T gives 204.
  - `signingInChangesTheSessionId`: POST the token with an existing `MockHttpSession`; the session in the result differs from it or has been invalidated.
  - `theSameAddressTypedDifferentlyReachesOneAccount`: sign in with links requested for `" Jane@Example.COM "` and for `"jane@example.com"`; one `account_email` row exists.
  - `aGuestIsOfferedEmailSignIn`: `$.emailSignIn == true`.

- [ ] **Step 2: Run them**

  Expected: FAIL.

- [ ] **Step 3: Implement** the wiring above, and add `"/sign-in"` to `HomeController`'s mappings.

- [ ] **Step 4: Run all tests**

  Run: `./gradlew test`. Expected: PASS.

- [ ] **Step 5: Commit**

  `feat(account): sign in with a one-time link sent by email (US-075)`

### Task 7: Client — sign-in panel and `/sign-in` view

**Files:**
- Modify: `static/index.html`, `static/app.js`, `static/styles.css`, `static/i18n.js`, `static/i18n-nl.js`
- Test: `AccountUiTest.java`, plus a headless-browser check (Step 4)

**Interfaces:**
- Consumes: `/api/account` (`emailSignIn`, `providers`), `POST /api/sign-in/link` (form fields `username` and `language`), `POST /login/ott` (form field `token`), and the `X-XSRF-TOKEN` header through the existing `withCsrfToken`.
- Produces, in the markup:
  - `#sign-in-panel`, a `<form>` with `<input type="email" id="sign-in-email" autocomplete="email" required maxlength="254">`, a submit button "Email me a sign-in link", the provider links, and `<p id="sign-in-status" role="status">`;
  - the `sign-in` view: `<h2>`"Sign in to Beelot", `<button id="sign-in-confirm">`"Sign in", and `<p id="sign-in-error" role="alert" hidden>`.
- Produces: `pathToView["/sign-in"] = "sign-in"`.

Behaviour:
- **Home:** "Sign in" in the guest bar toggles `#sign-in-panel`. The statistics prompt opens the same panel.
- **Panel submit:**
  - 204: "Check your inbox. If you can't find the email, look in your spam folder. The link works for 15 minutes."
  - 400, 429 or 503: the response's `message`, translated.
- **The `sign-in` view on load:** read `token` from the query, then `history.replaceState(null, "", "/sign-in")`.
- **Confirm button:**
  - 204: `window.location.assign("/")`.
  - 401: "This link has expired or has already been used." and show the panel.
  - Without a token: the same message.

Texts to translate (French / Dutch):

| English | French | Dutch |
| --- | --- | --- |
| Email me a sign-in link | Recevoir un lien de connexion | Stuur me een aanmeldlink |
| Email address | Adresse e-mail | E-mailadres |
| Check your inbox. If you can't find the email, look in your spam folder. The link works for 15 minutes. | Consultez votre boîte de réception. Si vous ne trouvez pas l’e-mail, regardez dans les indésirables. Le lien fonctionne pendant 15 minutes. | Kijk in je inbox. Vind je de e-mail niet, kijk dan in je spammap. De link werkt 15 minuten. |
| Enter a valid email address. | Saisissez une adresse e-mail valide. | Voer een geldig e-mailadres in. |
| Too many links requested. Please try again later. | Trop de liens demandés. Veuillez réessayer plus tard. | Te veel links aangevraagd. Probeer het later opnieuw. |
| The email could not be sent. Please try again later. | L’e-mail n’a pas pu être envoyé. Veuillez réessayer plus tard. | De e-mail kon niet worden verzonden. Probeer het later opnieuw. |
| Sign in to Beelot | Se connecter à Beelot | Aanmelden bij Beelot |
| This link has expired or has already been used. | Ce lien a expiré ou a déjà été utilisé. | Deze link is verlopen of al gebruikt. |

- [ ] **Step 1: Write the failing test**

  `AccountUiTest` asserts that:
  - `index.html` contains `id="sign-in-panel"`, `id="sign-in-email"` and `id="sign-in-confirm"`;
  - `app.js` contains `"/api/sign-in/link"`, `"/login/ott"`, `history.replaceState` and `"/sign-in": "sign-in"`;
  - both locale files contain each English key above.

- [ ] **Step 2: Run it**

  Expected: FAIL.

- [ ] **Step 3: Implement**

  Follow the existing view and `t()` patterns, and match the account bar's existing styles.

- [ ] **Step 4: Verify in a headless browser**

  Run `docker compose up -d --wait` and `./gradlew bootRun`. Then, headless:
  - open `/`, then "Sign in", enter an address and submit; the status text appears;
  - open the link from Mailpit (`http://localhost:8025/api/v1/messages`); the address bar shows `/sign-in` without the token;
  - click "Sign in"; home shows "Signed in as …";
  - sign out;
  - reopen the old link and click "Sign in"; the expired message appears;
  - repeat in French via Settings; the email arrives in French.

- [ ] **Step 5: Commit**

  `feat(ui): sign in with an email link from the home screen (US-075)`

### Task 8: Documentation and native build

**Files:**
- Modify: `README.md`, `docs/product-backlog.md`, `docs/superpowers/specs/2026-10-10-email-link-sign-in-design.md`

- [ ] **Step 1: Update the README**
  - Replace the Zitadel section with:
    - Mailpit;
    - `BEELOT_PUBLIC_URL`, `BEELOT_MAIL_HOST`, `_PORT`, `_USERNAME`, `_PASSWORD`, `_FROM`;
    - Google's and GitHub's variables, with their callback URLs `{public-url}/login/oauth2/code/google` and `…/github`.
  - Drop `zitadel/provision.sh`.

- [ ] **Step 2: Update the backlog and the spec**
  - Backlog: mark US-075 as delivered on ADR-004.
  - Spec: correct the language line to "the client's `language` parameter, else `Accept-Language`".

- [ ] **Step 3: Build the native image**

  Run: `./gradlew nativeCompile`, then start the binary against `docker compose`, request a link and sign in. If mail or one-time-token classes fail at run time, add `RuntimeHints` next to `AccountRuntimeHints`.

- [ ] **Step 4: Run the full suite**

  Run: `./gradlew test`. Expected: PASS. Report the test count.

- [ ] **Step 5: Commit**

  `docs: document email-link sign-in and record US-075 as delivered (ADR-004)`
