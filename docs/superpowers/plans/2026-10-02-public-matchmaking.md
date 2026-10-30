# Public Matchmaking (US-017) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a player click "Find a game" and be seated at a public table of their variant that starts automatically when full, or with bots after a 60 s wait.

**Architecture:** A public table is a `PrivateTable` with a `publicTable` flag, no invitation code and a `botFillAt` deadline. `PrivateTableService` stays the owner of tables, sessions and eviction and gains public-table operations; a new `MatchmakingService`/`MatchmakingController` picks the table. Play after seating reuses the existing `/api/private-tables/{id}/…` endpoints and the existing table view in the front end.

**Tech Stack:** Java 21, Spring Boot 4 (web MVC, scheduling), JUnit 5, MockMvc, vanilla JS/HTML.

**Spec:** `docs/superpowers/specs/2026-10-02-public-matchmaking-design.md`

## Global Constraints

- One commit for the whole story, spec + plan + code (user request): no intermediate commits. Message: `feat(matchmaking): find a public game with quick match (US-017)`.
- Bot-fill wait property: `beelot.matchmaking.bot-fill-wait`, default `PT60S`. Sweep: `beelot.matchmaking.sweep-interval`, default `PT5S`.
- Quick-match endpoint: `POST /api/matchmaking/quick-match`, body `{playerName, variant}`, `201`, same body as `POST /api/private-tables`.
- Leave endpoint: `POST /api/private-tables/{id}/leave`, body `{playerToken}`; `200` with the table, `204` when the table was removed.
- `PrivateTableResponse` gains `publicTable` (boolean) and `botFillAt` (ISO instant or `null`; `null` once started).
- Front-end routes: `/online/public` (form), `/online/public/table/{id}` (existing `private-table` view).
- Bot names and seat state for fill: exactly those of `startWithBots` (`Camille`, `Luc`, `Manon`, `BOT_TAKEOVER`, ready).
- Every new user-visible string has French (`i18n.js`) and Dutch (`i18n-nl.js`) entries.
- No new server error messages: leave after start uses `"This table has already started."`, unknown/non-public tables `"This table does not exist."`.

## Review Focus

1. Concurrent quick matches racing for the last seat — never a fifth seat, never two tables half-filled when one would do; covered by a parallel test in Task 3.
2. Bot fill racing with the fourth player's join (sweep vs. request) — the match must begin exactly once; covered in Task 2 (`fillAfterAutoStartDoesNothing`) and Task 1.
3. The owner leaves, then bots fill — the board is viewed through `ownerPlayerId`, which must be a seated player; covered in Task 2 (`ownerLeavingThenBotFillStillPlays`).
4. The last waiting player leaves — the table must disappear so the next quick match does not land on a ghost table; covered in Task 2 and Task 3.
5. Quick match must count against the creation rate limit like table creation does; covered in Task 3 (`RequestLimitFiltersTest`).

---

### Task 1: Public tables in the domain

**Files:**
- Modify: `src/main/java/fr/beelot/game/PrivateTable.java`
- Test: `src/test/java/fr/beelot/game/PublicTableTest.java` (create)

**Interfaces:**
- Produces:
  - `static PrivateTable openPublic(UUID id, UUID ownerId, String ownerName, GameVariant variant, Instant botFillAt)`
  - `boolean publicTable()`, `Instant botFillAt()` (null for private tables)
  - `synchronized void leave(UUID playerId)`
  - `synchronized boolean isEmpty()`
  - `synchronized boolean fillWithBotsIfDue(Instant now)` — `true` only when this call started the game
  - `join(...)` on a public table seating the 4th player sets all seats ready and status `IN_PROGRESS`.
  - `ownerPlayerId()` now `synchronized` (owner is mutable).

- [ ] **Step 1: Write the failing tests** in `PublicTableTest`, using `Instant T0 = Instant.parse("2026-10-02T12:00:00Z")` and `botFillAt = T0.plusSeconds(60)`:
  - `publicTableHasNoInvitationCode`: `invitationCode()` null, `publicTable()` true, `botFillAt()` equals the passed value, status `WAITING_FOR_PLAYERS`.
  - `fourthJoinStartsAPublicTable`: after 3 joins, status `IN_PROGRESS`, 4 seats, all `ready()`.
  - `fourthJoinDoesNotStartAPrivateTable`: regular constructor + 3 joins stays `WAITING_FOR_PLAYERS`.
  - `leaveFreesTheSeatAndTransfersOwnership`: owner + 1 joiner; owner leaves → 1 seat, `ownerPlayerId()` equals joiner id; joiner leaves → `isEmpty()`.
  - `leaveAfterStartFails`: full public table → `leave` throws `PrivateTableConflictException`.
  - `leaveUnknownPlayerFails`: throws `PrivateTableConflictException`.
  - `fillWithBotsWaitsForTheDeadline`: 2 humans; `fillWithBotsIfDue(T0.plusSeconds(59))` false and still waiting; `fillWithBotsIfDue(T0.plusSeconds(60))` true, 4 seats, seats 3–4 named `Camille`, `Luc` with `BOT_TAKEOVER`, all ready, `IN_PROGRESS`; a second call returns false.
  - `fillWithBotsIgnoresPrivateAndStartedTables`: private table → false; auto-started public table → false and still 4 seats.

- [ ] **Step 2: Run to verify failure**
  Run: `./gradlew test --tests fr.beelot.game.PublicTableTest`
  Expected: compilation failure (`openPublic` not defined).

- [ ] **Step 3: Implement in `PrivateTable`**
  Add `publicTable` (final) and `botFillAt` (final) fields; existing constructors set `false`/`null`. `ownerPlayerId` becomes non-final. Extract the bot-adding loop of `startWithBots` into `private void seatBotsAndStart()` (adds bots, sets status) and call it from both `startWithBots` and `fillWithBotsIfDue`; in `fillWithBotsIfDue` first mark existing seats ready. In `leave`, transfer ownership to `seats.get(0)` when the owner left and seats remain.

- [ ] **Step 4: Run to verify pass**
  Run: `./gradlew test --tests fr.beelot.game.PublicTableTest --tests 'fr.beelot.application.privategame.*'`
  Expected: all PASS (existing private-table tests unaffected).

### Task 2: Public-table operations in `PrivateTableService`

**Files:**
- Modify: `src/main/java/fr/beelot/application/privategame/PrivateTableService.java`
- Modify: `src/main/resources/application.properties` (add `beelot.matchmaking.bot-fill-wait=PT60S`, `beelot.matchmaking.sweep-interval=PT5S`, mention `MatchmakingService` in the comment)
- Test: `src/test/java/fr/beelot/application/privategame/PublicTableServiceTest.java` (create)

**Interfaces:**
- Consumes: Task 1 domain API.
- Produces (all `public`):
  - constructor `PrivateTableService(Duration reconnectTimeout, int maxTables, Duration idleExpiry, Duration botFillWait)` — the `@Autowired` one, with `@Value("${beelot.matchmaking.bot-fill-wait:PT60S}")`. Existing 0/1/3-arg constructors delegate with `Duration.ofSeconds(60)`.
  - `PrivateTableAccess openPublic(String playerName, GameVariant variant)` — `botFillAt = Instant.now().plus(botFillWait)`.
  - `PrivateTableAccess joinPublic(UUID tableId, String playerName)` — throws `PrivateTableConflictException("This table does not exist.")` for unknown or non-public tables; calls `beginMatch` if the join started the game.
  - `List<PrivateTable> openPublicTables(GameVariant variant)`.
  - `Optional<PrivateTable> leave(UUID tableId, UUID token)` — empty when the table was removed; also removes the session.
  - `void fillPublicTablesWithBots(Instant now)`.
  - `get(UUID)` also calls the fill for that table with `Instant.now()`.
  - private `void beginMatch(UUID tableId, PrivateTable table)` — puts `MatchScore`, `BiddingState`, drives bot turns; replaces the duplicated code in `start` and `startWithBots`.
  - private `void removeTable(UUID tableId)` — the per-table cleanup now inline in `evictIdle`, null-safe on the invitation code; used by `evictIdle` and `leave`.

- [ ] **Step 1: Write the failing tests** in `PublicTableServiceTest` (service built with `new PrivateTableService(Duration.ofMinutes(2), 2000, Duration.ofHours(2), Duration.ofSeconds(60))`):
  - `openPublicCreatesAWaitingPublicTable`: `publicTable()` true, `invitationCode()` null, `botFillAt()` within `[now+59s, now+61s]`; `openPublicTables(CLASSIC)` contains it, `openPublicTables(CONTREE)` empty.
  - `fourthPublicJoinBeginsTheMatch`: open + 3 `joinPublic`; `bidding(tableId, token)` of the 4th returns a view with 8 cards in `hand()`; `openPublicTables(CLASSIC)` empty.
  - `joinPublicRejectsPrivateTables`: `joinPublic(create("Ana").table().id(), "Ben")` throws `PrivateTableConflictException`.
  - `botFillBeginsTheMatch`: open + 1 join; `fillPublicTablesWithBots(Instant.now().plusSeconds(61))`; status `IN_PROGRESS`; `bidding` for the owner returns 8 cards and does not throw.
  - `fillAfterAutoStartDoesNothing`: full table, record `bidding(...).hand()`, call fill → same hand (match not restarted).
  - `getTriggersBotFill`: service with `botFillWait = Duration.ZERO`; `openPublic` then `get(id).status()` is `IN_PROGRESS`.
  - `ownerLeavingThenBotFillStillPlays`: open + 1 join; owner `leave` returns table with 1 seat; fill due; `bidding(id, joinerToken)` works; and owner's old token now fails `bidding` with `PrivateTableConflictException`.
  - `lastLeaveRemovesTheTable`: open, leave → `Optional.empty()`, `tableCount()` 0, `get(id)` throws.
  - `leaveAfterStartFails`: full table → `leave` throws `PrivateTableConflictException`.

- [ ] **Step 2: Run to verify failure**
  Run: `./gradlew test --tests fr.beelot.application.privategame.PublicTableServiceTest`
  Expected: compilation failure.

- [ ] **Step 3: Implement the service changes** listed under Interfaces. `fillPublicTablesWithBots` iterates `tables.values()`; synchronize on the table around `fillWithBotsIfDue` + `beginMatch` so the sweep and a concurrent `joinPublic` (also synchronized on the table for join + `beginMatch`) cannot begin twice.

- [ ] **Step 4: Run to verify pass**
  Run: `./gradlew test --tests 'fr.beelot.application.privategame.*' --tests 'fr.beelot.application.security.*'`
  Expected: all PASS.

### Task 3: Matchmaking service, endpoints and rate limit

**Files:**
- Create: `src/main/java/fr/beelot/application/matchmaking/MatchmakingService.java`
- Create: `src/main/java/fr/beelot/application/matchmaking/MatchmakingController.java`
- Modify: `src/main/java/fr/beelot/application/privategame/PrivateTableController.java` (leave endpoint; response fields; make `PrivateTableSessionResponse`, `PrivateTableResponse`, `SeatResponse`, `ErrorResponse` and their `from` methods `public` so the matchmaking controller can reuse them)
- Modify: `src/main/java/fr/beelot/application/security/RequestRateLimitFilter.java:82-83` (add `/api/matchmaking/quick-match` to `isCreation`)
- Modify: `src/main/java/fr/beelot/application/HomeController.java` (add `/online/public`, `/online/public/table/{tableId}`)
- Test: `src/test/java/fr/beelot/application/matchmaking/MatchmakingServiceTest.java`, `MatchmakingControllerTest.java` (create); `PrivateTableControllerTest.java`, `RequestLimitFiltersTest.java`, `HomeControllerTest.java` (extend)

**Interfaces:**
- Consumes: Task 2 service API.
- Produces:
  - `MatchmakingService(PrivateTableService)`; `synchronized PrivateTableAccess quickMatch(String playerName, GameVariant variant)` (null variant → `CLASSIC`); `@Scheduled(fixedDelayString = "${beelot.matchmaking.sweep-interval:PT5S}") void sweep()`.
  - Table choice: most seats first, then earliest `botFillAt`; on `PrivateTableConflictException` from `joinPublic`, try the next candidate; none left → `openPublic`.
  - `MatchmakingController` at `/api/matchmaking`, `POST /quick-match` → `201` `PrivateTableSessionResponse`; `PrivateTableConflictException` → `409 {message}`.
  - `PrivateTableController.leave` → `ResponseEntity<PrivateTableResponse>`: `200` with body or `204`.

- [ ] **Step 1: Write the failing tests**
  - `MatchmakingServiceTest` (plain unit test, `new MatchmakingService(new PrivateTableService(...))`):
    - `firstPlayerOpensATable`: one quick match → 1 seat, public.
    - `playersOfTheSameVariantShareATable`: Ana CLASSIC, Ben CLASSIC → same table id, 2 seats.
    - `variantsAreNotMixed`: Ana CLASSIC, Ben CONTREE → different table ids.
    - `prefersTheFullestTable`: open table A via `openPublic` + 2 `joinPublic`, open table B via `openPublic`; quick match lands on A (A then starts).
    - `fourQuickMatchesStartOneGameAndTheFifthOpensANewTable`: 4 calls same id, status `IN_PROGRESS`; 5th call different id with 1 seat.
    - `afterTheLastPlayerLeavesANewTableIsOpened`: quick match, leave, quick match → different table id.
    - `parallelQuickMatchesFillTablesCompletely`: 8 threads via `ExecutorService` + `CountDownLatch` start gate → exactly 2 distinct table ids, each `IN_PROGRESS` with 4 seats.
    - `rejectsABlankName`: `quickMatch(" ", CLASSIC)` throws `PrivateTableConflictException`.
  - `MatchmakingControllerTest` (`@WebMvcTest(MatchmakingController.class)`, `@Import({PrivateTableService.class, MatchmakingService.class})`):
    - `quickMatchSeatsThePlayerAtAPublicTable`: `201`, `$.table.publicTable` true, `$.table.invitationCode` null, `$.table.botFillAt` string, `$.playerToken` string, `$.table.variant` `CONTREE` when requested.
    - `blankNameIsAConflict`: `409`, `$.message` `"Enter a player name."`.
  - `PrivateTableControllerTest`: `waitingPlayerCanLeave` (`200`, seats length 1 after a 2-player table loses one); `lastPlayerLeavingRemovesTheTable` (`204`, then `GET` the table is `409`); `privateTableIsNotPublic` (`$.table.publicTable` false, `$.table.botFillAt` null).
  - `RequestLimitFiltersTest`: quick-match posts beyond the creation limit get `429` (mirror the existing private-table creation case).
  - `HomeControllerTest`: `/online/public` and `/online/public/table/{uuid}` forward to `/index.html` (mirror existing cases).

- [ ] **Step 2: Run to verify failure**
  Run: `./gradlew test --tests 'fr.beelot.application.*'`
  Expected: compilation failure in the new tests.

- [ ] **Step 3: Implement** the files listed. `PrivateTableResponse.from` sets `botFillAt` only while `publicTable()` and status is `WAITING_FOR_PLAYERS`.

- [ ] **Step 4: Run to verify pass**
  Run: `./gradlew test --tests 'fr.beelot.application.*'`
  Expected: all PASS.

### Task 4: Front end

**Files:**
- Modify: `src/main/resources/static/index.html`, `app.js`, `styles.css` (only if the waiting panel needs a rule), `i18n.js`, `i18n-nl.js`
- Test: `src/test/java/fr/beelot/application/PublicMatchmakingUiTest.java` (create, style of `ContreeUiTest`)

**Interfaces:**
- Consumes: Task 3 endpoints and response fields.
- Produces (element ids/names the test pins):
  - Home mode card `<a class="mode-card" href="/online/public">` with copy "Find a game" / "Join an open table — bots fill in if nobody shows up."
  - `<section class="destination-view" data-view="public">` with `<h1>Find a game.</h1>`, form `#quick-match-form` (name input `name="playerName"`, `maxlength="30"`; variant radios `name="public-variant"` CLASSIC checked / CONTREE; submit "Find a game"), `#public-form-message`, back link to `/`.
  - In the `private-table` view: `<div id="public-waiting" hidden>` holding `<p id="public-waiting-message" role="status">` and `<button class="button secondary-button" id="leave-table-button" type="button">Leave table</button>`.

- [ ] **Step 1: Write the failing test** `packagedUiExposesPublicMatchmaking`: `index.html` contains `href="/online/public"`, `data-view="public"`, `id="quick-match-form"`, `name="public-variant"`, `id="public-waiting"`, `id="leave-table-button"`; `i18n.js` and `i18n-nl.js` each contain `"Find a game"`, `"Leave table"` and `"Looking for players… bots take the empty seats in {0} s."` as keys.

- [ ] **Step 2: Run to verify failure**
  Run: `./gradlew test --tests fr.beelot.application.PublicMatchmakingUiTest`
  Expected: FAIL.

- [ ] **Step 3: Implement**
  - `app.js`: `pathToView["/online/public"] = "public"`; `viewForPath` maps `/online/public/table/` to `private-table`. `#quick-match-form` submit posts to `/api/matchmaking/quick-match`, stores the session like `enterPrivateTable` but navigates to `/online/public/table/{id}` (give `enterPrivateTable` a base-path parameter). In `showPrivateTable`, when `table.publicTable`: hide the invitation panel, ready/start buttons, timer settings and `#private-lobby-help`; set the eyebrow to "Public table"; show `#public-waiting` while `WAITING_FOR_PLAYERS`. A one-second interval updates `#public-waiting-message` from `privateTableSnapshot.botFillAt` (seconds left, floored at 0) while the waiting panel is visible. `#leave-table-button` posts to `leave`, removes the session from `sessionStorage` and navigates to `/online/public`. The existing reconnect fallback (`replaceState` to `/online/private`) goes to `/online/public` when the path is a public one.
  - French/Dutch strings for every new text, including the mode-card copy, "Public table", and "Looking for players… bots take the empty seats in {0} s.".

- [ ] **Step 4: Run to verify pass**
  Run: `./gradlew test`
  Expected: BUILD SUCCESSFUL.

- [ ] **Step 5: Manual check** (skill `run`): start the app (`./gradlew bootRun`), open `/online/public` in two browser sessions, find a game in each → same table with 2 seats and a countdown; let it reach 0 → bots join and the auction starts; in a third session leave before the fill → seat freed.

### Task 5: Documentation, verification and the single commit

**Files:**
- Modify: `docs/product-backlog.md` (US-017: expand into a full story with the spec's acceptance criteria; remove it from the post-MVP one-liners or mark it delivered, following how US-016 is recorded)
- Modify: `README.md` (the new route among the routes on line ~17; the bot-fill wait and sweep settings next to the capacity caps)

- [ ] **Step 1: Update the docs** as listed.
- [ ] **Step 2: Full verification**
  Run: `./gradlew test`
  Expected: BUILD SUCCESSFUL, no failures.
- [ ] **Step 3: Commit everything once**

```bash
git add docs/superpowers/specs/2026-10-02-public-matchmaking-design.md docs/superpowers/plans/2026-10-02-public-matchmaking.md docs/product-backlog.md README.md src
git commit -m "feat(matchmaking): find a public game with quick match (US-017)" \
  -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
