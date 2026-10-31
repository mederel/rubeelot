# Phase 2: Beelot Server in Rust — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `beelot-server`: an axum server with all of Beelot's server features, serving the frontend, with one table model, Server-Sent Events, MariaDB through sqlx, and passkeys through webauthn-rs.

**Architecture:** `tables/` is a synchronous domain layer (seats, lobby, game flow, snapshots), with a `TableRegistry` of `Mutex`-guarded tables that publishes versions on `watch` channels. HTTP handlers in `routes/` turn requests into actions on it. Persistence (`db/`), auth (`auth/`), history and privacy sit beside it and never reach inside a table. Background jobs are tokio loops.

**Tech Stack:**
- Web and async: axum 0.8, tokio, tower and tower-http, tokio-util (`CancellationToken`), dashmap.
- Data: sqlx 0.8 (`mysql`, `uuid`, `time`, `migrate`), uuid, time, serde and serde_json.
- Auth: webauthn-rs 0.5 (feature `conditional-ui`), sha2, getrandom, base64.
- Config, errors and logs: dotenvy, anyhow, tracing and tracing-subscriber.
- Tests: testcontainers-modules (`mariadb`), webauthn-authenticator-rs (`softpasskey`), and tower's `ServiceExt`.

**Spec:** `docs/superpowers/specs/2026-10-23-rust-backend-design.md` (sections "Phase 2", "HTTP API", "Frontend changes" and "Testing").

**Prerequisite:** phase 1 is merged: `beelot-game` provides `BiddingState`, `GameBoard`, `MatchScore`, the bots and `RandomSource`.

**Behaviour:** the spec lists what the server does. Each task's tests are Rust tests of the new API.

## Global Constraints

- **Paths and statuses:** everything the spec's "HTTP API" section lists, with the request and response field names below. All JSON is camelCase.
- **The snapshot reuses today's response shapes**, so the frontend renderers keep working:
  - `bidding` is the union of `BotGameController.BiddingResponse` and `PrivateTableController.BiddingResponse`.
  - `board` is the union of `GameBoardResponse` and `BoardResponse`. `roundResult` uses the `RoundResultResponse` fields plus `announcements`.
  - `table` is `PrivateTableResponse`, with these changes:
    - `ownerPlayerId` → `ownerPosition`;
    - seats lose `playerId` and gain `you: bool`;
    - the table gains `kind`, `difficulty`, `difficultyLabel` and `passAndPlay`.
  - `match` is `MatchResponse`, and `reactions` is a list of `ReactionResponse`.
- **English strings:**
  - auction and board messages (`"… is deciding."`, `"… bids 90 Hearts."`, `"… declares Belote."`, and so on);
  - call labels (`"Pass"`, `"90 Hearts"`, `"Coinche!"`, the suit display names);
  - every conflict message.

  The frontend translates them with English text as the key, including regex keys in `i18n.js`. They all live in `crates/beelot-server/src/tables/messages.rs`, kept as they are.
- **Error bodies** are `{"error": "<kebab-case code>", "message": "<English message>"}`. The frontend keeps showing `t(body.message)`.
- **Names:**
  - a solo human seat is named `"You"`;
  - the local bots are `Camille`, `Luc`, `Manon` and `Hugo`;
  - bots at online tables are `Camille`, `Luc` and `Manon`;
  - a deleted account's name is `"Deleted player"`;
  - the teams are `"North–South"` and `"East–West"` (en dash).
- **Limits, from `beelot/src/main/resources/application.properties`:**

  | Setting | Value |
  |---|---|
  | request body | 4096 bytes |
  | request header | 8 KiB |
  | rate limit | 300 requests and 20 creations per 60 s per IP |
  | local tables | 5000 |
  | online tables | 2000 |
  | idle expiry | 2 h, swept every 1 min |
  | bot-fill wait | 60 s, swept every 5 s |
  | reconnect timeout | 2 min |
  | trick review | 4 s |
  | passkey ceremonies | 30 per IP per hour |
  | anonymous security events | 100 per IP per hour, 10,000 per hour overall |
  | session idle limit | 7 days |
  | session absolute limit | 30 days |
  | "recent sign-in" for deletion | 10 min |
  | export interval | 1 h |
  | event retention | 365 days |
  | account retention | 3 years since the last sign-in |
  | retention job | 03:30 UTC daily |
- **Environment variables:**
  - `BEELOT_BIND` (default `0.0.0.0:8080`)
  - `BEELOT_DATABASE_URL` (default `mysql://localhost:3306/beelot`), `BEELOT_DATABASE_USERNAME` and `BEELOT_DATABASE_PASSWORD` (both default `beelot`)
  - `BEELOT_PUBLIC_URL` (default `http://localhost:8080`)
  - `BEELOT_PRIVACY_CONTROLLER`, `BEELOT_PRIVACY_CONTACT`, `BEELOT_HOSTING_PROVIDER` and `BEELOT_BACKUP_RETENTION_DAYS`
  - `BEELOT_TRUST_FORWARDED_HEADERS` (default false)

  They are read from the environment and from `.env` through dotenvy.
- **Time:** every timed rule reads `Arc<dyn Clock>` (`fn now(&self) -> OffsetDateTime`). Tests use a `FakeClock` with `advance(Duration)`. Database times are `DATETIME(6)` in UTC.
- **Locks:** a table's `std::sync::Mutex` is never held across an `.await`. Actions on a table with a Challenging bot to move run inside `tokio::task::spawn_blocking`.
- **Logging:** tokens, cookies, query strings and request bodies are never logged.
- **Commits:** the same style and trailer as phase 1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace` pass at the end of every task.

## Review Focus

1. **Two tabs on one seat.** A seat becomes `DISCONNECTED` only when its *last* SSE stream closes, and it's `CONNECTED` again as soon as one opens. Bot takeover happens only 2 min after the last stream closed. Pinned in Task 7 by `second_stream_keeps_seat_connected`.
2. **Concurrent clicks.** Two players sending `nextRound` (or `rematch`) at once deal only one new round. A `nextRound` for an already-dealt round is a no-op. Pinned in Task 4 by `next_round_twice_deals_once`.
3. **Session limits.** A session ends 7 days after its last use or 30 days after its sign-in, whichever comes first. Each end the server notices is recorded once as `SESSION_EXPIRED`. Deleting an account needs a sign-in less than 10 minutes old. Pinned in Task 9 by `session_ends_after_thirty_days_even_when_active` and in Task 12 by `deletion_needs_recent_sign_in`.
4. **An account deleted mid-match.** Its seats go to bots named `"Deleted player"`, its table tokens stop working, and the finished match is recorded without the account. Pinned in Task 12 by `deleted_account_seat_goes_to_bot`.
5. **The SSE token in the query string.** `?token=` must never appear in logs or error bodies. Pinned in Task 1 by `trace_spans_omit_query_strings`.

---

### Task 1: Server skeleton

**Files:**
- Create: `crates/beelot-server/Cargo.toml`
- Create: `src/{main.rs, lib.rs, config.rs, error.rs, clock.rs, app.rs}`
- Create: `tests/skeleton.rs`
- Create: `crates/beelot-server/static/`, the existing frontend, copied in as it is.

**Interfaces:**
- Produces:
  - `pub struct Config { bind, database_url, database_username, database_password, public_url, privacy: PrivacyConfig, trust_forwarded_headers: bool, limits: Limits }`. `Config::from_env() -> Result<Config, ConfigError>`; `Limits::default()` holds the values from the Global Constraints.
  - `pub trait Clock: Send + Sync { fn now(&self) -> OffsetDateTime; }`, with `SystemClock` and `FakeClock { fn advance(&self, by: Duration) }`.
  - `pub enum AppError { NotFound, Conflict { code: &'static str, message: String }, Unauthorized, Forbidden { reason: Option<&'static str> }, PayloadTooLarge, RateLimited { retry_after_secs: u64 }, CapacityExceeded, Internal(anyhow::Error) }`, implementing `IntoResponse` with the status table in the spec. 429 and 503 set `Retry-After`. The 503 body's message is `"The server is busy. Please try again later."`.
  - `pub struct AppState`: `Clone`, holding `Arc`s of `Config`, `Clock` and the subsystems later tasks add.
  - `pub fn router(state: AppState) -> Router`
- The router:
  - serves `static/` with `ServeDir`;
  - answers `index.html` for exactly the SPA paths in `HomeController` (`/play/bot`, `/play/local`, `/play/bot/bidding/{id}`, `/play/bot/game/{id}`, `/online/private`, `/online/private/table/{id}`, `/online/public`, `/online/public/table/{id}`, `/tutorial`, `/rules`, `/settings`, `/stats`, `/privacy`);
  - answers 404 elsewhere;
  - adds `Referrer-Policy: no-referrer` to every response;
  - limits header size to 8 KiB;
  - opens a `TraceLayer` span per request that records the method, the path (without the query) and an `x-request-id`.

- [ ] **Step 1: Write the failing tests** in `tests/skeleton.rs`, using `router(test_state()).oneshot(...)`:
  - `serves_index_for_spa_paths`
  - `serves_static_assets`
  - `unknown_path_is_404`
  - `every_response_has_no_referrer_policy`
  - `error_bodies_have_code_and_message`
  - `trace_spans_omit_query_strings`: captures spans with a test `tracing` subscriber, requests `/stats?token=secret`, and asserts that no recorded field contains `secret`.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test skeleton`; expect compilation errors.
- [ ] **Step 3: Implement the files.** `main.rs` reads `Config`, initialises `tracing_subscriber` (env-filter, default `info`), binds the listener and serves with graceful shutdown on Ctrl-C/SIGTERM.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server --test skeleton`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(server): skeleton serving the frontend`.

---

### Task 2: Request limits

**Files:**
- Create: `src/limits/{mod.rs, window.rs, rate.rs, size.rs}`
- Create: `tests/limits.rs`
- Tests: the request limits, and the passkey limiter cases as `window.rs` unit tests.

**Interfaces:**
- Produces:
  - `pub struct FixedWindowLimiter`, with `FixedWindowLimiter::new(limit: u32, window: Duration, max_tracked: usize, clock: Arc<dyn Clock>)`.
    - `fn consume(&self, key: &str) -> Result<(), u64>`: counts every attempt and returns the seconds to wait, as `RequestRateLimitFilter.consume` does.
    - `fn try_acquire(&self, key: &str) -> bool`: a refused attempt isn't counted, as `HourlyIpLimiter` does.
    - 50,000 keys are tracked at most, with the same purge-then-refuse behaviour.
  - `pub fn client_ip(headers: &HeaderMap, peer: SocketAddr, trust_forwarded: bool) -> IpAddr`. When trusted, it takes the leftmost valid address in `X-Forwarded-For`; otherwise the peer address.
  - Middleware on `/api/*` only:
    - `rate_limit_layer`, where these requests count as creations: `POST /api/tables`, `POST /api/tables/join` and `POST /api/matchmaking/quick-match`. A refusal is 429 with a `Retry-After` header and the body `{"error":"rate-limited","message":"Too many requests. Please slow down."}`.
    - `RequestBodyLimitLayer(4096)`. A declared or streamed body over the limit gives 413.

- [ ] **Step 1: Write the failing tests.**
  - Test the request limits.
  - Add `forwarded_header_ignored_unless_trusted`.
  - Add `creation_limit_is_separate`: 21 creations give 429 while ordinary GETs still pass.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test limits`; expect failures.
- [ ] **Step 3: Implement the limiters and layers**, and wire them in `app.rs`.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(server): request rate and size limits`.

---

### Task 3: Table seats and lobby

**Files:**
- Create: `src/tables/{mod.rs, seats.rs}`
- Build: private-table seats, connection state and status, plus the local seat rules for solo and pass-and-play tables.
- Tests: the private and public table lifecycle and the name validation of pass-and-play, as unit tests in `seats.rs`.

**Interfaces:**
- Produces:
  - `pub enum TableKind { Local, Private, Public }`
  - `pub enum SeatKind { Human, Bot }`
  - `pub enum Connection { Connected, Disconnected { since: OffsetDateTime }, BotTakeover }`
  - `pub struct Seat { pub name: String, pub kind: SeatKind, pub ready: bool, pub connection: Connection, pub account: Option<Uuid> }`
  - `pub enum Status { WaitingForPlayers, InProgress }`
  - `pub struct Lobby { seats: [Option<Seat>; 4], owner: usize, status, variant, announcements: bool, turn_timer_seconds: u32, bot_fill_at: Option<OffsetDateTime> }`, with the seat methods below, returning `Result<_, AppError>` (conflicts carry their English message):
    - `join(name, account) -> Result<usize, AppError>`
    - `leave(pos)`, `choose_seat(pos, to)`, `set_ready(pos, bool)`, `start(pos)`, `start_with_bots(pos)` and `fill_with_bots_if_due(now) -> bool`
    - `disconnect(pos, now)`, `reconnect(pos)`, `replace_expired_disconnections(now, timeout)` and `hand_to_bot(pos, name)`
    - `set_turn_timer(pos, secs)` and `set_announcements(pos, on)`

    The turn timer stays a stored setting; nothing enforces it.
  - `pub fn local_seats(seats: Option<&[Option<String>]>) -> Result<[Seat; 4], AppError>`. With `None`, it gives a solo table: `You`, `Camille`, `Luc` and `Manon`. With four names, it gives pass-and-play, applying `createPassAndPlay`'s validation, messages and bot naming.
  - `pub fn required_name(name: &str) -> Result<String, AppError>`: trims the name; empty gives `"Enter a player name."`; over 30 characters gives `"Player names are limited to 30 characters."`.

- [ ] **Step 1: Write the failing tests** for the lobby, the seats and the local tables.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server tables::seats`; expect failures.
- [ ] **Step 3: Implement `seats.rs`.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server tables::seats`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(tables): seats and lobby`.

---

### Task 4: Table game flow

**Files:**
- Create: `src/tables/{table.rs, actions.rs, finished.rs}`
- Build: the game flow of solo, pass-and-play and private tables:
  - bot driving
  - `storeCompletedBoard`
  - `recordRoundIfComplete`
  - `nextRound` and `rematch`
  - `collectTrickIfReviewed` (now timer-driven, see `trick_review_due`)
  - `releaseAccount`
  - the pass-and-play `viewer` logic
- Tests: the flow of solo, pass-and-play and private tables, in `tests/table_flow.rs`, driving `Table` directly with a `FakeClock`.

**Interfaces:**
- Consumes:
  - from `beelot-game`: `BiddingState`, `GameBoard`, `MatchScore`, `BotStrategy`, `strategies::for_difficulty`, `strategies::for_tables` and `RandomSource`;
  - from Task 3: `Lobby` and `Seat`.
- Produces:
  - `pub struct Table { pub id: Uuid, pub kind: TableKind, pub difficulty: Option<Difficulty>, pub invitation_code: Option<String>, lobby: Lobby, bidding: Option<BiddingState>, board: Option<GameBoard>, match_score: Option<MatchScore>, round_recorded: bool, viewer: usize, reactions: Reactions, version: u64, last_activity: OffsetDateTime }`
  - `pub enum Action`: the spec's tagged enum (`#[serde(tag = "type", rename_all = "camelCase")]`). Its variants are:
    - `Ready { ready }` and `Seat { position }`
    - `Start { fill_with_bots }` and `Settings { announcements: Option<bool>, turn_timer: Option<u32> }`
    - `Pass`, `Trump { suit }`, `Contract { value, suit }` and `Coinche`
    - `Card { rank: String, suit, declare: Option<bool> }`
    - `ContinueTrick`, `NextRound` and `Rematch`
    - `React { reaction: TableReaction }` and `Leave`
  - `impl Table`:
    - `fn apply(&mut self, actor: Actor, action: Action, now: OffsetDateTime) -> Result<Outcome, AppError>`. `Actor` is `Seat(usize)` for an online player, or `Device` for the one token of a local table, which acts for the human whose turn it is. Every accepted action increments `version`.
    - `fn trick_review_due(&self) -> Option<usize>`: the completed-trick count, while an online table shows a completed trick.
    - `fn collect_trick(&mut self, expected_tricks: usize) -> Outcome`: a no-op unless the count still matches.
    - `fn release_account(&mut self, account: Uuid) -> Outcome`
    - `fn needs_blocking_bot(&self) -> bool`: true when a Challenging bot is to move.
  - `pub struct Outcome { pub finished: Option<FinishedMatch>, pub removed: bool }`
  - `pub struct FinishedMatch`: a finished match for the history, with `Mode { Solo, Private, Public }`. The mode is `Solo` for every local table; pass-and-play matches are never handed over.
- Behaviour notes:
  - **Local tables at online pace.** Local tables keep the client-driven `continueTrick`. At online tables a completed trick is collected by the registry's timer 4 s after it completes (Task 7), instead of on the first poll 4 s later.
  - **Deals come from an OS RNG.** Production deals use `OsRandom`, a `RandomSource` over `getrandom` in `tables/mod.rs`. Tests inject a seeded `SplitMix64` through `Table::with_rng`.

- [ ] **Step 1: Write the failing tests.** Write the flow tests listed above. Add:
  - `next_round_twice_deals_once` (Review Focus 2)
  - `rematch_only_after_complete_match`
  - `device_acts_for_whoever_is_to_play`: a pass-and-play table with two humans, where a `Card` action from `Actor::Device` is played by the active human.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test table_flow`; expect failures.
- [ ] **Step 3: Implement `table.rs`, `actions.rs` and `finished.rs`.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server --test table_flow`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(tables): game flow for local and online tables`.

---

### Task 5: Snapshots and messages

**Files:**
- Create: `src/api/{mod.rs, snapshot.rs}` and `src/tables/messages.rs`
- Create: `tests/api_contract.rs` and `tests/contract/*.json`
- Build: the bidding view (with `message`, `calls`, `auction` and `auctionId`), the board view (with `declarationMessage`, `trickWinner`, `trickPoints` and `ownAnnouncements`), and the response records.
- Tests: the player views.

**Interfaces:**
- Consumes: Task 4's `Table`.
- Produces:
  - `pub struct Snapshot { version: u64, table_id: Uuid, kind: TableKind, you: Option<usize>, table: TableView, bidding: Option<BiddingView>, board: Option<BoardView>, r#match: Option<MatchView>, reactions: Vec<ReactionView> }`, built with these field names, from the Global Constraints:
    - `TableView` has `id`, `invitationCode`, `ownerPosition`, `status`, `variant`, `variantLabel`, `turnTimerSeconds`, `seats[{name, ready, connectionState, position, you}]`, `publicTable`, `botFillAt`, `announcements`, `kind`, `difficulty`, `difficultyLabel` and `passAndPlay`.
    - `BiddingView` covers the Classic and Contrée auctions, plus `currentPlayerIndex`.
    - `BoardView` is the union of `GameBoardResponse` and `BoardResponse`.
    - `MatchView` is `{northSouth, eastWest, complete, winner}`.
    - `ReactionView` is `{sequence, position, reaction}`.
  - `pub fn snapshot(table: &Table, viewer: Viewer) -> Snapshot`. `Viewer` is `Seat(usize)` or `Device`; `Device` uses `Table::viewer`, the pass-and-play viewer logic.
  - In `messages.rs`, every English string the views and conflicts use, with `fn call_label(call: &SeatCall) -> String`.
  - `auctionId` is a `Uuid` the table generates on each new deal (`BiddingState::deals()` changing, or a new `BiddingState`).

- [ ] **Step 1: Write the failing tests.**
  - Test the player views.
  - Add `api_contract`, which builds snapshots for 5 fixed scenarios with a seeded table and compares each serialized snapshot with `tests/contract/<scenario>.json`. The scenarios are: lobby waiting, classic auction round 2, contrée bidding, trick under review, and round over with announcements. Setting `UPDATE_CONTRACT=1` rewrites the files.
  - Add `messages_are_the_expected_english_text`: asserts the exact strings for a pass, a bid, a coinche, Belote, Rebelote and a redeal.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test api_contract`; expect failures.
- [ ] **Step 3: Implement the module**, then generate the contract files once with `UPDATE_CONTRACT=1` and review them by hand against the frontend's field names.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(api): table snapshots`.

---

### Task 6: Registry, matchmaking and reactions

**Files:**
- Create: `src/tables/{registry.rs, matchmaking.rs, reactions.rs, codes.rs}`
- Build: the table registry (capacity, idle eviction, invitation codes, joining, open public tables, filling public tables with bots and releasing an account across tables), plus matchmaking and table reactions.
- Tests: matchmaking, public tables, table reactions, the memory bounds and the registry's private tables.

**Interfaces:**
- Produces:
  - `pub struct TableHandle { pub state: Mutex<Table>, pub changes: watch::Sender<u64>, tokens: Mutex<HashMap<TokenHash, Actor>>, streams: Mutex<[u32; 4]> }`
  - `pub struct TableRegistry`, with:
    - `fn create_local(&self, variant, difficulty, announcements, seats: Option<Vec<Option<String>>>, account) -> Result<Access, AppError>`
    - `fn create_private(&self, name, variant, account) -> Result<Access, AppError>`
    - `fn join(&self, code: &str, name, account) -> Result<Access, AppError>`
    - `fn quick_match(&self, name, variant, account) -> Result<Access, AppError>`: fullest table first, then the earliest `bot_fill_at`, falling back to a new public table, under one registry-wide `Mutex<()>`.
    - `fn handle(&self, id: Uuid) -> Option<Arc<TableHandle>>`
    - `fn authorize(&self, id: Uuid, token: &str) -> Result<(Arc<TableHandle>, Actor), AppError>`. A bad token gives `Unauthorized`.
    - `fn evict_idle(&self, now)`, `fn sweep(&self, now) -> Vec<Outcome>` (bot fill plus expired disconnections) and `fn release_account(&self, account) -> Vec<Outcome>`
    - `fn counts(&self) -> (usize, usize)` (local and online)
  - `pub struct Access { table_id: Uuid, player_token: String, code: Option<String> }`. Tokens are 32 random bytes in URL-safe base64; the registry keeps only `sha256(token)`, using the `sha2` crate.
  - `codes::new_code(taken: impl Fn(&str) -> bool) -> String`: 6 characters from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`. Codes are matched by trimming and upper-casing; anything over 32 characters matches nothing.
  - `pub struct Reactions` (per table): 30 s retention, at most 40 kept, at most 3 per player per 10 s, and a global sequence from an `AtomicU64`.
  - Capacity: when a cap is reached, idle tables are evicted first, then the request gets `CapacityExceeded`.
  - Every mutation ends with `changes.send_replace(version)`.

- [ ] **Step 1: Write the failing tests.** Write the registry, matchmaking and reaction tests, all with a `FakeClock`. Add `quick_match_prefers_fullest_table`, and `token_hash_only`: the registry's debug output never contains the plain token.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server tables::`; expect failures.
- [ ] **Step 3: Implement the modules.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(tables): registry, matchmaking and reactions`.

---

### Task 7: Table routes, SSE and background jobs

**Files:**
- Create: `src/routes/{mod.rs, tables.rs, events.rs}` and `src/jobs.rs`
- Create: `tests/table_routes.rs` and `tests/sse.rs`
- Tests: the solo, private and matchmaking routes, under `/api/tables`.

**Interfaces:**
- Consumes: Tasks 4–6. `FinishedMatch` is handed to a `MatchSink`, a trait implemented by history in Task 11 and by a `NoopSink` until then.
- Produces:
  - Routes:
    - `POST /api/tables` → 201 `{tableId, playerToken, code?}`
    - `POST /api/tables/join` → 201 `{tableId, playerToken}`
    - `POST /api/matchmaking/quick-match` → 201 `{tableId, playerToken}`
    - `GET /api/tables/{id}` → 200 `Snapshot`
    - `POST /api/tables/{id}/actions` → 200 `Snapshot`
    - `GET /api/tables/{id}/events` → `text/event-stream`

    The `actions` handler locks the table, calls `apply`, publishes the version, releases the lock, then hands `Outcome.finished` to the sink with `tokio::spawn`. It runs inside `spawn_blocking` when `needs_blocking_bot()`.
  - Auth: `Authorization: Bearer <token>`, or `?token=` on `/events` only. A missing or bad token gives 401 `{"error":"unauthorized","message":"You are not authorized for this table."}`.
  - SSE (`axum::response::sse`):
    - on connect it increments `streams[seat]`, reconnects the seat (a `BotTakeover` seat gets its conflict message as a single `error` event, then the stream closes), and sends the current snapshot at once;
    - after that, each `watch` change sends `event: snapshot`;
    - keep-alive every 15 s;
    - on drop, a guard decrements `streams[seat]` and, at 0, calls `disconnect(seat, now)`.

    Local tables have no presence: their streams don't count.
  - `jobs::spawn_all(state, CancellationToken)`:
    - a sweep every 5 s: `registry.sweep(now)`, plus a check of each online table's `trick_review_due()`, collecting tricks shown for at least 4 s through `collect_trick`;
    - idle eviction every 1 min.

    A failure is logged with `tracing::warn!` and the loop goes on.

- [ ] **Step 1: Write the failing tests.**
  - Test the table routes.
  - In `tests/sse.rs`, read the first events from the response body stream:
    - `stream_sends_snapshot_on_connect`
    - `stream_sends_snapshot_after_other_players_action`
    - `second_stream_keeps_seat_connected` (Review Focus 1)
    - `seat_goes_to_bot_two_minutes_after_last_stream_closes`, using `FakeClock` plus a manual `sweep`
    - `trick_is_collected_four_seconds_after_completion`
  - Add `player_token_in_body_is_ignored`: only the header authorizes.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test table_routes --test sse`; expect failures.
- [ ] **Step 3: Implement the routes and jobs**, and wire them into `router` and `main`.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(server): table API with server-sent events`.

---

### Task 8: Database

**Files:**
- Create: `crates/beelot-server/migrations/0001_initial.sql`
- Create: `src/db/{mod.rs, accounts.rs, passkeys.rs, sessions.rs, matches.rs, events.rs}`
- Create: `tests/common/db.rs`
- Create: `compose.yaml`, so `cargo run` works locally from now on. The spec scheduled this for phase 3; it's needed earlier.

**Interfaces:**
- Produces:
  - Schema, all InnoDB `utf8mb4`, with times in `DATETIME(6)` UTC:
    - `account (id UUID PK, display_name VARCHAR(30), created_at, last_sign_in_at, previous_sign_in_at NULL)`
    - `passkey (account_id UUID PK FK→account ON DELETE CASCADE, credential_id VARBINARY(1023) UNIQUE, passkey JSON, created_at, last_used_at NULL)`
    - `session (token_hash BINARY(32) PK, account_id UUID FK→account ON DELETE CASCADE, signed_in_at, last_seen_at, INDEX(account_id))`
    - `match_record`, `match_seat` and `match_round`: with the announcements column; `match_seat.account_id` has `ON DELETE SET NULL`
    - `security_event`, with its two indexes
  - `pub async fn connect(config: &Config) -> Result<MySqlPool>`, which runs `sqlx::migrate!()`.
  - Repository functions taking `&MySqlPool` or `&mut MySqlConnection`. Each later task adds only the queries it needs; this task provides `accounts::{insert, find, record_sign_in, delete}` and `events::insert`.
  - `tests/common/db.rs`:
    - `async fn test_pool() -> MySqlPool` starts one `testcontainers_modules::mariadb::Mariadb` (image `mariadb:12.3`) per test binary through a `tokio::sync::OnceCell`;
    - each call creates a fresh database, `CREATE DATABASE t_<uuid>`, and runs the migrations.

- [ ] **Step 1: Write the failing test** `migrations_apply_and_account_round_trips`: insert an account, find it, record a sign-in, and check that `previous_sign_in_at` shifts as `Account.signedIn` does.
- [ ] **Step 2: Run it to make sure it fails.** Run `cargo test -p beelot-server --test db` (Docker must be running); expect a failure.
- [ ] **Step 3: Implement the migration and the modules.**
- [ ] **Step 4: Run the test to make sure it passes.** Run `cargo test -p beelot-server --test db`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(db): schema and repositories`.

---

### Task 9: Sessions, CSRF and the account endpoint

**Files:**
- Create: `src/auth/{mod.rs, sessions.rs, csrf.rs}` and `src/routes/account.rs`
- Create: `tests/sessions.rs`
- Tests: the session lifetime, the account sessions, the CSRF cases (with a `BrowserCsrf` helper) and the `/api/account` routes.

**Interfaces:**
- Produces:
  - `pub struct CurrentSession { pub account_id: Uuid, pub signed_in_at: OffsetDateTime }`, an extractor that is optional for guests. It reads the `BEELOT_SESSION` cookie (HttpOnly, SameSite=Lax, `Secure` when `public_url` is https, Path=/, no Max-Age) and looks up `sha256(token)`.
    - A session past 7 days idle or 30 days since sign-in is deleted, recorded once as `SESSION_EXPIRED`, and the request goes on as a guest's.
    - Otherwise `last_seen_at` is bumped, at most once a minute.
  - `sessions::start(pool, account_id, now) -> (token, Set-Cookie)`
  - `sessions::end(pool, token)`
  - `sessions::end_all(pool, account_id)`
  - `sessions::sweep_expired(pool, now) -> u64`, which records `SESSION_EXPIRED` for each one; Task 12's job runs it hourly.
  - CSRF middleware, which ports `SecurityConfiguration`:
    - it sets an `XSRF-TOKEN` cookie (readable from JS, SameSite=Lax, Path=/) when there is none;
    - an unsafe method (not GET, HEAD, OPTIONS or TRACE) must send a matching `X-XSRF-TOKEN` header when a session cookie is present, or always on the four passkey paths;
    - a mismatch gives 403 `{"error":"csrf","message":"Forbidden"}`;
    - starting or ending a session rotates the token.
  - `GET /api/account` returns `{signedIn, name?, lastSignInAt?}`, with `AccountController`'s "last sign-in elsewhere" rule.
  - `POST /api/sign-out` returns 204: it ends the session, records `SIGN_OUT`, and expires the session cookie.

- [ ] **Step 1: Write the failing tests.** Write the tests for these routes. Add:
  - `session_ends_after_thirty_days_even_when_active` (Review Focus 3)
  - `idle_session_ends_after_seven_days`
  - `guest_post_needs_no_csrf`
  - `signed_in_post_without_csrf_is_403`
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test sessions`; expect failures.
- [ ] **Step 3: Implement the modules.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server --test sessions`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(auth): sessions and CSRF`.

---

### Task 10: Passkeys

**Files:**
- Create: `src/auth/{passkeys.rs, pseudonyms.rs, relying_party.rs}` and `src/routes/passkeys.rs`
- Create: `tests/passkeys.rs`
- Tests:
  - the relying party, the pseudonym generator, the passkey rate limiter and the passkey request filter
  - the signature counter check
  - passkey sign-up and sign-in, the sign-in success handler and the account service

  Sign-up and sign-in are driven by `webauthn-authenticator-rs`'s `SoftPasskey`.

**Interfaces:**
- Produces:
  - `RelyingParty::of(public_url: &str) -> Result<RelyingParty, String>`: the rules (lower-cased scheme and host, http or https only, no default port).
  - `pseudonyms::generate(language: Language, rng) -> String`: the three word lists copied verbatim.
  - `pseudonyms::language(form_language: Option<&str>, accept_language: Option<&str>) -> Language`: ports `PseudonymGenerator.language`.
  - Routes, all `POST`; the two `options` routes are limited by a `FixedWindowLimiter` of 30 per IP per hour, refusing with 429, `Retry-After: 3600` and a `RATE_LIMITED` event:
    - `/api/passkeys/sign-up/options`. Its form body is `language`. It picks a new account id and a pseudonym, and calls `start_passkey_registration(account_id, &account_id.to_string(), &pseudonym, None)`. It then sets `authenticator_selection.resident_key = Required`, `require_resident_key = true` and `user_verification = Required` on the returned challenge, as `WebAuthnConfiguration` does. The registration state is kept for 5 minutes in a `DashMap` keyed by a random `BEELOT_CEREMONY` cookie (HttpOnly, SameSite=Strict, Max-Age=300).
    - `/api/passkeys/sign-up` takes the `RegisterPublicKeyCredential` JSON. On success it inserts the account and the passkey in one transaction, records `SIGN_UP`, starts a session and returns 204. Any failure records a `SIGN_UP` failure and returns 400 with no reason. The ceremony state is consumed either way.
    - `/api/passkeys/sign-in/options` calls `start_discoverable_authentication()`, with the state stored like sign-up's.
    - `/api/passkeys/sign-in` runs `identify_discoverable_authentication`, loads the passkey by account and credential id, then `finish_discoverable_authentication`. It refuses unless `counter_goes_up(stored, presented)`, updates the stored passkey and `last_used_at`, records the sign-in, starts a session and returns 204. Any failure records a `SIGN_IN` failure and returns 401 with no reason.

- [ ] **Step 1: Write the failing tests.** Write the tests for these routes. Add:
  - `registration_requires_resident_key`: the options JSON has `residentKey: "required"` and `userVerification: "required"`.
  - `ceremony_state_is_single_use`
  - `sign_in_with_cloned_counter_is_refused`
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test passkeys`; expect failures.
- [ ] **Step 3: Implement the modules.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server --test passkeys`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(auth): passkey sign-up and sign-in`.

---

### Task 11: Match history and statistics

**Files:**
- Create: `src/history/{mod.rs, recorder.rs, statistics.rs}` and `src/routes/statistics.rs`
- Create: `tests/history.rs`
- Build: the match history service, its match, seat and round records, the player statistics and the statistics route
- Tests: the match history service, its database integration and the player statistics

**Interfaces:**
- Consumes: Task 4's `FinishedMatch`, and Task 7's `MatchSink`.
- Produces:
  - `pub struct HistoryRecorder(MySqlPool)`, implementing `MatchSink`. It stores a match only when a seat has an account, renames seats whose account is gone to `"Deleted player"`, and stores a match id only once. A failure is logged, never propagated.
  - `statistics::of(account: Uuid, matches: &[StoredMatch]) -> PlayerStatistics`: the JSON field names the frontend reads.
  - `GET /api/statistics` returns 200, or 401 `{"error":"sign-in-required","message":"Sign in to see your statistics."}`.

- [ ] **Step 1: Write the failing tests** for the history and the statistics.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test history`; expect failures.
- [ ] **Step 3: Implement the modules**, and replace `NoopSink` in `main`.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server --test history`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(history): match history and statistics`.

---

### Task 12: Privacy

**Files:**
- Create: `src/privacy/{mod.rs, events.rs, export.rs, deletion.rs, retention.rs}` and `src/routes/privacy.rs`
- Create: `tests/privacy.rs`
- Build: security events, the data export, account deletion, the retention job and the privacy routes
- Tests:
  - security events
  - the data export
  - account deletion
  - the retention job
  - the policy info, set and unset

**Interfaces:**
- Produces:
  - `SecurityEvents::record(type, outcome, account: Option<Uuid>, ip: Option<IpAddr>)`: never fails. Anonymous events are capped at 100 per IP and 10,000 overall per hour, with deletions always kept.
  - `GET /api/account/export`:
    - 200, in format `beelot-export-2`, with the same fields as `DataExport` (the passkey dates come from the `passkey` table) and `Content-Disposition: attachment; filename="beelot-data-<UTC date>.json"`;
    - 429 `{"minutesLeft": n}` with `Retry-After` within the hour;
    - 401 for a guest.
  - `DELETE /api/account`:
    - 204;
    - 403 `{"error":"reauthenticate","message":"Sign in again to delete your account.","reason":"reauthenticate"}` when the session signed in 10 or more minutes ago;
    - 401 for a guest.

    The deletion runs in one transaction: rename the account's match seats, delete its events, and delete the account (the passkey and sessions cascade). After that come `registry.release_account(id)` (its outcomes go to history) and an anonymous `DELETION` event.
  - `GET /api/privacy/policy-info` returns the four values, leaving out the blank ones. A warning lists the unset variables once at startup.
  - `retention::run(pool, now)` purges events older than 365 days and deletes accounts with no sign-in for 3 years, one at a time through the deletion path. `jobs` runs it daily at 03:30 UTC, and runs `sessions::sweep_expired` hourly.

- [ ] **Step 1: Write the failing tests.** Write the tests for these routes. Add:
  - `deletion_needs_recent_sign_in` (Review Focus 3)
  - `deleted_account_seat_goes_to_bot` (Review Focus 4): a signed-in player seated at a live private table deletes their account; their seat becomes a `BotTakeover` seat named `"Deleted player"`, their table token gets 401, and the match finished afterwards is stored with no account on that seat.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-server --test privacy`; expect failures.
- [ ] **Step 3: Implement the modules.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-server --test privacy`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(privacy): export, deletion, retention and security events`.

---

### Task 13: Frontend switch-over

**Files:**
- Modify: `crates/beelot-server/static/app.js`, `i18n.js` and `i18n-nl.js`

**Interfaces:**
- Consumes: the routes of Tasks 7, 9, 10, 11 and 12, and `tests/contract/*.json` for the shapes.
- Changes to `app.js`. Renderers keep their code; only data loading and actions change.
  - Add `tableSession(tableId)` and `saveTableSession({tableId, playerToken})` in `sessionStorage`. This replaces `getPrivateSession`, and now covers local tables too.
  - Add `api.createTable(body)`, `api.join(code, name)`, `api.quickMatch(variant, name)`, `api.table(id)` and `api.act(id, action)`. They use `apiJson` with an `Authorization: Bearer` header.
  - Add `openTableStream(id)`: an `EventSource` on `/api/tables/{id}/events?token=…` that calls `renderSnapshot(snapshot)` on each `snapshot` event. If the stream closes and doesn't reopen within 10 s, it reconnects with a backoff of 1, 2, 4 and up to 30 s.
  - `renderSnapshot` dispatches `snapshot.table`, `snapshot.bidding`, `snapshot.board`, `snapshot.match` and `snapshot.reactions` to the existing private-table and bot-game renderers. Local tables use the solo renderers.
  - Every former call becomes an `api.act` with the matching action, or one of the calls above:
    - `/api/bot-games/{id}/bids/{action}`, `/cards`, `/tricks/continue`, `/rounds/next` and `/rematch`;
    - `/api/private-tables/{id}/ready`, `/seat`, `/start`, `/start-with-bots`, `/announcements`, `/turn-timer`, `/reactions`, `/leave`, the bids, `/cards`, `/tricks/continue`, `/rounds/next` and `/rematch`;
    - creation, join and quick-match.
  - Remove: the 3-second `setInterval` (`app.js:1766`), the `pagehide` beacon (`app.js:1756`), the `/reconnect` calls, the per-part GETs and every `?playerToken=` in a URL.
  - The owner check uses `table.ownerPosition === table.seats.find(s => s.you)?.position` instead of comparing player ids.
  - Passkeys move to `/api/passkeys/sign-up/options`, `/api/passkeys/sign-up`, `/api/passkeys/sign-in/options` and `/api/passkeys/sign-in`. Sign-out moves to `/api/sign-out`. Request and response JSON follow webauthn-rs (`publicKey` wrapper).
- `i18n.js` and `i18n-nl.js`: add translations for any new English message (`"Sign in again to delete your account."`, `"You are not authorized for this table."` if missing).

- [ ] **Step 1: Check that the old URLs are gone.** Run `grep -nE "bot-games|private-tables|playerToken=|/logout|/login/webauthn|/webauthn/" crates/beelot-server/static/app.js`; expect no output.
- [ ] **Step 2: Run the server.** Run `docker compose up -d --wait && cargo run -p beelot-server`; expect it to listen on `0.0.0.0:8080`.
- [ ] **Step 3: Check it in a browser**, using the `run` skill or a manual session, with the console open:
  - a solo Classic game to the end of a round;
  - a solo Contrée game with a coinche;
  - pass-and-play with two humans;
  - a private table in two browsers: create, join by code, choose seats, ready, start, play a trick (both update without reloading), and send a reaction;
  - quick-match in one browser, waiting 60 s for the bots;
  - closing one browser at a private table: after 2 min its seat shows a bot;
  - sign-up with a passkey, sign-out, sign-in, statistics after a finished match, export, deletion.

  Expect no console errors and no 4xx or 5xx responses in the network panel apart from intended refusals.
- [ ] **Step 4: Run every check.** Run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`; expect all clean.
- [ ] **Step 5: Commit** with the message `feat(frontend): one table API with live updates`. Note any check from Step 3 that couldn't be run.

---

### Task 14: Documentation

**Files:**
- Modify: `README.md` (Quick start: `docker compose up -d --wait && cargo run -p beelot-server`, configuration variables, Verify) and `AGENTS.md` (the server and its Docker requirement for tests)

- [ ] **Step 1: Write the sections**, taking the environment variables from `Config::from_env` and the commands from Task 13.
- [ ] **Step 2: Check the README commands** by running each one as written; expect them to work.
- [ ] **Step 3: Commit** with the message `docs: run and verify the server`.
