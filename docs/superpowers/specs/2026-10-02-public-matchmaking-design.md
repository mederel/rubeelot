# Public matchmaking (US-017) — design

## Goal

As a player, I want public matchmaking, so that I can find a game without
arranging one privately.

A player enters a name, picks a variant, and clicks **Find a game**. The server
seats them at an open public table of that variant. The game starts as soon as
four players are seated, or after a fixed wait with bots in the empty seats.

## Decisions

- **Quick match only.** No browsable lobby list, no skill-based matching.
- **Bots fill after a wait.** If a public table is not full when the wait
  (default 60 s, counted from when the table opened) expires, bots take the
  empty seats and the game starts. There is no early "start with bots" vote.
- **Matching criterion:** variant only (Classic or Contrée).
- **Reuse the private-table engine.** A public table is a `PrivateTable` with a
  `publicTable` flag. Once seated, play uses the existing bidding, card, round,
  rematch, disconnect and bot-takeover code and endpoints
  (`/api/private-tables/{id}/…`). Renaming these endpoints is out of scope.

## Acceptance criteria

- The home screen offers a "Find a game" mode leading to `/online/public`.
- Finding a game seats the player at the open public table of the chosen
  variant with the most seated players, or opens a new one when none exists.
- Players are only matched with players who chose the same variant.
- The game starts automatically when the fourth player is seated; no ready
  step or owner action is needed.
- While waiting, the player sees the seated players and a countdown until bots
  take the empty seats; when it reaches zero, bots fill the table and the game
  starts.
- A waiting player can leave the table; their seat is freed for others.
- Public tables show no invitation code and cannot be joined by code.
- New texts are translated into French and Dutch.

## Domain — `fr.beelot.game.PrivateTable`

New state:

- `boolean publicTable` — fixed at construction.
- `Instant botFillAt` — the deadline after which bots fill the empty seats,
  computed by the service as opening time + wait and passed in (testability);
  `null` for private tables.
- `ownerPlayerId` becomes mutable (still read through `ownerPlayerId()`).
- `invitationCode` is `null` for public tables.

New constructor/factory: `PrivateTable.openPublic(UUID id, UUID ownerId,
String ownerName, GameVariant variant, Instant botFillAt)`.

New behaviour (all `synchronized`):

- `join(...)` — unchanged rules; additionally, on a public table, seating the
  fourth player marks every seat ready and sets status to `IN_PROGRESS`.
- `leave(UUID playerId)` — only while `WAITING_FOR_PLAYERS`, otherwise
  `PrivateTableConflictException("This table has already started.")`. Removes
  the seat. If the leaver was the owner and seats remain, the first remaining
  seat becomes owner. Allowed on private tables too (same rules), although the
  private UI does not expose it in this story.
- `isEmpty()` — no seats left.
- `fillWithBotsIfDue(Instant now)` — on a public table still
  `WAITING_FOR_PLAYERS` with `!botFillAt.isAfter(now)`: marks seated
  humans ready, adds bots for the empty seats (same names and seat state as
  `startWithBots`) and sets `IN_PROGRESS`. Returns `true` when it started the
  game, `false` otherwise. The bot-adding code is shared with `startWithBots`.
- Accessors `publicTable()`, `botFillAt()`.

## Application

### `PrivateTableService` (package `fr.beelot.application.privategame`)

It stays the single owner of tables, sessions, capacity and idle eviction.
Additions:

- `PrivateTableAccess openPublic(String name, GameVariant variant)` — creates a
  public table (capacity check, `lastActivity`, no invitation-code entry) and
  returns a session.
- `PrivateTableAccess joinPublic(UUID tableId, String name)` — joins a public
  table and returns a session; if the join started the game, initialises the
  match and bidding exactly as `start` does (shared private helper
  `beginMatch(tableId, table)`, also used by `start` and `startWithBots`).
- `List<PrivateTable> openPublicTables(GameVariant variant)` — public tables of
  that variant in `WAITING_FOR_PLAYERS` with fewer than four seats.
- `PrivateTable leave(UUID tableId, UUID token)` — removes the player's seat
  and session; removes the table (same cleanup as eviction) when it becomes
  empty.
- `void fillPublicTablesWithBots(Instant now)` — for each public waiting table,
  calls `fillWithBotsIfDue`; when it returns `true`, calls `beginMatch` and
  drives the bot turns.
- `get(tableId)` also runs the bot-fill check for that table, so polling
  clients see the game start without waiting for the sweep.
- `evictIdle` tolerates a `null` invitation code.
- The bot-fill wait is injected:
  `@Value("${beelot.matchmaking.bot-fill-wait:PT60S}") Duration botFillWait`.
  Existing convenience constructors keep their signatures and use the default.

### `MatchmakingService` (new, package `fr.beelot.application.matchmaking`)

- `PrivateTableAccess quickMatch(String playerName, GameVariant variant)` —
  `synchronized`; `null` variant means Classic. Picks the table from
  `openPublicTables(variant)` with the most seats (ties: earliest `botFillAt`) and
  calls `joinPublic`; otherwise `openPublic`. If a join fails with a conflict
  because the table filled meanwhile, it retries with the next candidate.
- `@Scheduled(fixedDelayString = "${beelot.matchmaking.sweep-interval:PT5S}")
  sweep()` — calls `fillPublicTablesWithBots(Instant.now())`.

Name validation, capacity errors and messages are those of
`PrivateTableService`.

### API

- `POST /api/matchmaking/quick-match` — body `{playerName, variant}`; `201`
  with the same session body as `POST /api/private-tables`. Conflicts map to
  `409 {message}`. Implemented by a new `MatchmakingController`; the session
  response record is reused from `PrivateTableController` (moved to package
  visibility or shared as needed).
- `POST /api/private-tables/{id}/leave` — body `{playerToken}`; returns the
  table, or `204` when the table was removed.
- `PrivateTableResponse` gains `publicTable` (boolean) and `botFillAt`
  (ISO instant, `null` for private tables or once started).

Both endpoints sit behind the existing request size and rate limit filters.

## Front end

- `index.html`:
  - Home: a "Find a game" mode card linking to `/online/public`.
  - New view `data-view="public"`: name input, variant choice
    (`name="public-variant"`), **Find a game** button, message area, back link.
  - Table view: a waiting panel (`#public-waiting`) with the countdown text and
    a **Leave table** button (`#leave-table-button`).
- `app.js`:
  - `/online/public` → `public` view; `/online/public/table/{id}` → the
    existing `private-table` view.
  - Submitting the form calls the quick-match endpoint, stores the session as
    for private tables, and navigates to `/online/public/table/{id}`.
  - When `table.publicTable`: hide the invitation panel, ready/start buttons,
    timer settings and private help; show the waiting panel while
    `WAITING_FOR_PLAYERS`, with a countdown computed from `botFillAt` and
    refreshed each second; change the eyebrow/heading to public wording.
  - **Leave table** calls the leave endpoint, clears the session and returns to
    `/online/public`.
  - The existing 3-second polling and `pagehide` disconnect apply to both
    paths.
- `HomeController`: forward `/online/public` and `/online/public/table/{id}`.
- `i18n.js` / `i18n-nl.js`: French and Dutch strings for all new texts.

## Error handling

- Unknown or started table on leave → `409` with the existing messages.
- Capacity exceeded → existing `CapacityExceededException` handling.
- A player who closes the page while waiting is marked disconnected; after the
  reconnect timeout the existing mechanism turns the seat into a bot takeover,
  which then counts as a seated player. This is accepted behaviour.

## Out of scope

- Server-side enforcement of the turn timer (currently UI text only): a player
  who stays connected but never plays can stall a public game. Candidate for a
  separate story.
- Browsable lobby, skill or language based matching, renaming
  `/api/private-tables`.

## Testing

- `PrivateTableTest` (new or extended): public table has no code; fourth join
  auto-starts; `leave` frees a seat and transfers ownership; leaving after
  start fails; `fillWithBotsIfDue` before/after the wait and on a full or
  private table.
- `PrivateTableServiceTest`: `openPublic`/`joinPublic`/`leave` (table removed
  when empty), bot fill begins the match and bidding, `get` triggers the fill.
- `MatchmakingServiceTest`: same-variant matching, picks the fullest table,
  opens a new table when none is open, four quick matches start one game,
  a fifth player gets a new table.
- `MatchmakingControllerTest` and a leave-endpoint test in
  `PrivateTableControllerTest`.
- `PublicMatchmakingUiTest`: packaged `index.html` contains the new mode card,
  view, form controls and leave button.
- Run `./gradlew test`, then a manual run of the app.
- Record US-017 acceptance criteria in `docs/product-backlog.md` and document
  the new mode and setting in the README.
