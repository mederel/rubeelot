# Rewrite the backend in Rust — design

## Goal

Replace the Java 21 / Spring Boot 4 backend with a Rust one that offers the same
features: solo games against bots, pass-and-play, private tables, public
matchmaking, passkey accounts, match history and statistics, GDPR export,
deletion and retention, and the bot arena. The API, schema and architecture may
change where Rust suggests a better shape; the static frontend is adjusted to
match. MariaDB stays.

## Decisions

- **Same features, reshaped where it helps** (not a drop-in port). The HTTP API,
  the schema and the frontend's networking code change; game rules and bot
  behaviour do not.
- **No data to keep.** The schema starts again from one fresh migration; there
  is no migration path from the Flyway schema.
- **MariaDB stays** (ADR-002), through `sqlx` with its MySQL driver and embedded
  migrations, in place of JPA/Hibernate and Flyway.
- **Online tables get Server-Sent Events** in place of the 3-second poll.
- **Bots reproduce Java's decisions exactly**: same seed and same view give the
  same bid and the same card. Fixtures recorded from Java prove it.
- **Two repositories.** The Rust backend lives in
  [`mederel/rubeelot`](https://github.com/mederel/rubeelot), started with a
  fresh history: no Java, no Gradle. The Java backend stays in
  [`mederel/beelot`](https://github.com/mederel/beelot) and remains the product
  until the cutover. The only changes made in `beelot` for this work are the
  determinism fix and the fixture recorder (phase 1).
- **Incremental.** `rubeelot` grows in three phases, each its own PR or set of
  PRs; Java stays the product until the cutover in phase 3.
- **Rejected:** a big-bang rewrite on a long-lived branch (one huge PR, features
  landing on `master` ported twice), and a server-first port calling the Java
  rules over FFI (glue thrown away entirely). WebSocket was rejected for
  updates: nothing needs to travel from the client over the socket, and SSE is
  simpler to build, test and proxy. OpenAPI was rejected: there is one client,
  shipped from the same repository.

## Repositories

`rubeelot`:

```
Cargo.toml                 workspace
crates/beelot-game/        rules, scoring, announcements, bots, solver (no I/O, no async)
crates/beelot-arena/       binary: the bot arena CLI
crates/beelot-server/      binary: axum server, serves the static frontend
fixtures/                  JSON lines recorded from Java, copied from beelot
docs/                      this spec, plans, ADRs of the Rust backend
```

`beelot` keeps the Java backend, its frontend, its ADRs and its specs. Rules or
bot changes made in `beelot` before the cutover are ported to `rubeelot` with
freshly recorded fixtures.

## Phase 1: `beelot-game` and `beelot-arena`

### Determinism fix in Java, first (in `beelot`)

`SamplingStrategy.mix` hashes `card.rank().hashCode()`. An enum's `hashCode` is
an identity hash that changes on every JVM start, so the Challenging bot is not
reproducible from a seed today. Change it to `card.rank().ordinal()` in Java
before recording fixtures. The distribution of seeds is unchanged; only their
values become stable across runs.

### Randomness reproduced exactly

A `rng` module in `beelot-game` implements:

- `JavaRandom`: `java.util.Random`'s 48-bit linear congruential generator,
  including `nextInt(bound)`. The arena deals with it.
- `SplitMix64`: `java.util.SplittableRandom`, including `nextInt(bound)` and
  `nextLong`. The arena bots and `SamplingStrategy` use it.
- `java_shuffle`: `Collections.shuffle`'s backward Fisher–Yates.

Production deals use a cryptographic RNG (today `SecureRandom`); they need no
parity.

### Crate shape

- Modules follow `fr.beelot.game`: `card`, `bidding`, `board`, `scoring`
  (`RoundScoring`), `announcements`, `match_score`, `rules` (`BeloteRules`),
  `bot::{rule_based, sampling, solver, memory, turns}`.
- `Card`, `Suit`, `Rank`, `Variant` and `Difficulty` are `Copy` enums.
- Seats are `SeatIndex(u8)`, not `UUID` keys in a `HashMap`; player identity
  belongs to the server.
- `BiddingState` and `GameBoard` are structs whose operations return
  `Result<_, GameError>`; `GameError` is a `thiserror` enum with one variant per
  rejected move, and each variant has a stable code for the API.
- Bots implement
  `trait BotStrategy { fn decide_auction(&self, view: &AuctionView, rng: &mut dyn RngCore) -> AuctionDecision; fn choose_card(&self, view: &PlayView, rng: &mut dyn RngCore) -> Card; }`.
- Dependencies: `thiserror`, `rand_core`, and `serde` behind a `serde`
  feature (used by the fixtures and the server).

### `beelot-arena`

A `clap` binary that replaces the `botArena` Gradle task with the same options
and defaults: `--first`, `--second`, `--deals`, `--seed`, `--variant`,
`--announcements`. It offers the same bots as `ArenaBot` (current, challenging,
baseline, previous-play, previous-trumps, no-coinche,
no-announcement-bidding, random) and prints the same report as `ArenaReport`.
`SolverBenchmark` becomes a `criterion` benchmark in `beelot-game`.

### Fixtures

- `FixtureRecorder`, a new test-scope class in `beelot`, plays seeded arena
  matches: both variants, announcements on and off, every arena bot. It writes
  JSON lines: each line holds the seed and the input view, then the legal
  moves, the bot's decision, and the trick and round scores.
- About 2,000 decision lines are copied into `rubeelot/fixtures/`, with a
  `fixtures/SOURCE` file naming the `beelot` commit they were recorded from.
  `tests/fixtures.rs` replays every line.
- Arena parity: for one fixed seed and configuration per variant, the Java and
  Rust arena reports must be identical; the expected reports are checked in
  next to the fixtures.

### Tests

The Java tests under `src/test/java/fr/beelot/game` are ported one-for-one as
Rust `#[test]`s, so each rule keeps a readable test of its own beside the
fixtures.

### Done when

All fixtures and ported tests pass, the arena reports match, and the
`rubeelot` CI runs `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`.

## Phase 2: `beelot-server`

### Stack

axum and tokio; `sqlx` (MySQL); `webauthn-rs`; `tower-http` (static files,
body limit, request ID); `tracing`; `dashmap`; `dotenvy`.

### One table model

A solo game, pass-and-play and an online table are the same thing: a `Table`
with four seats, each human or bot. This replaces `BotGameService`'s eight
parallel maps and the duplicated lifecycle in `PrivateTableService`.

- `Table` holds the seats, the `BiddingState` or `GameBoard`, the `MatchScore`,
  the settings (announcements, turn timer), the reactions and the timers.
- `TableRegistry` is `DashMap<TableId, Arc<TableHandle>>`, with
  `TableHandle { state: std::sync::Mutex<Table>, changes: tokio::sync::watch::Sender<u64> }`.
- An accepted action locks the table, applies the change through
  `beelot-game`, increments the version, and releases the lock. The lock is
  never held across an `.await`.
- Bot turns run inside the action that hands them the turn. Challenging bots
  run on `spawn_blocking`.
- Capacity caps as today: 5,000 local tables and 2,000 online tables
  (`beelot.limits.*`), with `CapacityExceeded` past them.
- A `trait Clock` is injected everywhere time matters, so tests drive timers
  without sleeping.

### Server-Sent Events

`GET /api/tables/{id}/events` subscribes to the table's `watch` channel. On
each new version it builds that player's view (hidden hands differ per seat) and
sends it as one `snapshot` event. A client that misses versions only gets the
latest one. A keep-alive comment is sent every 15 seconds. When an online
player's stream closes, the seat becomes disconnected after a grace period, as
the `/disconnect` beacon does today; opening a new stream reconnects it.

### Background jobs

`tokio::spawn` loops that stop on a `CancellationToken` during graceful
shutdown. A failed iteration is logged and the loop goes on.

- Idle expiry every minute: tables idle for 2 hours are removed.
- Matchmaking sweep every 5 seconds: bots take the empty seats of a public
  table after 60 seconds.
- Retention purge daily at 03:30 UTC, with the rules of `RetentionJob`.

### Persistence

One migration, `crates/beelot-server/migrations/0001_initial.sql`, embedded
with `sqlx::migrate!` and run at startup. Tables:

- `account`: as today, without Spring's tables.
- `passkey`: `account_id` (unique: one passkey per account),
  `credential_id`, the serialized `webauthn_rs::Passkey` as JSON, `created_at`,
  `last_used_at`. It replaces `user_entities` and `user_credentials`.
- `session`: token hash, `account_id`, `created_at`, `expires_at`.
- `match_record`, `match_seat`, `match_round`: as today, announcements
  included.
- `security_event`: as today.

Identifiers stay MariaDB `UUID`; times stay `DATETIME(6)` in UTC. Queries live
in repository functions in `db::{accounts, passkeys, sessions, matches, events}`.
Tables in progress stay in memory, so a restart drops them, as today.

### Auth and sessions

- Guests need no session. Signing up or signing in creates a `session` row and
  sets a random 256-bit token in an HttpOnly, SameSite=Lax cookie; the database
  stores only its hash. Sessions last 7 days, as today.
- Deleting an account deletes its sessions with one statement (replacing
  `AccountSessions`).
- Passkeys use `webauthn-rs` with the relying party from configuration, as
  `RelyingParty` does today. The state of a sign-up or sign-in in progress is
  kept in memory for 5 minutes, keyed by a short-lived pre-auth cookie. One
  passkey per account; there is no endpoint to register another.
- A failed sign-in answers 401 with no reason and records a `SIGN_IN` failure
  event; the passkey rate limit of `PasskeyRateLimiter` carries over.
- The signature counter check of `CounterCheckingRelyingPartyOperations`
  carries over through `webauthn-rs`'s counter handling.
- CSRF keeps today's rule: an `XSRF-TOKEN` cookie readable by the client and an
  `X-XSRF-TOKEN` header, required on unsafe methods when a session exists, and
  always on sign-up and sign-in.
- `Referrer-Policy: no-referrer` on every response, as today.

### Limits

- Request body limited to 4 KiB, headers to 8 KiB.
- A per-IP sliding-window limiter, ported from `RequestRateLimitFilter` and
  `HourlyIpLimiter`: 300 requests and 20 creations per minute.
- Behind a reverse proxy the client address comes from `X-Forwarded-For` only
  when `BEELOT_TRUST_FORWARDED_HEADERS` is set.

### Layout

```
crates/beelot-server/
  migrations/
  static/                       the frontend, copied from beelot's src/main/resources/static
  src/
    main.rs  config.rs  error.rs  clock.rs
    api/       snapshot and action types (serde): the contract
    tables/    registry, table, bots, sse, matchmaking, reactions, expiry
    auth/      passkeys, sessions, csrf
    history/   recorder, statistics
    privacy/   export, deletion, retention, events
    limits/    rate, size
    routes/    one module per API area
    db/
```

## HTTP API (version 1)

### Tables

```
POST   /api/tables                   {kind: LOCAL|PRIVATE, variant, difficulty?, announcements?,
                                      seats?: ["HUMAN"|"BOT" ×4], playerName}
                                     → {tableId, playerToken, code?}
POST   /api/tables/join              {code, playerName} → {tableId, playerToken}
POST   /api/matchmaking/quick-match  {variant, playerName} → {tableId, playerToken}
GET    /api/tables/{id}              snapshot
GET    /api/tables/{id}/events       SSE stream of `snapshot` events
POST   /api/tables/{id}/actions      one action → 200 with the new snapshot
```

- `LOCAL` covers solo and pass-and-play: `seats` says which seats are human;
  the one player token acts for every human seat on that device. `PRIVATE`
  tables are joined by code; public tables come from quick-match.
- The snapshot: `{version, kind, status, seats, lobby?, bidding?, board?, match, reactions}`,
  each part shown from the requesting player's seat. It replaces the four GETs
  `/`, `/bidding`, `/board` and `/match`.
- Actions are one tagged enum (`#[serde(tag = "type")]`):

  ```
  {type: "ready", ready} | {type: "seat", position} | {type: "start", fillWithBots}
  {type: "settings", announcements?, turnTimer?}
  {type: "pass"} | {type: "trump", suit} | {type: "contract", value, suit} | {type: "coinche"}
  {type: "card", rank, suit, declare?} | {type: "continueTrick"} | {type: "nextRound"}
  {type: "rematch"} | {type: "react", reaction} | {type: "leave"}
  ```

  On a pass-and-play table, `card`, bid and announcement actions apply to the
  seat whose turn it is.
- The player token travels in `Authorization: Bearer <playerToken>`. The SSE
  endpoint also accepts `?token=` because `EventSource` cannot set headers; the
  server never logs query strings.
- `/reconnect` and `/disconnect` go: the SSE stream carries presence.

### Accounts and privacy

```
GET    /api/account                    {signedIn, name, lastSignInAt}
DELETE /api/account
GET    /api/account/export
POST   /api/passkeys/sign-up/options
POST   /api/passkeys/sign-up
POST   /api/passkeys/sign-in/options   (was /webauthn/authenticate/options)
POST   /api/passkeys/sign-in           (was /login/webauthn)
POST   /api/sign-out                   (was /logout)
GET    /api/statistics
GET    /api/privacy/policy-info
```

Response contents are the same as today. The passkey options are the standard
WebAuthn JSON that `webauthn-rs` produces.

### Errors

Every error body is `{error: <stable code>, message}`. The frontend translates
the code through i18n and no longer shows the server's English message.

| `AppError` | Status |
|---|---|
| `NotFound` | 404 |
| `Conflict(GameError)` | 409 |
| `Unauthorized` (bad player token, failed passkey) | 401 |
| `Forbidden` (missing CSRF token) | 403 |
| `PayloadTooLarge` | 413 |
| `RateLimited` | 429 |
| `CapacityExceeded` | 503 |
| `Internal` | 500, logged with the request ID, opaque body |

`tracing` writes structured logs. Tokens, cookies and query strings are never
logged; IP addresses go only into `security_event`, as today.

### Contract test

`tests/api_contract.rs` serializes example snapshots and actions to JSON files
under `crates/beelot-server/tests/contract/`, and fails when they change without
the files being updated. `app.js` refers to them in its comments.

## Frontend changes

The frontend is copied from `beelot` at the start of phase 2 (the commit is
named in the copying commit's message) and changed only in `rubeelot`; the
`beelot` frontend keeps talking to the Java API.

- `app.js`: one `api.table(id)` and one `api.act(id, action)` replace the
  separate bot-game and private-table calls; the renderers take one snapshot.
- An `EventSource` replaces the 3-second `setInterval` and the `pagehide`
  beacon, with a backoff if the browser does not reconnect by itself.
- The new sign-in, sign-up and sign-out URLs.
- Error-code keys added to `i18n.js` and `i18n-nl.js`.
- Unknown paths that are not API routes fall back to `index.html`, as
  `HomeController` does today.

## Testing

| Java today | Rust |
|---|---|
| `game/**Test` | `beelot-game` unit tests and `tests/fixtures.rs` (phase 1) |
| `*ServiceTest`, `TableReactionsTest`, `MemoryBoundsTest`, limiter tests | Unit tests on `Table`, `TableRegistry`, matchmaking and the limiters, with a fake `Clock` |
| `*ControllerTest`, `RequestLimitFiltersTest`, `SessionLifetimeFilterTest` | Router tests with `tower::ServiceExt::oneshot` on the real `Router`, CSRF and SSE included |
| `*IntegrationTest` (Testcontainers MariaDB) | `testcontainers-modules` MariaDB: one container per test binary, a fresh database per test |
| `SoftwarePasskey` (webauthn4j-test) | `webauthn-authenticator-rs`'s `SoftPasskey`, driving sign-up and sign-in through the router |
| `PasskeysMigrationTest` | Dropped: no data to migrate |

Coverage moves from JaCoCo to `cargo llvm-cov`, reported without a threshold, as
today.

Before the cutover, a manual browser check: a solo game, pass-and-play, a
private table in two browsers (live updates over SSE), quick-match with bots
filling seats, and sign-up, sign-in, export and account deletion.

## Phase 3: cutover

Production switches from the `beelot` build to the `rubeelot` build. In
`rubeelot`:

- `docs/adr/001-rust-platform.md` records the Rust platform, superseding
  `beelot`'s ADR-001 and noting that `sqlx` replaces JPA and Flyway under
  `beelot`'s ADR-002 (MariaDB).
- README: what Beelot is (from `beelot`'s README), Quick start
  (`docker compose up -d --wait && cargo run -p beelot-server`), Verify, Bot
  arena (`cargo run --release -p beelot-arena -- …`), Solver benchmark
  (`cargo bench -p beelot-game`), Release build (`cargo build --release`), and
  the badges.
- AGENTS.md: build, test and formatting commands.
- `compose.yaml` for MariaDB, copied from `beelot`.

In `beelot`: a README note pointing to `rubeelot`. Whether to archive `beelot`
is decided at that point, not in this spec.

Configuration keeps its environment variables: `BEELOT_DATABASE_URL` (now a
`mysql://` URL), `BEELOT_DATABASE_USERNAME`, `BEELOT_DATABASE_PASSWORD`, the
relying party settings and the limits, read from the environment or `.env`.

## Out of scope

New features, running several instances, keeping tables across restarts, and
frontend changes beyond those above.
