# Play all trumps and no trumps in Contrée (US-063) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In Contrée, players can bid tout-atout and sans-atout (FFB §11) when the table option is on; the engine plays and scores both, bots play them correctly, and the history records the option.

**Architecture:** A `Trump` enum (four suits + `ALL_TRUMPS` + `NO_TRUMPS`) replaces the nullable `GameCard.Suit trump` in the rule engine, the board, the auction views and the bots; every "is this a trump" question goes through `Trump.isTrump`. The option follows the US-060 announcements pattern layer by layer (`BiddingState` → `BotGame`/`PrivateTable` → services → API → `match_record` V11 → client).

**Tech Stack:** Java 21, Spring Boot, JPA + Flyway (MariaDB), JUnit 5 / MockMvc / Testcontainers, vanilla JS client, headless Playwright for the browser check.

**Spec:** `docs/superpowers/specs/2026-10-23-all-trumps-design.md`

## Global Constraints

- Sans-atout: order A, 10, K, Q, J, 9, 8, 7; points A 19, 10 10, K 4, Q 3, J 2, 9/8/7 0.
- Tout-atout: order J, 9, A, 10, K, Q, 8, 7 in every suit; points J 13, 9 9, A 6, 10 5, K 3, Q 2, 8/7 0.
- Every contract: 152 card points + 10 dix de der = 162. `RoundScoring` is unchanged.
- No cutting in either contract: a card of another suit than the one led never wins.
- Tout-atout: a player following suit must beat the winning card when they can. Sans-atout: no such obligation.
- Belote: none in sans-atout; in tout-atout one per player per suit holding K+Q, 20 each, up to four.
- Sans-atout carrés: A 200, 10 150, K/Q/J/9 100. Tout-atout carrés: unchanged. The trump tie-break of announcements applies to suit contracts only.
- A bid must be strictly higher in value than the previous one, whatever its contract (no same-value overcall).
- Option off: bidding `ALL_TRUMPS`/`NO_TRUMPS` → 409 "Tout-atout and sans-atout are not played at this table.".
- Option default `true` (absent request field, `PrivateTable`, public matchmaking); `BiddingState` off unless set; old history rows read `false`.
- Private table guards (409), in the order public → owner → started: "Public tables always play with tout-atout and sans-atout.", "Only the table owner can change tout-atout and sans-atout.", "Tout-atout and sans-atout cannot change after the game starts.".
- Migration `V11__all_trumps.sql`: `ALTER TABLE match_record ADD COLUMN all_trumps BOOLEAN NOT NULL DEFAULT FALSE;`. Export stays `beelot-export-2`.
- English names "All trumps" / "No trumps"; French "Tout atout" / "Sans atout"; Dutch "Alles troef" / "Geen troef". Every new user-visible string in all three languages.
- Bots never bid `ALL_TRUMPS`/`NO_TRUMPS` in this story (US-064).

## Review Focus

- A bot defending or partnering a human's tout-atout/sans-atout contract always plays a legal card and the deal completes — Task 2 `BotsPlayNewContractsTest` (both strategies, both contracts, every seat).
- In tout-atout, a player void in the suit led may play any card, and holding only lower cards of the suit led may play any of them — Task 1 `allTrumpsDiscardsWhenVoid`, `allTrumpsFollowsLowWhenItCannotBeat`.
- Two players each holding a Belote in tout-atout both score it, even on the same trick — Task 2 `twoBelotesInAllTrumpsBothScore`.
- A rematch / the next deal keeps the option off when the owner turned it off — Task 3 `nextDealKeepsTheOption` and `rematchKeepsTheOption`.
- Turning the option off hides the two buttons in both auctions, and a stale bid sent anyway is refused — Task 5 UI test and Task 4 `aNewContractIsRefusedWhenOff`.

## File map

- Create: `src/main/java/fr/beelot/game/Trump.java`, `src/main/resources/db/migration/V11__all_trumps.sql`, `src/test/java/fr/beelot/game/TrumpRulesTest.java`, `src/test/java/fr/beelot/game/GameBoardAllTrumpsTest.java`, `src/test/java/fr/beelot/game/bot/BotsPlayNewContractsTest.java`, `src/test/java/fr/beelot/application/AllTrumpsUiTest.java`.
- Modify: `game/BeloteRules.java`, `GameCard.java`, `GameBoard.java`, `Announcements.java`, `Announcement.java` (if it compares the trump), `BiddingState.java`, `BotGame.java`, `PrivateTable.java`; `game/bot/*` and `game/arena/*` (callers); `application/bot/BotGameService.java`, `BotGameController.java`; `application/privategame/PrivateTableService.java`, `PrivateTableController.java`; `application/history/FinishedMatch.java`, `MatchRecord.java`, `RoundRecord.java`; `application/privacy/DataExport.java`, `DataExportService.java`; `static/index.html`, `app.js`, `i18n.js`, `i18n-nl.js`; `docs/product-backlog.md`.

---

### Task 1: `Trump` and the card rules

**Files:**
- Create: `src/main/java/fr/beelot/game/Trump.java`, `src/test/java/fr/beelot/game/TrumpRulesTest.java`
- Modify: `src/main/java/fr/beelot/game/BeloteRules.java`, `GameCard.java`, and every caller of `BeloteRules` and `GameCard.displayOrder` (`GameBoard`, `game/bot/*`, `game/arena/*`, tests), mechanically
- Test: `TrumpRulesTest`, `GameCardDisplayOrderTest`, existing `GameBoard*Test`

**Interfaces:**
- Produces: `public enum Trump { CLUBS, DIAMONDS, HEARTS, SPADES, ALL_TRUMPS, NO_TRUMPS }` with `static Trump of(GameCard.Suit suit)` (null → null), `GameCard.Suit suit()` (null for the two new ones), `boolean isTrump(GameCard.Suit suit)`, `boolean suitContract()`, `String displayName()` ("Clubs", "Diamonds", "Hearts", "Spades", "All trumps", "No trumps").
- Produces: `BeloteRules.strength(GameCard, Trump)`, `points(GameCard, Trump)`, `beats(GameCard contender, GameCard winner, GameCard.Suit lead, Trump)`, `legalCards(List<GameCard> hand, List<GameCard> trick, Trump)`, `winningIndex(List<GameCard>, Trump)`; `GameCard.displayOrder(Trump)` (null allowed: the auction's natural order).
- In this task, `GameBoard` keeps its fields but stores a `Trump` built with `Trump.of(suit)`; the bots replace `card.suit() == trump` by `trump.isTrump(card.suit())` and pass `Trump` to `BeloteRules`. Public signatures outside `BeloteRules`/`GameCard` that take a `Suit` trump keep it until Task 2/3 change them, converting with `Trump.of`.

- [ ] **Step 1: Write the failing tests** in `TrumpRulesTest`:
  - `trumpOfASuit`: `Trump.of(HEARTS).isTrump(HEARTS)` true, `isTrump(SPADES)` false; `ALL_TRUMPS.isTrump(x)` true and `NO_TRUMPS.isTrump(x)` false for every suit; `ALL_TRUMPS.suit()` null; `suitContract()` true for the four suits only.
  - `eachContractTotals152CardPoints`: the sum of `points` over the 32 cards is 152 for `HEARTS`, `ALL_TRUMPS` and `NO_TRUMPS`.
  - `noTrumpsValues` / `allTrumpsValues`: each rank's points per the Global Constraints (e.g. `points(A♣, NO_TRUMPS)` 19, `points(J♣, ALL_TRUMPS)` 13, `points(9♦, ALL_TRUMPS)` 9, `points(Q♠, ALL_TRUMPS)` 2).
  - `noTrumpsOrder` / `allTrumpsOrder`: `winningIndex([10♥, A♥], NO_TRUMPS)` 1, `winningIndex([J♥, 9♥], NO_TRUMPS)` 0, `winningIndex([A♥, 9♥], ALL_TRUMPS)` 1, `winningIndex([9♥, J♥], ALL_TRUMPS)` 1.
  - `anotherSuitNeverWins`: `winningIndex([7♥, J♠], ALL_TRUMPS)` 0 and `winningIndex([7♥, A♠], NO_TRUMPS)` 0.
  - `noTrumpsFollowsWithoutOvertaking`: trick `[K♥]`, hand `[7♥, A♥, A♠]` → legal `[7♥, A♥]`.
  - `noTrumpsDiscardsWhenVoid`: trick `[K♥]`, hand `[A♠, 7♣]` → the whole hand.
  - `allTrumpsMustOvertake`: trick `[A♥]`, hand `[7♥, 9♥, J♠]` → `[9♥]`.
  - `allTrumpsFollowsLowWhenItCannotBeat`: trick `[J♥]`, hand `[7♥, 8♥, A♠]` → `[7♥, 8♥]`.
  - `allTrumpsDiscardsWhenVoid`: trick `[A♥]`, hand `[J♠, 7♣]`, partner not winning → the whole hand.
  - `allTrumpsOvertakesTheWinnerNotTheLastCard`: trick `[9♥, 10♥]` (9♥ wins), hand `[A♥, J♥]` → `[J♥]` (A♥ beats the 10 but not the 9; overtaking applies even when the partner wins, as with trumps today).
  - In `GameCardDisplayOrderTest`: `allTrumpsSortsEverySuitInTrumpOrder` (♣ J after 9 after A…: `[7, 8, Q, K, 10, A, 9, J]` per suit, suits ♣ ♦ ♥ ♠) and `noTrumpsSortsEverySuitInPlainOrder`.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.TrumpRulesTest' --tests '*GameCardDisplayOrderTest'` — expected: compilation failure (`Trump` missing).
- [ ] **Step 3: Implement** `Trump`, the new `BeloteRules` tables (a third order and two point tables chosen by the `Trump`; `legalCards` under the two new contracts: lead cards, overtaking within them only under `ALL_TRUMPS`, else the whole hand), `GameCard.displayOrder(Trump)`, and migrate every caller so the project compiles.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass (suit contracts behave exactly as before).
- [ ] **Step 5: Commit** `feat(game): card rules for all trumps and no trumps (US-063)`.

### Task 2: The board plays the new contracts; bots follow

**Files:**
- Create: `src/test/java/fr/beelot/game/GameBoardAllTrumpsTest.java`, `src/test/java/fr/beelot/game/bot/BotsPlayNewContractsTest.java`
- Modify: `src/main/java/fr/beelot/game/GameBoard.java` (constructor `:48-70`, `fromBidding`/`fromContract` `:72-123`, Belote `:280-308`, views `:317-370`), `Announcements.java`, `Announcement.java`; bot and arena callers of `GameBoard.PlayView.trump()` / `RoundSummary.trump()`; `application/history/RoundRecord.java:54` (`round.trump().name()` keeps working)
- Test: `GameBoardAllTrumpsTest`, `AnnouncementsTest`, `BotsPlayNewContractsTest`, existing `GameBoardBeloteTest`, `GameBoardAnnouncementsTest`

**Interfaces:**
- Consumes: Task 1.
- Produces: `GameBoard.fromContract(List<GamePlayer>, Map<UUID, List<GameCard>>, Trump, int declaringPlayerIndex, int contractValue, boolean coinched, int dealerIndex, boolean announcements)` (and its shorter overloads taking `Trump`); `fromBidding` keeps a `GameCard.Suit` (Classic) and converts.
- Produces: `RoundSummary.trump()`, `PlayView.trump()` of type `Trump`; `GameBoardView.trump()` stays a `String`, now `trump.displayName()`.
- Produces: `Announcements.find(List<GameCard> hand, Trump)`, `strength(Trump)`, `bestSeat(List<Declared>, Trump)`. The existing `find(hand)` is removed; callers pass the board's `Trump` (bots: the auction's candidate suit via `Trump.of`).
- Belote: a private record `Belote(UUID playerId, GameCard.Suit suit)` list replacing `belotePlayerId`; each Belote tracks its own cards played and awards 20 once. Messages unchanged.

- [ ] **Step 1: Write the failing tests:**
  - `GameBoardAllTrumpsTest.twoBelotesInAllTrumpsBothScore`: North holds K♥ Q♥, East K♠ Q♠, `ALL_TRUMPS`; after the deal, `RoundResult` Belote is 20 for each team.
  - `onePlayerWithTwoBelotesScoresForty`: North holds K♥ Q♥ K♣ Q♣ under `ALL_TRUMPS` → North–South Belote 40.
  - `noBeloteInNoTrumps`: North holds K♥ Q♥ under `NO_TRUMPS` → 0 Belote, no "declares Belote" message.
  - `aFullDealTotals162`: for `ALL_TRUMPS` and `NO_TRUMPS`, playing a fixed deal with the first legal card each turn, the two teams' card points + dix de der sum to 162.
  - `AnnouncementsTest.noTrumpsCarres`: four aces under `NO_TRUMPS` value 200, four tens 150, four nines 100, four jacks 100; under `ALL_TRUMPS` four jacks 200, four nines 150.
  - `AnnouncementsTest.noTrumpTieBreakOutsideSuitContracts`: two tierces to the ace, the second in hearts, `bestSeat` under `ALL_TRUMPS` and under `NO_TRUMPS` returns the first declarer; under `HEARTS` the hearts one.
  - `BotsPlayNewContractsTest` (parameterised over `ALL_TRUMPS`/`NO_TRUMPS` × each `BotStrategies` difficulty): build `GameBoard.fromContract` with a seeded deal, every seat played by the strategy; each chosen card is in `legalCards`, and the board completes with card points + dix de der = 162. One case where seat 0 is the human taker's seat played by `BeloteRules.legalCards(...).getFirst()`, the others by bots.
  - `DoubleDummySolver`: `solverScoresANewContractDeal` — the solver's value of a full `NO_TRUMPS` deal from the taker's side is between 0 and 162.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.*'` — expected: FAIL.
- [ ] **Step 3: Implement** the `Trump` board, the Belote list (`hasBelote` per suit where `trump.isTrump(suit)`), the sans-atout carré table, the tie-break on `trump.suitContract()`, and the bot/solver/arena signature changes.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(game): play and score all trumps and no trumps (US-063)`.

### Task 3: The auction and the table option

**Files:**
- Modify: `src/main/java/fr/beelot/game/BiddingState.java`, `BotGame.java`, `PrivateTable.java` (`:24`, near `setAnnouncements` `:202-211`, and where it builds each `BiddingState`), `game/bot/AuctionDecision.java` and its callers in `BotTurns`/strategies (a bot `Bid` keeps a `GameCard.Suit`; `BotTurns` passes `Trump.of(suit)`)
- Test: `src/test/java/fr/beelot/game/ContreeBiddingStateTest.java`, `PrivateTableTest.java`, `BiddingStateTest.java`

**Interfaces:**
- Consumes: Task 2 (`GameBoard.fromContract(..., Trump, ...)`).
- Produces: `BiddingState.allTrumps(boolean) -> BiddingState`, `boolean allTrumps()`; `bid(UUID, int, Trump)`; `BiddingView.highestBidSuit` and `AuctionView.highestBidSuit` become `Trump`; `AuctionView` gains a final component `boolean allTrumps`; `SeatCall.suit` becomes `Trump trump` (Classic `TRUMP` calls store `Trump.of(suit)`).
- Produces: `BotGame(UUID id, BotDifficulty difficulty, GameVariant variant, List<GameSeat> seats, boolean announcements, int target, boolean allTrumps)`.
- Produces: `PrivateTable.setAllTrumps(UUID playerId, boolean enabled)`, `boolean allTrumps()` (true initially); every deal's `BiddingState` gets `.allTrumps(allTrumps)`.

- [ ] **Step 1: Write the failing tests:**
  - `ContreeBiddingStateTest.bidsAllTrumpsWhenOn`: option on, `bid(p, 100, ALL_TRUMPS)`, three passes → `completedBoard()` round summary trump `ALL_TRUMPS`, contract 100.
  - `bidsNoTrumpsWhenOn`: same with `NO_TRUMPS`; the view's `highestBidSuit` is `NO_TRUMPS`.
  - `aNewContractIsRefusedWhenOff`: option off (the default) → `bid(p, 100, ALL_TRUMPS)` throws `PrivateTableConflictException` "Tout-atout and sans-atout are not played at this table."; a suit bid still works.
  - `noOvercallAtTheSameValue`: 100 ♥ then 100 `NO_TRUMPS` → throws "Your bid must be higher than the current contract."; 110 `NO_TRUMPS` → accepted.
  - `PrivateTableTest.ownerTurnsAllTrumpsOff`; `onlyTheOwnerChangesAllTrumps`, `allTrumpsIsFixedOnceStarted`, `publicTablesAlwaysPlayAllTrumps` — each with the Global Constraints message.
  - `PrivateTableTest.nextDealKeepsTheOption`: owner turns it off, starts; after a redeal (everyone passes) the new auction still refuses `ALL_TRUMPS`.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.*'` — expected: FAIL.
- [ ] **Step 3: Implement**; every `new BotGame(...)` (main and tests) passes `true` until Task 4 passes the real value.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(game): bid all trumps and no trumps when the table plays them (US-063)`.

### Task 4: Services, API and history

**Files:**
- Create: `src/main/resources/db/migration/V11__all_trumps.sql`
- Modify: `src/main/java/fr/beelot/application/bot/BotGameService.java` (`create`, `createPassAndPlay`, `rematch`, `bid` `:217`, `toFinishedMatch`, each new deal's `BiddingState`), `BotGameController.java` (`:30-60`, `:78`, `:133-165`), `src/main/java/fr/beelot/application/privategame/PrivateTableService.java` (`:306`, new `setAllTrumps`, `toFinishedMatch`), `PrivateTableController.java` (`:84-92`, `:128`, `:219`, `:247-259`), matchmaking (public tables keep the default `true`), `application/history/FinishedMatch.java`, `MatchRecord.java`, `application/privacy/DataExport.java`, `DataExportService.java:100-130`
- Test: `BotGameServiceTest`, `PassAndPlayServiceTest`, `BotGameControllerTest`, `PrivateTableServiceTest`, `PrivateTableControllerTest`, `PublicTableServiceTest`, `MatchHistoryIntegrationTest`, `DataExportIntegrationTest`, every `new FinishedMatch(...)` in tests

**Interfaces:**
- Consumes: Task 3.
- Produces: `BotGameService.create(BotDifficulty, GameVariant, UUID accountId, boolean announcements, int target, boolean allTrumps)` and `createPassAndPlay(..., boolean announcements, int target, boolean allTrumps)`; shorter overloads pass `true`. `bid(UUID, int, Trump)` in both services.
- Produces: `PrivateTableService.setAllTrumps(UUID tableId, UUID token, boolean enabled) -> PrivateTable`.
- API: `CreateBotGameRequest` gains `Boolean allTrumps` (null → true); `ContractBidRequest.suit` becomes `Trump`; `POST /api/private-tables/{tableId}/all-trumps` with `record AllTrumpsRequest(UUID playerToken, boolean enabled)` → `PrivateTableResponse`; `BotGameResponse` and `PrivateTableResponse` gain `boolean allTrumps`.
- History: `FinishedMatch(..., int target, boolean allTrumps)`; `MatchRecord` column `all_trumps` (`@Column(name = "all_trumps", nullable = false)`), `boolean allTrumps()`; `DataExport.Match` gains `boolean allTrumps` right after `target`.

- [ ] **Step 1: Write the failing tests:**
  - `BotGameControllerTest.allTrumpsDefaultsToOn`: create Contrée without `allTrumps` → `$.allTrumps` true; with `false` → false.
  - `BotGameControllerTest.bidsAllTrumps`: Contrée game, human to bid, `{"value": 100, "suit": "ALL_TRUMPS"}` → 200 and `$.highestBidSuit` `ALL_TRUMPS`; `{"value": 100, "suit": "HEARTS"}` still 200 on another game.
  - `BotGameControllerTest.aNewContractIsRefusedWhenOff` → 409 with the Global Constraints message.
  - `BotGameServiceTest.rematchKeepsTheOption` (and in `PassAndPlayServiceTest`, `PrivateTableServiceTest`).
  - `PrivateTableControllerTest.ownerTurnsAllTrumpsOff`: POST `/all-trumps` `false` → 200, `$.allTrumps` false; a non-owner → 409; a public matchmaking table → 409 "Public tables always play with tout-atout and sans-atout.".
  - `MatchHistoryIntegrationTest.recordsAllTrumps`: a finished bot game with the option off stores `all_trumps` false, on stores true; `oldMatchesReadWithoutAllTrumps`: a row inserted without the column reads false.
  - `DataExportIntegrationTest`: the match has `"allTrumps": true` and a round with `"trump": "NO_TRUMPS"` when one was played (insert the round record directly); the format is still `beelot-export-2`.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.application.*'` — expected: FAIL.
- [ ] **Step 3: Implement** V11 and the plumbing; recorders pass `game.allTrumps()` / `table.allTrumps()`.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(api): choose and bid all trumps and no trumps (US-063)`.

### Task 5: Client, rules reference and translations

**Files:**
- Create: `src/test/java/fr/beelot/application/AllTrumpsUiTest.java` (pattern: `AnnouncementsUiTest`, `MatchTargetUiTest`)
- Modify: `src/main/resources/static/index.html` (solo setup near `:100-101`, pass-and-play setup, lobby near `:271-273`, auction suit groups `:204` and `:328`, rules section, tutorial glossary), `static/app.js` (create requests, lobby rendering and change handler like `table-announcements`, auction rendering `:448-477`, contract/trump lines `:497-508`, `:721-722`, `suitName`, `orderHandForDisplay` `:929`), `static/i18n.js`, `static/i18n-nl.js`

**Interfaces:**
- Consumes: Task 4 (`allTrumps` on requests and responses; `POST /api/private-tables/{id}/all-trumps` with `{playerToken, enabled}`; `suit` may be `ALL_TRUMPS`/`NO_TRUMPS`; the board's `trump` string is the display name, `"All trumps"`/`"No trumps"` for the new contracts).
- Radio groups `name="all-trumps"` (solo), `name="local-all-trumps"` (pass and play), `name="table-all-trumps"` (lobby), values `on|off`, `on` checked; shown only when the variant is Contrée; the lobby disables them for non-owners, hides them on public tables, and shows `#all-trumps-summary`: "Tout-atout and sans-atout are played." / "Only the four suits are played.".
- Auction suit groups `contract-suit` and `private-contract-suit` gain inputs `ALL_TRUMPS` ("All trumps") and `NO_TRUMPS` ("No trumps"), hidden when the game's or table's `allTrumps` is false (and never shown in Classic).
- Trump line: "{0} are trump" for a suit; "All trumps" or "No trumps" for the new contracts. Contract line: "100 All trumps".
- `orderHandForDisplay`: under "All trumps", trump ranks for every suit; under "No trumps", plain ranks; suits in natural order for both.

- [ ] **Step 1: Write the failing tests** in `AllTrumpsUiTest`: the three option groups exist with `on` checked; both auction groups have `ALL_TRUMPS` and `NO_TRUMPS` inputs; `app.js` sends `allTrumps` in both create requests and posts to `/all-trumps`; `app.js` hides the two inputs when `allTrumps` is false; the rules section has a "Tout-atout and sans-atout" heading and the two point lists (e.g. "Ace 19", "Jack 13"); the glossary has both terms; every new string is in `i18n.js` and `i18n-nl.js`.
- [ ] **Step 2: Run** `./gradlew test --tests '*AllTrumpsUiTest'` — expected: FAIL.
- [ ] **Step 3: Implement** markup, script and translations (French: "Tout atout", "Sans atout", "Tout atout et sans atout", "Le tout atout et le sans atout se jouent.", "Seules les quatre couleurs se jouent."; Dutch: "Alles troef", "Geen troef", "Alles troef en geen troef", "Alles troef en geen troef worden gespeeld.", "Alleen de vier kleuren worden gespeeld."; the error and rules strings likewise, matching the dictionaries' existing terms for trump, Belote and carré).
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass (existing translation tests included).
- [ ] **Step 5: Commit** `feat(client): bid and play all trumps and no trumps (US-063)`.

### Task 6: Verify in a browser and record

**Files:**
- Modify: `docs/product-backlog.md` (US-063 criteria, functional task, the roadmap decision line, "Delivered" line)

- [ ] **Step 1: Run** `./gradlew clean build` — expected: BUILD SUCCESSFUL.
- [ ] **Step 2: Browser check (headless Chrome/Playwright)**, the app on a throwaway database as in earlier stories: solo Contrée → bid 80 All trumps → the trump line reads "All trumps", the hand sorts every suit J-high, a full deal completes and the breakdown totals 162 (plus Belote/announcements); a new game, bid No trumps → "No trumps", no Belote message; solo with the option off → the two buttons are hidden; private table: the owner turns it off, a second context sees "Only the four suits are played." with the choice disabled; French and Dutch labels show. Stop the app and the database afterwards.
- [ ] **Step 3: Update** the backlog: replace "Each ranks above the suits at the same level." with "A bid must still be higher in value than the previous one (FFB §4.1).", add the table option criterion, tick the functional task with the FFB scales, strike the roadmap decision on the scales, and add the "Delivered" line pointing to the spec.
- [ ] **Step 4: Commit** `docs(backlog): US-063 delivered`.
