# Declare announcements (US-060) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Players declare tierce, cinquante, cent and carrés on their first card; announcements score FFB-style and count towards the contract, with a table option to turn them off.

**Architecture:** A pure `Announcements` class finds a hand's announcements and picks the best declaration. A pure `RoundScoring` class turns a round's totals into a `RoundResult` (Belote and announcements count towards the contract). `GameBoard` records declarations on each player's first card, reveals them after trick 1, and delegates the end-of-round score to `RoundScoring`. Services, controllers, history and the client carry an `announcements` flag and a `declare` flag.

**Tech Stack:** Java 21, Spring Boot 4.1.1, JPA + Flyway on MariaDB, JUnit 5 / MockMvc / Testcontainers, vanilla JS client, Playwright (headless) for the browser check.

**Spec:** `docs/superpowers/specs/2026-10-21-announcements-design.md`

## Global Constraints

- Carrés: jacks 200, nines 150, aces, tens, kings, queens 100; eights and sevens don't count.
- Sequences in the order 7-8-9-10-J-Q-K-A in one suit: 3 cards tierce 20, 4 cinquante 50, 5 or more cent 100.
- A card counts in one announcement at most; carrés come first.
- Best announcement: higher value, then longer run (sequence = its length, carré = 4), then higher top card (A, K, Q, J, 10, 9, 8, 7), then trump suit (a carré is never trump), then declared first in the first trick's play order.
- Only the team with the best announcement scores, and it scores all its announcements.
- Team total = card points + dix de der + its Belote + its announcements if they score. This applies with announcements on or off.
- Classic: taker makes the contract if its total > the defence's total; a tie fails. Contrée: taker's total ≥ the bid.
- Failed: defence scores 162 + every announcement that scored + its own Belote; taker scores its own Belote.
- Capot: capot team scores 250 (+ the bid in Contrée when it is the taker) + its own announcements + its Belote; a capoted taker fails; the other team scores its Belote. Coinche doubles both scores.
- The engine defaults announcements to off (arena, solver, old callers); bot games, pass-and-play and private tables default to on; public matchmaking is always on.
- `declare: true` never fails a card play; it is ignored with nothing to declare, after the first card, or with announcements off.
- Migration is `V9__announcements.sql`; the export stays `beelot-export-2`.
- Every new user-visible string in English, French and Dutch.

## Review Focus

- The next round and a rematch keep the table's announcement choice (bot games and private tables) — tests in Task 4.
- After a player's first card, their "Declare" option is gone, also when the board is fetched again (reload) — test in Task 3 `ownAnnouncementsGoneAfterFirstCard`.
- A round where nobody declares: no reveal panel, and the breakdown line reads "Announcements: none." — tests in Tasks 3 and 6.
- `declare: true` at a table with announcements off scores nothing — test in Task 3 `declareIgnoredWhenOff`.
- A public matchmaking table refuses `POST /announcements` with 409 — test in Task 4.

## File map

- Create `src/main/java/fr/beelot/game/Announcement.java`, `Announcements.java`, `RoundScoring.java`.
- Modify `game/GameBoard.java`, `game/BiddingState.java`, `game/bot/BotTurns.java`, `game/BotGame.java`, `game/PrivateTable.java`.
- Modify `application/bot/BotGameService.java`, `BotGameController.java`, `application/privategame/PrivateTableService.java`, `PrivateTableController.java`.
- Modify `application/history/FinishedMatch.java`, `MatchRecord.java`, `RoundRecord.java`; `application/privacy/DataExport.java`, `DataExportService.java`.
- Create `src/main/resources/db/migration/V9__announcements.sql`.
- Modify `src/main/resources/static/index.html`, `app.js`, `i18n.js`, `i18n-nl.js`, `styles.css`.

Paths under `game/` are relative to `src/main/java/fr/beelot/`; `application/` to `src/main/java/fr/beelot/`. Tests mirror them under `src/test/java/fr/beelot/`.

---

### Task 1: Find and rank announcements

**Files:**
- Create: `game/Announcement.java`, `game/Announcements.java`
- Test: `src/test/java/fr/beelot/game/AnnouncementsTest.java`

**Interfaces:**
- Produces:
  - `public record Announcement(Kind kind, List<GameCard> cards, int value, GameCard.Suit suit)` with `public enum Kind { TIERCE, CINQUANTE, CENT, CARRE }`, `suit` null for a carré, and `public GameCard topCard()` (highest card in the order A, K, Q, J, 10, 9, 8, 7). `cards` is in sequence order (lowest first) for sequences.
  - `public static List<Announcement> Announcements.find(List<GameCard> hand)`: every announcement of the hand, strongest first.
  - `public static Comparator<Announcement> Announcements.strength(GameCard.Suit trump)`: ascending, by the Global Constraints order minus the "declared first" step.
  - `public record Announcements.Declared(int seat, List<Announcement> announcements)`
  - `public static OptionalInt Announcements.bestSeat(List<Declared> inDeclarationOrder, GameCard.Suit trump)`: the seat holding the strongest announcement; on a full tie the earlier entry wins; empty when the list is empty.

- [ ] **Step 1: Write the failing tests** (`AnnouncementsTest`, cards written as `new GameCard("K", HEARTS)`)
  - `carresScoreByRank`: four J → one CARRE 200; four 9 → 150; four A, four 10, four K, four Q → 100 each.
  - `carresOfEightsAndSevensDontCount`: four 8 + four 7 → empty.
  - `sequencesByLength`: 8-9-10 ♥ → TIERCE 20, `topCard` 10♥; 9-10-J-Q ♠ → CINQUANTE 50; 7 to J ♦ → CENT 100; all eight ♣ → one CENT with 8 cards.
  - `sequencesFollowTheNaturalOrderWhateverTheTrump`: 9-10-J ♥ is a tierce (J after 10), and Q-K-A ♥ is a tierce.
  - `carreTakesACardASequenceWouldUse`: four K + Q♥ J♥ → only the CARRE (Q-J-K♥ is not formed from the leftovers).
  - `severalAnnouncementsInOneHand`: 7-8-9 ♠ + J-Q-K ♦ + 2 others → two TIERCEs, strongest (K top) first.
  - `strengthOrder`: CENT beats CARRE of kings (same value, longer); tierce to the A beats tierce to the K; trump tierce beats a plain tierce of the same top; CARRE of jacks beats a CENT.
  - `bestSeatPicksTheStrongestAndTheFirstOnATie`: seats 1 and 2 hold equal plain tierces to the K, seat 1 listed first → 1; seat 3 holding a cinquante added last → 3; empty list → `OptionalInt.empty()`.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*AnnouncementsTest'`
Expected: FAIL (classes missing).

- [ ] **Step 3: Implement `Announcement` and `Announcements`**

Carrés first (ranks J, 9, A, 10, K, Q with four cards), then, per suit, the maximal runs of three or more among the remaining cards in the order 7, 8, 9, 10, J, Q, K, A. Sort the result with `strength(null).reversed()`.

- [ ] **Step 4: Run them to verify they pass** — same command. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main/java/fr/beelot/game/Announcement.java src/main/java/fr/beelot/game/Announcements.java src/test/java/fr/beelot/game/AnnouncementsTest.java
git commit -m "feat(game): find and rank announcements (US-060)"
```

---

### Task 2: Score a round with Belote and announcements towards the contract

**Files:**
- Create: `game/RoundScoring.java`
- Modify: `game/GameBoard.java` (`calculateRoundResult`, `RoundResult`, new `Declaration` record)
- Test: `src/test/java/fr/beelot/game/RoundScoringTest.java`; update existing `GameBoard*Test`, `MatchScoreTest`, history tests that construct `RoundResult`

**Interfaces:**
- Consumes: `Announcement` (Task 1).
- Produces:
  - `GameBoard.RoundResult` gains, at the end: `int northSouthAnnouncements, int eastWestAnnouncements, List<GameBoard.Declaration> announcements` (scored points per team, 0 for the team that doesn't score; the declarations, empty when none).
  - `public record GameBoard.Declaration(int seat, String player, String team, List<Announcement> announcements, boolean scored)`.
  - `public record RoundScoring.Totals(GameVariant variant, boolean northSouthDeclares, int contractValue, boolean coinched, int northSouthCardPoints, int eastWestCardPoints, int northSouthDixDeDer, int eastWestDixDeDer, int northSouthBelote, int eastWestBelote, int northSouthTricks, int northSouthAnnouncements, int eastWestAnnouncements)` (announcements = scored points, one side 0).
  - `public static RoundResult RoundScoring.score(Totals totals, List<GameBoard.Declaration> declarations)`.

- [ ] **Step 1: Write the failing tests** (`RoundScoringTest`, Classic unless said)
  - `classicBeloteCountsTowardsTheContract`: NS takes, NS 65 card + 10 dix + 20 Belote, EW 87 → made, NS 95, EW 87.
  - `classicTieFails`: NS takes, NS 61 + 10 dix + 20 Belote = 91, EW 91 → failed, EW 162, NS 20.
  - `contreeBeloteCountsTowardsTheBid`: bid 100, NS 72 + 10 dix + 20 Belote = 102 → made, NS 102, EW 80.
  - `announcementsCountTowardsTheContract`: NS takes, NS 66 + 10 dix + 20 announcements = 96, EW 86 → made, NS 96, EW 86, `northSouthAnnouncements` 20.
  - `failedContractGivesTheDefenceEveryScoredAnnouncement`: NS takes, NS 40 card + 50 announcements = 90, EW 112 + 10 dix = 122 → failed, EW 162 + 50 = 212, NS 0.
  - `capotAddsTheTeamsOwnAnnouncements`: NS takes all 8 tricks with 50 announcements, EW Belote 20 → NS 300, EW 20.
  - `capotedTakerFailsWhateverItsAnnouncements`: Contrée bid 80, NS takes, NS 200 announcements, EW all tricks (NS tricks 0) → failed, EW 250, NS 0.
  - `coincheDoublesBothScores`: the made case of `announcementsCountTowardsTheContract` coinched in Contrée bid 90 → NS 192, EW 172.
  - `withoutAnnouncementsCardPointsDecide`: NS 82 + 0 dix vs EW 80 → made; NS 81 vs 81 → failed.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*RoundScoringTest'`
Expected: FAIL (class missing).

- [ ] **Step 3: Implement `RoundScoring.score` and make `GameBoard.calculateRoundResult` call it** with `northSouthAnnouncements = eastWestAnnouncements = 0` and no declarations (Task 3 fills them). Add the three `RoundResult` components and `Declaration`; update every `new RoundResult(...)` in tests with `0, 0, List.of()`.

- [ ] **Step 4: Run the whole suite**

Run: `./gradlew test`
Expected: PASS after updating the existing tests whose expectation changes only because Belote now counts towards the contract. List each one changed, with the old and new expectation, in the task report.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(game): count Belote and announcements towards the contract (US-060)"
```

---

### Task 3: Declare on the first card and reveal after the first trick

**Files:**
- Modify: `game/GameBoard.java`, `game/BiddingState.java`, `game/bot/BotTurns.java`
- Test: `src/test/java/fr/beelot/game/GameBoardAnnouncementsTest.java`, `BiddingStateTest` (one test)

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces:
  - `GameBoard.fromBidding(players, hands, trump, declaringPlayerIndex, dealerIndex, boolean announcements)` and `GameBoard.fromContract(players, hands, trump, declaringPlayerIndex, contractValue, coinched, dealerIndex, boolean announcements)`; the existing overloads pass `false`.
  - `public synchronized void play(UUID playerId, GameCard card, boolean declare)`; `play(UUID, GameCard)` calls it with `false`.
  - `GameBoardView` gains, at the end: `boolean announcements, List<Announcement> ownAnnouncements, List<String> declarers, List<GameBoard.Declaration> revealedAnnouncements`.
    - `ownAnnouncements`: `Announcements.find(hand)` for the viewer while announcements are on and the viewer has not played a card this round; else empty.
    - `declarers`: names of the players who declared, while `revealedAnnouncements` is empty.
    - `revealedAnnouncements`: every declaration, with `scored`, from the end of the first trick.
  - `BiddingState.announcements(boolean enabled)` returns `this`; it passes the flag to the board it completes. `boolean BiddingState.announcements()` reads it.
  - `BotTurns.playTurn` plays with `declare = true`.

- [ ] **Step 1: Write the failing tests** (`GameBoardAnnouncementsTest`, crafted hands, North leads)
  - `declaringOnTheFirstCardScoresAfterTheFirstTrick`: North holds 8-9-10 ♣ (clubs not trump), plays with `declare = true`; after the fourth card of trick 1, `northSouthScore` includes 20 more than the trick points, `revealedAnnouncements` has North's tierce with `scored = true`; the final `RoundResult.northSouthAnnouncements()` is 20.
  - `notDeclaringLosesThePoints`: same hand, `declare = false` → no declaration, `northSouthAnnouncements` 0.
  - `declareIgnoredWhenOff`: board built with `announcements = false`, `declare = true` → no declaration, `ownAnnouncements` empty.
  - `declareIgnoredAfterTheFirstCard`: North plays trick 1 without declaring, then trick 2 with `declare = true` → no declaration.
  - `onlyTheBestTeamScores`: North declares a tierce, East a cinquante → `eastWestAnnouncements` 50, North's declaration `scored = false`, `northSouthAnnouncements` 0.
  - `opponentsSeeOnlyNamesDuringTheFirstTrick`: after North declares, East's view has `declarers` ["North"] and empty `revealedAnnouncements`.
  - `ownAnnouncementsGoneAfterFirstCard`: North's view lists the tierce before North's first card and is empty after it, on a fresh `viewFor` too.
  - `nobodyDeclaresNoReveal`: no one declares → after trick 1, `revealedAnnouncements` and `declarers` empty; `RoundResult.announcements()` empty.
  - `BiddingStateTest.completedBoardKeepsTheAnnouncementsFlag`: `new BiddingState(players, CONTREE).announcements(true)`, bid then three passes → the board's view has `announcements` true.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*GameBoardAnnouncementsTest' --tests '*BiddingStateTest'`
Expected: FAIL.

- [ ] **Step 3: Implement.** Record the announcements in `play` before the card leaves the hand; at the end of the first trick call `Announcements.bestSeat` with the declarations in play order, add the scoring team's total to the live score, and keep the result for `RoundScoring`.

- [ ] **Step 4: Run the whole suite**

Run: `./gradlew test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(game): declare announcements on the first card (US-060)"
```

---

### Task 4: Table option and `declare` in the services and APIs

**Files:**
- Modify: `game/BotGame.java`, `game/PrivateTable.java`, `application/bot/BotGameService.java`, `BotGameController.java`, `application/privategame/PrivateTableService.java`, `PrivateTableController.java`
- Test: `BotGameControllerTest`, `BotGameServiceTest`, `PassAndPlayServiceTest`, `PrivateTableControllerTest`, `PrivateTableServiceTest`, `MatchmakingControllerTest`

**Interfaces:**
- Consumes: Task 3.
- Produces:
  - `BotGame` gains a last component `boolean announcements`.
  - `BotGameService.create(BotDifficulty, GameVariant, UUID accountId, boolean announcements)` and `createPassAndPlay(BotDifficulty, GameVariant, List<String>, boolean announcements)`; the existing overloads pass `true`. `nextRound` and `rematch` keep `game.announcements()`. `play(UUID, GameCard, boolean declare)`; `play(UUID, GameCard)` passes `false`.
  - `CreateBotGameRequest` gets `Boolean announcements` (null → `true`); `PlayCardRequest` and `CardPlayRequest` get `boolean declare`.
  - `PrivateTable.announcements()` (default `true`) and `setAnnouncements(UUID playerId, boolean enabled)`, refusing with `PrivateTableConflictException`: "Only the table owner can change announcements.", "Announcements cannot change after the game starts.", "Public tables always play with announcements."
  - `PrivateTableService.setAnnouncements(UUID tableId, UUID token, boolean enabled)`; every `new BiddingState(...)` there calls `.announcements(table.announcements())`; `play(UUID tableId, UUID token, GameCard card, boolean declare)`.
  - `POST /api/private-tables/{tableId}/announcements` with `AnnouncementsRequest(UUID playerToken, boolean enabled)` → `PrivateTableResponse`, which gains `boolean announcements`.
  - `BotGameResponse`, `GameBoardResponse` and `BoardResponse` carry the new view fields: `announcements`, `ownAnnouncements`, `declarers`, `revealedAnnouncements`.

- [ ] **Step 1: Write the failing tests**
  - Bot games: `announcementsAreOnByDefault` (`$.announcements` true on the game and its board); `announcementsCanBeTurnedOff` (`"announcements": false` → false); `declareIsAcceptedOnACardPlay` (`{rank, suit, declare: true}` with a legal card → 200); `nextRoundAndRematchKeepTheChoice` (service: off game → next round's and rematch's boards have `announcements` false).
  - Pass-and-play: `passAndPlayKeepsTheChoice`.
  - Private tables: `ownerTurnsAnnouncementsOff` (200, `$.announcements` false); `otherPlayerCannotChangeAnnouncements` (409); `announcementsCannotChangeAfterTheStart` (409); `nextRoundKeepsTheChoice`.
  - Matchmaking: `publicTablesAlwaysPlayWithAnnouncements` (`$.announcements` true; `POST /announcements` → 409).

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*BotGame*Test' --tests '*PassAndPlay*Test' --tests '*PrivateTable*Test' --tests '*Matchmaking*Test'`
Expected: FAIL.

- [ ] **Step 3: Implement** the interfaces above, following `setTurnTimer` for the private-table option.

- [ ] **Step 4: Run the whole suite**

Run: `./gradlew test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(table): announcements option and declare flag (US-060)"
```

---

### Task 5: Match history and data export

**Files:**
- Create: `src/main/resources/db/migration/V9__announcements.sql`
- Modify: `application/history/FinishedMatch.java`, `MatchRecord.java`, `RoundRecord.java`, `application/bot/BotGameService.java`, `application/privategame/PrivateTableService.java`, `application/privacy/DataExport.java`, `DataExportService.java`
- Test: `MatchHistoryIntegrationTest`, `DataExportIntegrationTest`; update `new FinishedMatch(...)` in `PlayerStatisticsTest`, `MatchHistoryServiceTest`

**Interfaces:**
- Consumes: `RoundResult.northSouthAnnouncements()` / `eastWestAnnouncements()` (Task 2), `BotGame.announcements()` and `PrivateTable.announcements()` (Task 4).
- Produces:
  - V9: `ALTER TABLE match_record ADD COLUMN announcements BOOLEAN NOT NULL DEFAULT FALSE;` and on `match_round`, `north_south_announcements INTEGER NOT NULL DEFAULT 0` and `east_west_announcements INTEGER NOT NULL DEFAULT 0`.
  - `FinishedMatch` gains a last component `boolean announcements`.
  - `DataExport.Match` gains `boolean announcements` after `difficulty`; `DataExport.Round` gains `int northSouthAnnouncements, int eastWestAnnouncements` at the end.

- [ ] **Step 1: Write the failing tests**
  - `MatchHistoryIntegrationTest.recordsAnnouncements`: a match with `announcements = true` and a round with `northSouthAnnouncements` 50 → the rows hold `announcements` TRUE and `north_south_announcements` 50, `east_west_announcements` 0.
  - `DataExportIntegrationTest`: in `exportHoldsAccountEventsAndMatches`, add `$.matches[0].announcements` false and `$.matches[0].rounds[0].northSouthAnnouncements` 0 (the fixture inserts rows without the new columns, which take their defaults); `$.format` stays `beelot-export-2`.

- [ ] **Step 2: Run them to verify they fail**

Run: `./gradlew test --tests '*MatchHistoryIntegrationTest' --tests '*DataExportIntegrationTest'`
Expected: FAIL.

- [ ] **Step 3: Implement** the migration, the entity columns, the two `new FinishedMatch(...)` call sites in the services, and the export queries.

- [ ] **Step 4: Run the whole suite**

Run: `./gradlew test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(history): record and export announcements (US-060)"
```

---

### Task 6: Client: option, declare, reveal, breakdown and rules

**Files:**
- Modify: `src/main/resources/static/index.html`, `app.js`, `i18n.js`, `i18n-nl.js`, `styles.css`
- Test: `src/test/java/fr/beelot/application/AnnouncementsUiTest.java`

**Interfaces:**
- Consumes: the API fields of Task 4.
- Produces (element ids):
  - Solo setup: radios `name="announcements"` values `on` / `off` (on checked); pass-and-play: `name="local-announcements"`; sent as `announcements: true/false` in the create request.
  - Private lobby: `#announcement-settings` (owner only, like `#timer-settings`) with radios `name="table-announcements"`, saved on change by `POST …/announcements`; `#announcements-summary` for everyone.
  - Board: `#declare-option` with checkbox `#declare-announcements` (bot games) and `#private-declare-option` / `#private-declare-announcements` (private tables), shown only when `ownAnnouncements` is not empty, ticked by default; the card request sends `declare` from the checkbox.
  - `#declarers-note` / `#private-declarers-note` during trick 1; `#announcements-panel` / `#private-announcements-panel` from `revealedAnnouncements`, hidden when it is empty.
  - `roundScoreText` appends the announcements line when `board.announcements` is true.
  - Rules view: a `<section>` titled "Announcements" in `.rules-grid`.

Copy (English source; French and Dutch in both dictionaries):
- "Announcements", "On", "Off", "Declare: {0}", "{0} declares.", "{0} score {1} points of announcements.", "Announcements are on.", "Announcements are off."
- "Tierce to the {0}", "Cinquante to the {0}", "Cent to the {0}" ({0} = `cardName(topCard)`), "Carré of {0}" with "jacks", "nines", "aces", "tens", "kings", "queens".
- Breakdown: "Announcements: {0} {1} ({2})." ({0} team, {1} points, {2} kinds joined by " + ", e.g. "tierce + cinquante"), "tierce", "cinquante", "cent", "carré", and "Announcements: none."
- The three new 409 messages of Task 4.
- Rules section: "Declare tierce (three cards in a row, 20), cinquante (four, 50), cent (five or more, 100) and carrés (jacks 200, nines 150, aces, tens, kings or queens 100) when you play your first card. Sequences follow 7, 8, 9, 10, jack, queen, king, ace in one suit; a card counts once, carrés first. Only the team with the best announcement scores, and it scores all of its own: the higher value wins, then the longer run, then the higher card, then trump, then whoever declared first. Belote and announcements count towards the contract."

- [ ] **Step 1: Write the failing test** `AnnouncementsUiTest` (string checks on the packaged files, like `AccountUiTest`)
  - `setupOffersTheOption`: `index.html` has `name="announcements"`, `name="local-announcements"`, `id="announcement-settings"`, `name="table-announcements"`; `app.js` sends `announcements:` in both create requests and posts `/announcements`.
  - `cardPlaySendsDeclare`: `app.js` sends `declare` in both card requests; `index.html` has `id="declare-announcements"` and `id="private-declare-announcements"`.
  - `revealAndBreakdown`: ids `announcements-panel`, `private-announcements-panel`, `declarers-note`, `private-declarers-note`; `app.js` reads `revealedAnnouncements`, `ownAnnouncements`, `declarers`, and contains "Announcements: none.".
  - `rulesExplainAnnouncements`: the rules view contains the rules section's English text.
  - `everyNewTextIsTranslated`: every copy line above is a key in `i18n.js` and `i18n-nl.js`.

- [ ] **Step 2: Run it to verify it fails**

Run: `./gradlew test --tests '*AnnouncementsUiTest'`
Expected: FAIL.

- [ ] **Step 3: Implement the client.**

- [ ] **Step 4: Run the UI tests**

Run: `./gradlew test --tests '*UiTest'`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat(client): declare and reveal announcements (US-060)"
```

---

### Task 7: Backlog and end-to-end checks

**Files:**
- Modify: `docs/product-backlog.md` (US-060: "Delivered: design in `docs/superpowers/specs/2026-10-21-announcements-design.md`"; strike the US-060 rule-source decision as settled: FFB scoring)

- [ ] **Step 1: Full build** — `./gradlew clean build` → BUILD SUCCESSFUL.
- [ ] **Step 2: Headless browser** (Playwright, headless), against a throwaway MariaDB container, not `docker compose` (the local database holds real accounts): start a Classic solo game with announcements on; retry deals until the first hand shows "Declare"; play with it ticked; after trick 1 the panel shows; at the round end the breakdown has the announcements line. Repeat once with announcements off: no option, no line. Check one string in French and Dutch.
- [ ] **Step 3: Commit** `docs(backlog): US-060 delivered`.
