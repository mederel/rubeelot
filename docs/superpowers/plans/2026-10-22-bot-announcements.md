# Bots declare and use announcements (US-061) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bots count their announcements when they bid, remember the cards others reveal, score their samples as the engine scores a round, and the arena plays with announcements by default.

**Architecture:** The bot views (`BiddingState.AuctionView`, `GameBoard.PlayView`) gain the announcement switch and the revealed declarations. `RuleBasedStrategy` adds a weighted announcement term to its estimates. `CardMemory` tracks revealed cards still in other hands; `SamplingStrategy` deals them to their owners and scores samples through `RoundScoring.score`. `BotArena` gains an announcements switch and a reference bot that bids without them.

**Tech Stack:** JUnit 5, Gradle (`./gradlew test`, `./gradlew build`), the bot arena (`fr.beelot.game.arena.BotArenaMain`).

**Spec:** `docs/superpowers/specs/2026-10-22-bot-announcements-design.md`

## Global Constraints

- Bots always declare (`declare = true`); they never keep an announcement quiet.
- Announcements count in bidding only when the view says the round plays with them.
- Contrée: `contractEstimate` and `defensiveEstimate` add `ANNOUNCEMENT_WEIGHT × A` (A = total value of `Announcements.find(hand)`); the weight starts at 0.5.
- Classic: `handScore` adds `A / ANNOUNCEMENT_STEP` hand points (integer division), the upturned card counted as today; the step starts at 50.
- The partner's support raise (US-035) doesn't change.
- Sample scoring uses `RoundScoring.score`; coinche doubling and capot stay out of it.
- The arena defaults to announcements on; `--announcements on|off`; the report header says which.
- A bot sees nothing an opponent at the table couldn't: `revealedAnnouncements` is empty during the first trick.

## Review Focus

- Announcements off (engine default, existing callers, old arena runs): every bot bids and plays exactly as before — tests in Tasks 2 and 3 (`withAnnouncementsOffBidsAsBefore`, existing suites unchanged).
- A revealed card that has since been played must not stay "known" in its owner's hand, or samples fail to deal — test in Task 3 `playedRevealedCardIsForgotten` and Task 4 `dealsRevealedCardsToTheirOwners` (after a revealed card is played).
- The bot's own revealed cards are in its hand, not another seat's — test in Task 3 `ownRevealedCardsAreNotKnownElsewhere`.
- A Classic contract tied on totals fails for the bots as for the engine — test in Task 4 `outcomeMatchesRoundScoring` (tie case).
- A declaration that didn't score (lost to the opponents' better one) adds no points to the sample score — test in Task 4 `unscoredDeclarationsAddNothing`.

## File map

- Modify `game/BiddingState` (AuctionView), `game/GameBoard` (PlayView).
- Modify `game/bot/RuleBasedStrategy`, `game/bot/CardMemory`, `game/bot/SamplingStrategy`.
- Modify `game/arena/BotArena`, `game/arena/BotArenaMain`, `game/arena/ArenaReport`, `game/arena/ArenaBot`; create `game/arena/NoAnnouncementBiddingBot`.
- Tests: `tests/game/bot/{RuleBasedStrategyBiddingTest,CardMemoryTest,SamplingStrategyTest}`, `tests/game/GameBoardAnnouncementsTest`, `tests/game/arena/BotArenaTest`.
- Docs: the spec's "Arena results", `docs/product-backlog.md` (US-061 delivered).

---

### Task 1: Bot views carry announcements

**Files:**
- Modify: `game/BiddingState:172-176, 297-299`
- Modify: `game/GameBoard:188-193, 360-367`
- Modify (constructor calls): `tests/game/bot/RuleBasedStrategyBiddingTest:246-257`, `CardMemoryTest:85`, `SamplingStrategyTest:81`
- Test: `tests/game/GameBoardAnnouncementsTest`

**Interfaces:**
- Produces: `BiddingState.AuctionView(..., boolean coincheAllowed, boolean announcements)` — new last component, filled from `BiddingState.announcements`.
- Produces: `GameBoard.PlayView(..., List<SeatCard> currentTrick, boolean announcements, List<GameBoard.Declaration> revealedAnnouncements)` — two new last components; the list is the board's `revealed` (empty during the first trick), copied.

- [ ] **Step 1: Write the failing tests** in `GameBoardAnnouncementsTest`:

```
@Test
void botsSeeTheRevealOnlyAfterTheFirstTrick() {
    GameBoard board = board(true);
    board.play(north, card("8", CLUBS), true);

    assertTrue(board.playViewFor(east).announcements());
    assertTrue(board.playViewFor(east).revealedAnnouncements().isEmpty());
    finishFirstTrick(board, false);
    board.continueAfterTrick();

    UUID next = board.activePlayerId();
    assertEquals(board.viewFor(next).revealedAnnouncements(), board.playViewFor(next).revealedAnnouncements());
    assertEquals(1, board.playViewFor(next).revealedAnnouncements().size());
}

@Test
void botsSeeAnnouncementsOff() {
    assertFalse(board(false).playViewFor(north).announcements());
}
```

And in `RuleBasedStrategyBiddingTest` (or a new `BiddingStateTest` case if one exists): `new BiddingState(players, GameVariant.CONTREE, 0, new Random(1)).announcements(true).auctionViewFor(first).announcements()` is true, and false without the call.

- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.GameBoardAnnouncementsTest'` — expected: compilation failure (no `announcements()` on `PlayView`).
- [ ] **Step 3: Add the components** and fill them in `auctionViewFor` and `playViewFor`; update the test helpers' constructor calls with `false` / `List.of()`.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(bot): bot views carry announcements (US-061)`.

### Task 2: Bidding counts the bot's announcements

**Files:**
- Modify: `game/bot/RuleBasedStrategy`
- Test: `tests/game/bot/RuleBasedStrategyBiddingTest`

**Interfaces:**
- Consumes: `AuctionView.announcements()` (Task 1); `Announcements.find(List<GameCard>) -> List<Announcement>`, `Announcement.value()`.
- Produces: `static final double ANNOUNCEMENT_WEIGHT = 0.5`, `static final int ANNOUNCEMENT_STEP = 50` (package-private, tuned in Task 6); `static int announcementPoints(List<GameCard> hand)` = sum of `find(hand)` values.
- The estimate functions keep their signatures; the decision methods add the term when `view.announcements()`: Contrée level and coinche use `contractEstimate(...) + round(WEIGHT × A)` and `defensiveEstimate(...) + round(WEIGHT × A)`; Classic compares `handScore(handWithUpturned, suit) + announcementPoints(handWithUpturned) / STEP` with the threshold.

- [ ] **Step 1: Write the failing tests** (hands as in the file's helpers; the AuctionView helpers gain an `announcements` parameter):
  - `announcementPointsAddCarresAndSequences`: four jacks + tierce in another suit → `announcementPoints` = 220.
  - `aCarreRaisesTheContreeBid`: one opening hand holding a carré of aces (A = 100, term 50), evaluated twice: the bid with `announcements` true is the bid with it false plus 50 (both multiples of 10, under the 160 cap).
  - `aCarreMakesAClassicBotTake`: a hand scoring `takingThreshold − 2` in the upturned suit plus a carré (A = 100 → +2) takes with announcements and passes without.
  - `withAnnouncementsOffBidsAsBefore`: the carré hands above, with `announcements` false, give the same decision as the current code (pass / the original level).
  - `aCarreMakesTheBotCoinche`: a defensive hand just below the coinche limit coinches with a carré and announcements on.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.bot.RuleBasedStrategyBiddingTest'` — expected: FAIL.
- [ ] **Step 3: Implement** `announcementPoints` and the terms in `contreeDecision`, `expectsToDefeat`, `classicDecision`; extend the doc comment of the decision methods with one sentence on the term (match the file's tone: what it adds and that the weight was tuned in the arena).
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass (existing bidding tests unchanged: their views have announcements off).
- [ ] **Step 5: Commit** `feat(bot): bots bid on their announcements (US-061)`.

### Task 3: Card memory knows revealed cards

**Files:**
- Modify: `game/bot/CardMemory`
- Test: `tests/game/bot/CardMemoryTest`

**Interfaces:**
- Consumes: `PlayView.revealedAnnouncements()` (Task 1): each `Declaration.seat()` and its announcements' `cards()`.
- Produces: `Set<GameCard> knownCards(int seat)` — revealed cards the seat still holds (not played, never for the bot's own seat); `boolean knownElsewhere(GameCard card, int seat)` — the card is known to be in a seat other than `seat`.
- `mayBeat(seat, winner, lead)`: cards known in another seat don't count, both for the beating card and for `remainingTrumps` in the trump test (a seat may trump only if some unseen trump is not known elsewhere). `master` and `unseen` don't change.

- [ ] **Step 1: Write the failing tests** (extend the file's view helper with a `revealed` list):
  - `revealedCardsAreKnownUntilPlayed`: seat 1 revealed a tierce K♠ Q♠ J♠ → `knownCards(1)` holds the three; `knownElsewhere(K♠, 3)` true, `knownElsewhere(K♠, 1)` false.
  - `playedRevealedCardIsForgotten`: after seat 1 plays J♠ in a trick, `knownCards(1)` holds K♠ and Q♠ only.
  - `ownRevealedCardsAreNotKnownElsewhere`: the bot's own declaration gives `knownCards(view.seat())` empty.
  - `aCardHeldByThePartnerCannotBeatForAnOpponent`: the bot (seat 0) wins a spade trick with the Q♠; the only higher unseen spade, K♠, was revealed by seat 2 (partner) → `mayBeat(1, Q♠, SPADES)` and `mayBeat(3, Q♠, SPADES)` false; without the reveal, true.
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.bot.CardMemoryTest'` — expected: FAIL.
- [ ] **Step 3: Implement**: a `List<Set<GameCard>> known` filled from the declarations (skipping the bot's seat), each card removed when `remember` sees it played; update `mayBeat` and the class doc comment.
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(bot): bots remember revealed announcements (US-061)`.

### Task 4: The Challenging bot deals and scores with announcements

**Files:**
- Modify: `game/bot/SamplingStrategy:111-224`
- Test: `tests/game/bot/SamplingStrategyTest`

**Interfaces:**
- Consumes: `CardMemory.knownCards(int)` (Task 3); `RoundScoring.score(RoundScoring.Totals, List<Declaration>) -> GameBoard.RoundResult`; `PlayView.revealedAnnouncements()`.
- `deal`: each other seat first receives `memory.knownCards(seat)`, its capacity reduced by that many, and those cards are removed from the cards dealt at random. Signatures unchanged.
- `outcome(PlayView view, List<List<GameCard>> hands, int northSouth, int eastWest)` keeps its signature. It builds `RoundScoring.Totals(view.variant(), view.declaringSeat() % 2 == 0, view.variant() == CONTREE ? view.contractValue() : 0, false, northSouth − nsBelote, eastWest − ewBelote, 0, 0, nsBelote, ewBelote, 4, nsAnnouncements, ewAnnouncements)` — solved points already include dix de der and Belote; `4` tricks keeps capot out; the announcement totals sum the values of the `scored()` declarations by team (`seat % 2`). Returns the bot's team's awarded score less the opponents'.

- [ ] **Step 1: Write the failing tests:**
  - `dealsRevealedCardsToTheirOwners`: a view at trick 3 where seat 1 revealed a tierce, one card of it already played; over 200 `deal` calls, the two unplayed cards always land in hand 1 and every hand has its capacity.
  - `outcomeMatchesRoundScoring`: Classic, North–South (seat 0) taking, bot at seat 0: 82/80 made → `outcome` = 2; 81/81 tie fails → −162; 72 + Belote 20 (solved NS 92) vs 70 made only thanks to Belote → 22; Contrée 100 bid, 90 + scored NS tierce 20 → made → 110 − 72.
  - `unscoredDeclarationsAddNothing`: the same Classic 81/81 with a declaration that has `scored()` false for North–South still fails.
  - Update `scoresAFailedContractAsTheDefendersWinningEverything` only if the constructor calls need it (its expected values stay 162 and −58).
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.bot.SamplingStrategyTest'` — expected: FAIL.
- [ ] **Step 3: Implement** the known-cards pre-deal in `deal`/`tryDeal` and `outcome` via `RoundScoring`; update the doc comments of `deal` and `outcome` (drop the "card points without Belote" wording).
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(bot): the Challenging bot deals and scores with announcements (US-061)`.

### Task 5: The arena plays with announcements

**Files:**
- Modify: `game/arena/BotArena`, `BotArenaMain`, `ArenaReport`, `ArenaBot`, `build.gradle:56-63` (`-Pannouncements=on`, passed as `--announcements`, listed in the options comment)
- Create: `game/arena/NoAnnouncementBiddingBot`
- Test: `tests/game/arena/BotArenaTest`

**Interfaces:**
- Consumes: `BiddingState.announcements(boolean)`; `AuctionView` record (Task 1).
- Produces: `BotArena.run(GameVariant variant, int dealPairs, long seed, boolean announcements)`; the existing 3-argument `run` delegates with `true`. `ArenaReport` gains a `boolean announcements` component, printed in the header line as `, announcements on|off`. `BotArenaMain` option `announcements` (default `"on"`, `on|off`, anything else → `IllegalArgumentException("Announcements must be on or off.")`).
- `NoAnnouncementBiddingBot` (`name()` = `"no-announcement-bidding"`): `decideAuction` hands `RuleBasedStrategy` a copy of the view with `announcements` false; `chooseCard` plays like `CurrentBot`. Added to `ArenaBot.AVAILABLE` after `NoCoincheBot`.
- `BotArena.play` plays every card with `board.play(player, card, true)`.

- [ ] **Step 1: Write the failing tests:**
  - `theReportSaysWhetherAnnouncementsWereOn`: `new BotArena(named("current"), named("current")).run(CONTREE, 2, 1, true)` → `report.announcements()` true and `format()` contains `announcements on`; with `false`, `announcements off`.
  - `theDefaultRunPlaysWithAnnouncements`: the 3-argument `run(CONTREE, 2, 1)` gives `announcements()` true.
  - `theNoAnnouncementBiddingBotIsAvailable`: `ArenaBot.named("no-announcement-bidding").name()` equals it.
  - Existing arena tests keep passing (they now run with announcements on).
- [ ] **Step 2: Run** `./gradlew test --tests 'fr.beelot.game.arena.BotArenaTest'` — expected: FAIL.
- [ ] **Step 3: Implement** as above; update the doc comment of `BotArenaMain` (options list) and `BotArena` (announcements).
- [ ] **Step 4: Run** `./gradlew test` — expected: all pass.
- [ ] **Step 5: Commit** `feat(arena): the arena plays with announcements (US-061)`.

### Task 6: Tune and record

**Files:**
- Modify: `game/bot/RuleBasedStrategy` (the two constants, if tuning moves them)
- Modify: `docs/superpowers/specs/2026-10-22-bot-announcements-design.md` ("Arena results"), `docs/product-backlog.md` (US-061 "Delivered" line, like US-060's)

- [ ] **Step 1: Baseline run** with the starting values, each variant, 5,000 deal pairs:
  `./gradlew botArena -Pfirst=current -Psecond=no-announcement-bidding -Pdeals=5000 -Pseed=1` (both variants; the report is in `build/reports/bot-arena/report.txt`). Record point difference ± margin and contracts made, per variant.
- [ ] **Step 2: Tune** the weight over {0.25, 0.5, 0.75, 1.0} (Contrée) and the step over {30, 40, 50, 70} (Classic), one run per value; keep the best point difference. Expected: a positive difference whose 95% interval excludes 0 in at least one variant; if no value beats the reference, keep the weight/step with the best mean and say so in the results.
- [ ] **Step 3: Write** the table (value, difference ± margin, contracts made) into the spec's "Arena results" and the backlog's US-061 "Delivered" line.
- [ ] **Step 4: Run** `./gradlew build` — expected: BUILD SUCCESSFUL.
- [ ] **Step 5: Commit** `feat(bot): tune announcement bidding in the arena (US-061)` (and `docs(backlog): US-061 delivered`).
