# Rewrite the backend in Rust — design

## Goal

Build Beelot's backend in Rust, offering these
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
  is no earlier schema to migrate from.
- **MariaDB stays** (ADR-002), through `sqlx` with its MySQL driver and embedded
  migrations.
- **Online tables get Server-Sent Events** in place of the 3-second poll.
- **Bots are deterministic**: same seed and same view give the same bid and the
  same card. Recorded deals check it.
- **One repository.** The Rust backend and the static frontend live in
  [`mederel/rubeelot`](https://github.com/mederel/rubeelot), started with a
  fresh history.
- **Incremental.** `rubeelot` grows in three phases, each its own PR or set of
  PRs; the cutover is phase 3.
- **Rejected:** a big-bang rewrite on a long-lived branch (one huge PR, features
  landing meanwhile ported twice), and a server-first port calling the old
  rules over FFI (glue thrown away entirely). WebSocket was rejected for
  updates: nothing needs to travel from the client over the socket, and SSE is
  simpler to build, test and proxy. OpenAPI was rejected: there is one client,
  shipped from the same repository.

## Repository

```
Cargo.toml                 workspace
crates/beelot-game/        rules, scoring, announcements, bots, solver (no I/O, no async)
crates/beelot-arena/       binary: the bot arena CLI
crates/beelot-server/      binary: axum server, serves the static frontend
fixtures/                  JSON lines of recorded deals
docs/                      this spec, plans, ADRs of the Rust backend
```

## Phase 1: `beelot-game` and `beelot-arena`

### What is deterministic

Every decision is a function of the seed and the view, with two exceptions:

- **The Challenging bot's time budget** (200 ms by default) stops its search at
  a point that depends on timing. Fixtures record it with no budget, and the
  Rust `SamplingStrategy` takes the budget as `Option<Duration>` so tests can do
  the same. Arena runs with `challenging` are therefore compared by strength,
  not line by line.
- **Decision times** in the arena report differ by nature; that line is left
  out of the comparison.

`SamplingStrategy` mixes a hash of each rank's label into its seeds, so Rust
uses a constant per rank (`Rank::label_hash`).

### Randomness

A `rng` module in `beelot-game` implements:

- `SplitMix64`: a 64-bit generator with `next_int(bound)` and `next_long`. The
  arena, the arena bots and `SamplingStrategy` use it.
- `shuffle`: a backward Fisher–Yates.

Production deals use the operating system's random generator and need no
parity.

### Crate shape

- Modules: `card`, `bidding`, `board`, `scoring`
  (`RoundScoring`), `announcements`, `match_score`, `rules` (`BeloteRules`),
  `bot::{rule_based, sampling, solver, memory, turns}`.
- `Suit`, `Rank`, `Variant`, `Difficulty` and `Team` are `Copy` enums; `Card`
  is a `Copy` struct of a rank and a suit.
- Seats are `Seat = usize` indices 0–3 in play order (North–South hold the even
  seats), not `UUID` keys in a `HashMap`; player identity and names belong to
  the server.
- `BiddingState` and `GameBoard` are structs whose operations return
  `Result<_, GameError>`; `GameError` is a `thiserror` enum with one variant per
  rejected move, and each variant has a stable code for the API. The English
  messages of the engine move to the server and the frontend in phase 2,
  as do the player-facing views (`BiddingView`, `GameBoardView`).
- Production bots implement
  `trait BotStrategy { fn decide_auction(&self, view: &AuctionView) -> AuctionDecision; fn choose_card(&self, view: &PlayView) -> Card; }`;
  arena bots implement `ArenaBot`, which also receives a
  `&mut dyn RandomSource`.
- `RandomSource` is the crate's own one-method trait
  (`next_int(bound) -> i32`), implemented by `SplitMix64`; the server implements it over an OS RNG in phase 2.
- Dependencies: `thiserror` and `serde`.

### `beelot-arena`

A `clap` binary with the options `--first`, `--second`, `--deals`, `--seed`,
`--variant` and `--announcements`. It offers these bots: current, challenging,
baseline, previous-play, previous-trumps, no-coinche, no-announcement-bidding
and random. A `criterion` benchmark in `beelot-game` times the solver.

### Fixtures

- About 100 seeded arena deals (some 3,500 decisions) are recorded in
  `fixtures/`: both variants, announcements on and off, every arena bot. Each
  JSON line holds the seed, the dealer, the bots, then every step (each bot's
  view and decision, each trick continuation) and the round result.
  `fixtures/SOURCE` says when they were recorded. Rust tests check the sampling
  bot's choices against the recorded views.

### Tests

Each rule has a readable Rust `#[test]` of its own beside the
fixtures. `PrivateTableTest`, `PublicTableTest` and `PlayerViewsTest` test the
table lifecycle and player-facing views, so they are ported in phase 2.

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
latest one. A keep-alive comment is sent every 15 seconds. An online seat
becomes disconnected when its last open stream closes (a player may have two
tabs), as the `/disconnect` beacon does today, and a bot takes it over 2 minutes
later; opening a new stream before then reconnects it.

At online tables a completed trick is collected 4 seconds after it completes,
by the server's sweep, instead of on the first poll 4 seconds after someone
loaded it. Local tables keep the player's "continue" click.

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
- `session`: token hash, `account_id`, `signed_in_at`, `last_seen_at`.
- `match_record`, `match_seat`, `match_round`: as today, announcements
  included.
- `security_event`: as today.

Identifiers stay MariaDB `UUID`; times stay `DATETIME(6)` in UTC. Queries live
in repository functions in `db::{accounts, passkeys, sessions, matches, events}`.
Tables in progress stay in memory, so a restart drops them, as today.

### Auth and sessions

- Guests need no session. Signing up or signing in creates a `session` row and
  sets a random 256-bit token in an HttpOnly, SameSite=Lax cookie; the database
  stores only its hash. As today, a session ends after 7 idle days or 30 days
  after its sign-in, whichever comes first, and deleting an account needs a
  sign-in less than 10 minutes old.
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
- The snapshot: `{version, tableId, kind, you, table, bidding?, board?, match?, reactions}`,
  each part shown from the requesting player's seat. It replaces the four GETs
  `/`, `/bidding`, `/board` and `/match`. Its parts keep the JSON shapes of
  today's responses (`PrivateTableResponse`, both `BiddingResponse`s, both board
  responses, `MatchResponse`, `ReactionResponse`), so the frontend renderers
  keep working. Player ids are no longer exposed: seats carry `you`, and the
  table carries `ownerPosition`.
- Local tables get a player token too; today anyone with a bot game's id can
  play it.
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

Every error body is `{error: <stable code>, message}`. The frontend's i18n
uses English text as its key, including patterns such as `"(.+) bids (\d+) …"`,
so the server keeps sending the same English messages (errors, auction
messages, call labels, Belote announcements) and the frontend keeps
translating `message`. The code is for the frontend's logic, such as
`reauthenticate`.

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

The frontend starts phase 2 as it was and is changed in `rubeelot`.

- `app.js`: one `api.table(id)` and one `api.act(id, action)` replace the
  separate bot-game and private-table calls; the renderers take one snapshot.
- An `EventSource` replaces the 3-second `setInterval` and the `pagehide`
  beacon, with a backoff if the browser does not reconnect by itself.
- The new sign-in, sign-up and sign-out URLs.
- Translations added to `i18n.js` and `i18n-nl.js` only for new English
  messages; the existing ones are unchanged.
- Unknown paths that are not API routes fall back to `index.html`.

## Testing

| Kind of test | How |
|---|---|
| Rules and bots | `beelot-game` unit tests and `tests/*.rs` (phase 1) |
| Tables, registry, matchmaking, limiters | Unit tests on `Table`, `TableRegistry`, matchmaking and the limiters, with a fake `Clock` |
| Routes, CSRF, SSE | Router tests with `tower::ServiceExt::oneshot` on the real `Router` |
| Database | `testcontainers-modules` MariaDB: one container per test binary, a fresh database per test |
| Passkeys | `webauthn-authenticator-rs`'s `SoftPasskey`, driving sign-up and sign-in through the router |

Coverage is reported by `cargo llvm-cov`, without a threshold.

Before the cutover, a manual browser check: a solo game, pass-and-play, a
private table in two browsers (live updates over SSE), quick-match with bots
filling seats, and sign-up, sign-in, export and account deletion.

## Phase 3: cutover

Production switches to the `rubeelot` build. In this repository:

- `docs/adr/001-rust-platform.md` records the Rust platform and notes that
  `sqlx` serves the MariaDB decision of ADR-002.
- README: what Beelot is, Quick start
  (`docker compose up -d --wait && cargo run -p beelot-server`), Verify, Bot
  arena (`cargo run --release -p beelot-arena -- …`), Solver benchmark
  (`cargo bench -p beelot-game`), Release build (`cargo build --release`), and
  the badges.
- AGENTS.md: build, test and formatting commands.

Configuration keeps its environment variables: `BEELOT_DATABASE_URL` (now a
`mysql://` URL), `BEELOT_DATABASE_USERNAME`, `BEELOT_DATABASE_PASSWORD`, the
relying party settings and the limits, read from the environment or `.env`.

## Out of scope

New features, running several instances, keeping tables across restarts, and
frontend changes beyond those above.
