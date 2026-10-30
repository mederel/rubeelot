# Choose the match target (US-062) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Solo, pass-and-play and private-table players choose a match target of 500, 1,000, 1,500 or 2,000 points; the match ends there, the score shows it, and the history records it.

**Architecture:** `MatchScore` takes a validated `int` target (default 1,000). `BotGame` and `PrivateTable` carry the target, the services build their `MatchScore` with it and keep it for the next round and rematch, the controllers accept and return it, `match_record` stores it (V10), and the client offers a button group and shows "to {target}" in the score line. It mirrors the US-060 announcements option layer by layer.

**Tech Stack:** Java 21, Spring Boot, JPA + Flyway (MariaDB), JUnit 5 / MockMvc / Testcontainers, vanilla JS client, headless Playwright for the browser check.

**Spec:** `docs/superpowers/specs/2026-10-22-match-target-design.md`

## Global Constraints

- Allowed targets: `MatchScore.TARGETS = List.of(500, 1000, 1500, 2000)`; default 1,000 everywhere (absent request field, arena, `new MatchScore()`, old history rows).
- Rule: first team to reach the target wins; both reach it in the same round → higher score wins; exact tie → another round.
- The next round and a rematch keep the target.
- Invalid target in a request → 400 `{"message": "The match target must be 500, 1,000, 1,500 or 2,000 points."}`.
- Private table guards (409): "Public tables always play to 1,000 points.", "Only the table owner can change the match target.", "The match target cannot change after the game starts.".
- Migration `V10__match_target.sql`: `ALTER TABLE match_record ADD COLUMN target INTEGER NOT NULL DEFAULT 1000;`. Export stays `beelot-export-2`.
- Score line copy: "Match score — North–South {0} · East–West {1} · to {2}".
- Rules copy: "The first team to reach the match target (1,000 points unless the players choose 500, 1,500 or 2,000) wins the match. When both teams reach it in the same round, the higher score wins; an exact tie is settled by another round."
- Every new user-visible string in English, French and Dutch.

## Review Focus

- A public matchmaking table always plays to 1,000, even if a request tries to set a target — test in Task 2 `PublicTableServiceTest.publicTablesStayAtOneThousand` (service) and Task 3 (409 from the endpoint).
- A rematch after a match to 500 is again to 500, for bot games and private tables — tests in Task 2 `rematchKeepsTheTarget`.
- A request with `target: null` or no `target` creates a match to 1,000 — test in Task 3 `targetDefaultsToOneThousand`.
- Matches recorded before V10 show 1,000 in statistics and the export — test in Task 4 `oldMatchesReadAsOneThousand` (insert a row without the column value).
- The private-table lobby shows the chosen target to non-owners and refreshes it when the owner changes it — test in Task 5 (resource test: `table.target` rendered for every player, controls disabled unless owner) and the Task 6 browser check.

## File map

- Create: `src/main/java/fr/beelot/game/InvalidMatchTargetException.java`, `src/main/resources/db/migration/V10__match_target.sql`, `src/test/java/fr/beelot/application/MatchTargetUiTest.java`.
- Modify: `game/MatchScore.java`, `game/BotGame.java`, `game/PrivateTable.java`; `application/bot/BotGameService.java`, `BotGameController.java`; `application/privategame/PrivateTableService.java`, `PrivateTableController.java`; `application/history/FinishedMatch.java`, `MatchRecord.java`, `PlayerStatistics.java`; `application/privacy/DataExport.java`, `DataExportService.java`; `static/index.html`, `static/app.js`, `static/i18n.js`, `static/i18n-nl.js`; `docs/product-backlog.md`.
- Tests: `game/MatchScoreTest.java`, `game/PrivateTableTest.java`, `application/bot/BotGameServiceTest.java`, `BotGameControllerTest.java`, `application/privategame/PrivateTableServiceTest.java`, `PrivateTableControllerTest.java`, `application/history/*`, `application/privacy/DataExportIntegrationTest.java`, and the constructor calls of `BotGame`/`FinishedMatch` in tests.

---

### Task 1: Engine — the match target

**Files:**
- Create: `src/main/java/fr/beelot/game/InvalidMatchTargetException.java`
- Modify: `src/main/java/fr/beelot/game/MatchScore.java`, `BotGame.java`, `PrivateTable.java:24, 200-210`
- Test: `src/test/java/fr/beelot/game/MatchScoreTest.java`, `src/test/java/fr/beelot/game/PrivateTableTest.java`

**Interfaces:**
- Produces: `InvalidMatchTargetException extends IllegalArgumentException` with the spec's 400 message.
- Produces: `MatchScore.TARGETS`, `MatchScore(int target)` (throws `InvalidMatchTargetException`), `MatchScore()` = 1,000, `int target()`, `static int requireTarget(int target)` (returns it or throws).
- Produces: `BotGame(UUID id, BotDifficulty difficulty, GameVariant variant, List<GameSeat> seats, boolean announcements, int target)`; the compact constructor calls `MatchScore.requireTarget`.
- Produces: `PrivateTable.setTarget(UUID playerId, int target)`, `int target()` (1,000 initially), guards in the order public → owner → started, as `setAnnouncements`.

- [ ] **Step 1: Write the failing tests:**
  - `MatchScoreTest.eachTargetEndsTheMatchWhenReached`: for each of `TARGETS`, recording rounds of 250/0 ends the match on the first round where North–South ≥ target, and not before.
  - `MatchScoreTest.aTieAtTheTargetPlaysOn`: target 500, North–South and East–West both at 520 → not complete; next round 10/0 → complete, North–South wins.
  - `MatchScoreTest.anInvalidTargetIsRefused`: `new MatchScore(750)` throws `InvalidMatchTargetException` with the spec message; `new MatchScore().target()` is 1,000.
  - `PrivateTable`: `ownerSetsTheTargetBeforeTheStart` (target 1,500 after the call); `onlyTheOwnerSetsTheTarget`, `targetIsFixedOnceStarted`, `publicTablesStayAtOneThousandPoints` — each `assertThrows(PrivateTableConflictException.class, …)` with the spec message; `setTarget(owner, 750)` throws `InvalidMatchTargetException`.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.*'` — expected: compilation failure.
- [ ] **Step 3: Implement**; update every `new BotGame(...)` call (main and tests) with `1000` until Task 2 passes the real value; restate `MatchScore`'s Javadoc with the target.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(game): a match plays to its target (US-062)`.

### Task 2: Services keep the target

**Files:**
- Modify: `src/main/java/fr/beelot/application/bot/BotGameService.java:88-150, 376-405`, `src/main/java/fr/beelot/application/privategame/PrivateTableService.java:281, 403-410, 415, 657`
- Test: `src/test/java/fr/beelot/application/bot/BotGameServiceTest.java`, `PassAndPlayServiceTest.java`, `src/test/java/fr/beelot/application/privategame/PrivateTableServiceTest.java`, `PublicTableServiceTest.java`

**Interfaces:**
- Consumes: Task 1.
- Produces: `BotGameService.create(BotDifficulty, GameVariant, UUID accountId, boolean announcements, int target)` and `createPassAndPlay(BotDifficulty, GameVariant, List<String> seats, boolean announcements, int target)`; the existing shorter overloads pass 1,000.
- Produces: `PrivateTableService.setTarget(UUID tableId, UUID token, int target) -> PrivateTable`.
- Produces: `BotGameService.MatchStatus(int northSouth, int eastWest, boolean complete, String winner, int target)` and the same for `PrivateTableService.MatchStatus`.
- `MatchScore` is built with `game.target()` / `table.target()` at start and in `rematch`.

- [ ] **Step 1: Write the failing tests:**
  - `BotGameServiceTest.aMatchToFiveHundredEndsThere`: create with target 500; play rounds (the file's `playRound` helper) until complete; `matchStatus.target()` is 500 and the winner's score ≥ 500 and < 500 + the last round's award.
  - `BotGameServiceTest.rematchKeepsTheTarget`: target 1,500; `rematch` → `matchStatus(...).target()` 1,500. Same in `PassAndPlayServiceTest` and `PrivateTableServiceTest` (`rematchKeepsTheTarget`).
  - `PrivateTableServiceTest.ownerSetsTheTarget`; `PublicTableServiceTest.publicTablesStayAtOneThousand` (setTarget on a matchmaking table → 409 message, and its match status target is 1,000).
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.application.bot.*' --tests 'fr.beelot.application.privategame.*Service*'` — expected: FAIL (compile).
- [ ] **Step 3: Implement**; `toFinishedMatch` keeps passing what it passes today (Task 4 adds the target).
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(game): bot games and private tables keep their match target (US-062)`.

### Task 3: API carries the target

**Files:**
- Modify: `src/main/java/fr/beelot/application/bot/BotGameController.java:30-60, 113-150`, `src/main/java/fr/beelot/application/privategame/PrivateTableController.java:84-90, 150-175, 213, 228-246`
- Test: `src/test/java/fr/beelot/application/bot/BotGameControllerTest.java`, `src/test/java/fr/beelot/application/privategame/PrivateTableControllerTest.java`

**Interfaces:**
- Consumes: Task 2.
- `CreateBotGameRequest` gains `Integer target` (null → 1,000); `BotGameResponse` and `PrivateTableResponse` gain `int target`; both match-status responses gain `target`.
- `POST /api/private-tables/{tableId}/target` with `record TargetRequest(UUID playerToken, int target)` → `PrivateTableResponse`.
- Both controllers: `@ExceptionHandler(InvalidMatchTargetException.class) @ResponseStatus(BAD_REQUEST)` returning their `ErrorResponse(message)`.

- [ ] **Step 1: Write the failing tests:**
  - `BotGameControllerTest.targetDefaultsToOneThousand`: create without `target` → `$.target` 1000; match status `$.target` 1000.
  - `BotGameControllerTest.chosenTargetIsReturned`: `target: 1500` → `$.target` 1500 (solo and pass-and-play).
  - `BotGameControllerTest.anInvalidTargetIsABadRequest`: `target: 750` → 400 and `$.message` the spec message.
  - `PrivateTableControllerTest.ownerChangesTheTarget`: POST `/target` 500 → 200, `$.target` 500; a non-owner → 409; 750 → 400; on a public matchmaking table → 409 "Public tables always play to 1,000 points.".
- [ ] **Step 2: Run** `./gradlew test --tests '*BotGameControllerTest' --tests '*PrivateTableControllerTest'` — expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(api): choose the match target (US-062)`.

### Task 4: History, export and statistics record the target

**Files:**
- Create: `src/main/resources/db/migration/V10__match_target.sql`
- Modify: `src/main/java/fr/beelot/application/history/FinishedMatch.java`, `MatchRecord.java:50-84`, `PlayerStatistics.java:24, 96`; `src/main/java/fr/beelot/application/privacy/DataExport.java:25`, `DataExportService.java:100-130`; the two `toFinishedMatch` methods (`BotGameService.java:400`, `PrivateTableService.java:458`); `StatisticsController` if it maps `RecentMatch` by hand
- Test: `src/test/java/fr/beelot/application/history/MatchHistoryIntegrationTest.java`, `PlayerStatisticsTest.java`, `src/test/java/fr/beelot/application/privacy/DataExportIntegrationTest.java`, and every `new FinishedMatch(...)` in tests

**Interfaces:**
- Consumes: Tasks 1–2 (`game.target()`, `table.target()`).
- Produces: `FinishedMatch(..., boolean announcements, int target)`; `MatchRecord` column `target` (`@Column(nullable = false)`), `int target()`; `PlayerStatistics.RecentMatch(..., boolean won, int target)`; `DataExport.Match(..., boolean announcements, int target, ...)` — `target` right after `announcements`.

- [ ] **Step 1: Write the failing tests:**
  - `MatchHistoryIntegrationTest.recordsTheTarget`: a finished bot game to 500 is stored with `target` 500 and the statistics' recent match has `target` 500.
  - `MatchHistoryIntegrationTest.oldMatchesReadAsOneThousand`: insert a `match_record` row with plain SQL leaving out `target` (as V9-era rows) → statistics show 1000.
  - `DataExportIntegrationTest`: the exported match has `"target": 500` for a match to 500 and the export version is still `beelot-export-2`.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.application.history.*' --tests '*DataExportIntegrationTest'` — expected: FAIL.
- [ ] **Step 3: Implement** V10 and the fields; recorders pass `game.target()` / `table.target()`.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(history): record the match target (US-062)`.

### Task 5: Client — choose and show the target

**Files:**
- Modify: `src/main/resources/static/index.html` (setup forms near the announcements choices at `:100-101`, the local pass-and-play form, the private-table lobby near `#announcements-summary` `:266`, the rules section `:358`, the statistics table header), `static/app.js` (`:259-261` lobby, `:480-481` and `:721` score lines, `:1426` and `:1445` create requests, `:1625-1633` lobby change handler, the statistics recent-matches rows `:1888-1889`), `static/i18n.js`, `static/i18n-nl.js`
- Create: `src/test/java/fr/beelot/application/MatchTargetUiTest.java` (pattern: `AnnouncementsUiTest`)

**Interfaces:**
- Consumes: Task 3 (`target` on requests and responses; `POST /api/private-tables/{id}/target` with `{playerToken, target}`), Task 4 (`recentMatches[].target`).
- Radio groups: `name="target"` (solo), `name="local-target"` (pass and play), `name="table-target"` (lobby), values `500|1000|1500|2000`, ids `target-500`, …; 1,000 checked. The lobby uses `setChoice("table-target", String(table.target))`, disables the inputs for non-owners and hides the group on public tables, and shows "The match is played to {0} points." in a `#target-summary` line.
- New strings: "Match target", "{0} points" (button labels via `formatNumber`), "The match is played to {0} points.", "Target", the score line, the rules sentence, "Only the table owner can change the match target.", "The match target cannot change after the game starts.", "Public tables always play to 1,000 points.", "The match target must be 500, 1,000, 1,500 or 2,000 points.".

- [ ] **Step 1: Write the failing tests** in `MatchTargetUiTest`: the three radio groups exist with the four values and 1,000 checked; `app.js` sends `target` in both create requests and posts to `/target`; the score line key with `· to {2}` is used three times; `recentMatches` rows render `match.target`; every new string is in `i18n.js` and `i18n-nl.js`; the rules section contains the new rules sentence and no longer the old "The first team to reach 1,000 points wins the match." sentence.
- [ ] **Step 2: Run** `./gradlew test --tests '*MatchTargetUiTest'` — expected: FAIL.
- [ ] **Step 3: Implement** markup, script and translations (French: "Objectif de la partie", "{0} points", "La partie se joue en {0} points.", "Objectif", "Score de la partie — Nord–Sud {0} · Est–Ouest {1} · en {2}"; Dutch: "Doel van de partij", "{0} punten", "De partij wordt gespeeld tot {0} punten.", "Doel", "Partijstand — Noord–Zuid {0} · Oost–West {1} · tot {2}"; match the dictionaries' existing team names and the existing score-line translations).
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass (existing translation tests included).
- [ ] **Step 5: Commit** `feat(client): choose and show the match target (US-062)`.

### Task 6: Verify in a browser and record

**Files:**
- Modify: `docs/product-backlog.md` (US-062 "Delivered" line, like US-060's)

- [ ] **Step 1: Run** `./gradlew clean build` — expected: BUILD SUCCESSFUL.
- [ ] **Step 2: Browser check (headless Chrome/Playwright)**, with the app on a throwaway database as in earlier stories: solo setup → choose 500 → the score line ends "· to 500"; pass-and-play with 2,000 → "· to 2,000"; private table: the owner picks 1,500, a second browser context sees "The match is played to 1,500 points." with the choice disabled; French and Dutch labels show. Expected: every check passes; stop the app and the database afterwards.
- [ ] **Step 3: Write** the backlog's US-062 "Delivered" line pointing to the spec.
- [ ] **Step 4: Commit** `docs(backlog): US-062 delivered`.
