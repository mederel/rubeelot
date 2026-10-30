# Bots bid and play all trumps and no trumps (US-064) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Contrée bots bid, coinche and play tout-atout (TA) and sans-atout (SA), with estimates fitted on bot play and margins tuned in the arena.

**Architecture:** `RuleBasedStrategy` gains suit-contract-only tactics, TA/SA contract and defensive estimates whose features live in a shared `HandFeatures` class, and a TA/SA bidding margin. A new `EstimateFit` tool (arena package) fits the weights by least squares. The arena gains an all-trumps switch, a TA/SA report line and two reference bots.

**Tech Stack:** Java 25, Gradle, JUnit 5. No new dependency.

**Spec:** `docs/superpowers/specs/2026-10-24-bot-all-trumps-design.md`

## Global Constraints

- Branch `feat/bot-all-trumps` (cut from master `2f9ad63`, spec commit `b2b1df6`).
- Commit messages: `type(scope): sentence (US-064)`, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- The suit bidding margin (`BIDDING_MARGIN = 15`), `COINCHE_MARGIN = -4`, `ANNOUNCEMENT_WEIGHT = 0.75` and the suit estimates are unchanged.
- With `allTrumps` off, bots bid exactly as before US-064.
- Sans-atout play is unchanged; the solver and `SamplingStrategy` are unchanged.
- Javadoc on estimates lists the weights in words, in the style of today's `contractEstimate`.
- No UI, API, database or i18n change.
- Run `./gradlew test` before each commit; all tests green (532 at the start).

## Review Focus

- Bot games in the app play with the option on by default: tests that let bots bid (BotGameServiceTest, BotGameControllerTest, already flaky ~1/300) may now meet TA/SA contracts and must stay green — repeated runs in Task 4.
- `allTrumps` off with a hand that would bid SA: the bot must never send `ALL_TRUMPS`/`NO_TRUMPS` (the engine answers 409) — test in Task 4.
- The partner raise under TA/SA at 150 or 160: the raise is capped at 160 and a bot never bids at 160 again — test in Task 6.
- Classic in the arena with `--all-trumps on`: Classic deals must be unaffected (same report as off) — test in Task 2.
- A defensive estimate on a TA/SA contract at 160: the coinche test must not throw or coinche with an empty hand of honours — test in Task 5.

---

### Task 1: Tout-atout play tactics

**Files:**
- Modify: `src/main/java/fr/beelot/game/bot/RuleBasedStrategy.java`
- Create: `src/main/java/fr/beelot/game/arena/PreviousAllTrumpsPlayBot.java`
- Modify: `src/main/java/fr/beelot/game/arena/ArenaBot.java` (add to `AVAILABLE`)
- Test: `src/test/java/fr/beelot/game/bot/RuleBasedStrategyPlayTest.java`

**Interfaces:**
- Produces: `public static RuleBasedStrategy withoutAllTrumpsTactics()` — plays as US-063 (filters on `trump.isTrump`). Arena bot name `previous-all-trumps-play` (plays and bids like `current`, card play from `withoutAllTrumpsTactics()`).

- [ ] **Step 1: Write the failing tests** in `RuleBasedStrategyPlayTest`, each on a `GameBoard.fromContract(players, hands, Trump.ALL_TRUMPS, declarer, 80, false, dealer)` board (add a helper `allTrumpsBoard(declarer, dealer, ana, ben, chloe, david)`; full 8-card hands, so pad with a helper that never adds jacks or nines):
  - `cashesAMasterCardInDefenceUnderAllTrumps`: Ana (seat 0) defends (declarer Ben, seat 1), leads, holds J♠ and low cards in other suits with no jack; asserts the trick is `[J♠]`.
  - `addsPointsToItsPartnersSureTrickUnderAllTrumps`: dealer Ana (seat 0), so Ben leads; Ben plays 7♣, Chloe J♣ (master), David 8♣ with `board.play`; Ana, void in clubs, holds 9♦ (9 points, not master: J♦ unseen) and 7♦; asserts Ana plays 9♦.
  - `winsACheapTrickWithAJackUnderAllTrumps`: opponent leads 7♥ (trick worth 0), Ana holds J♥ and 8♥, last to play; asserts Ana plays J♥.
  - `sameTestsWithoutAllTrumpsTacticsPlayTheOldCards`: the three positions with `RuleBasedStrategy.withoutAllTrumpsTactics()` give the US-063 cards (assert the card each actually plays today, so the reference bot is pinned).

- [ ] **Step 2: Run** `./gradlew test --tests '*RuleBasedStrategyPlayTest'` — Expected: the three new TA tests FAIL.

- [ ] **Step 3: Implement.** Add a `boolean allTrumpsTactics` field (true by default; the existing private constructor gains it; `withoutTrumpControl()` keeps it true). A private helper `trumpTactics(GameCard card, Trump trump)` returns `trump.isTrump(card.suit())` when `!allTrumpsTactics`, else `trump.suitContract() && trump.isTrump(card.suit())`. Use it in place of `trump.isTrump` in: the first-trick defence filter, the master lead filter and the `leads` filter, the partner-sure-trick filter, `lowest`, `trumpHonour`. Leave `opponentsMayHoldTrumps` and the declarer's master-trump lead as they are. `lowest` and `trumpHonour` become instance methods (or take the flag). Update the class Javadoc of `chooseCard` with one sentence: the trump tactics apply to suit contracts; under tout-atout every card is played like a plain card.

- [ ] **Step 4: Add `PreviousAllTrumpsPlayBot`** modelled on `PreviousTrumpsBot`, Javadoc: "Reference bot: bids like the current bot, but plays tout-atout as bots did in US-063, holding back the jack and nine of every suit and never cashing a master card in defence."

- [ ] **Step 5: Run** `./gradlew test` — Expected: all green.

- [ ] **Step 6: Commit** `feat(bot): play tout-atout like plain cards (US-064)`.

---

### Task 2: Arena all-trumps switch and report

**Files:**
- Modify: `src/main/java/fr/beelot/game/arena/BotArena.java`, `BotArenaMain.java`, `ArenaReport.java`, `ArenaBot.java`
- Create: `src/main/java/fr/beelot/game/arena/NoAllTrumpsBiddingBot.java`
- Modify: `build.gradle` (`botArenaArgs`: `'--all-trumps', option('allTrumps', 'on')`; the options comment)
- Test: `src/test/java/fr/beelot/game/arena/BotArenaTest.java`

**Interfaces:**
- Produces: `ArenaReport run(GameVariant variant, int dealPairs, long seed, boolean announcements, boolean allTrumps)`; the existing overloads default `allTrumps` to `true`. `ArenaReport` gains `boolean allTrumps`, `Rate firstAllTrumpsContracts`, `Rate secondAllTrumpsContracts` (made out of TA/SA contracts declared by each bot). Arena bot `no-all-trumps-bidding`.

- [ ] **Step 1: Write the failing tests** in `BotArenaTest`:
  - `classicIgnoresTheAllTrumpsOption`: `current` vs `random`, Classic, 200 pairs, seed 1; `run(..., true, true).pointDifference() == run(..., true, false).pointDifference()` and equal `firstContracts()`.
  - `theReportShowsTheAllTrumpsOption`: a Contrée report with the option on contains `"tout-atout and sans-atout on"`, off contains `"off"` in the header, and contains the line prefix `"Tout-atout and sans-atout contracts: "`.
  - `withTheOptionOffNoBotDeclaresAllTrumps`: Contrée, option off, both `firstAllTrumpsContracts().attempts()` and `second…` are 0.

- [ ] **Step 2: Run** `./gradlew test --tests '*BotArenaTest'` — Expected: compile FAIL.

- [ ] **Step 3: Implement.** `play` calls `.allTrumps(allTrumps)` on the `BiddingState`. `Deal` gains `boolean allTrumpsContract` (`!view.trump().suitContract()` from the board view's trump). `Tally.contract` counts TA/SA contracts and made ones per declaring bot. Header line becomes `%s vs %s, %d duplicate deal pairs, seed %d, announcements %s, tout-atout and sans-atout %s`; add after "Contract success": `Tout-atout and sans-atout contracts: %s %s; %s %s`. `BotArenaMain` reads `--all-trumps on|off` (default `on`, error "All trumps must be on or off."), and the class Javadoc lists it. `NoAllTrumpsBiddingBot` copies `NoAnnouncementBiddingBot`, passing `view.announcements()` and `false` for `allTrumps`; Javadoc: "Reference bot: plays like the current bot, but bids as though the table played without tout-atout and sans-atout (US-064)." Add it to `AVAILABLE`.

- [ ] **Step 4: Run** `./gradlew test` — Expected: all green.

- [ ] **Step 5: Commit** `feat(arena): play with tout-atout and sans-atout (US-064)`.

---

### Task 3: Hand features, least squares and the offence fit

**Files:**
- Create: `src/main/java/fr/beelot/game/bot/HandFeatures.java`
- Create: `src/main/java/fr/beelot/game/arena/LeastSquares.java`
- Create: `src/main/java/fr/beelot/game/arena/EstimateFit.java`
- Modify: `build.gradle` (task `fitEstimates`, `JavaExec`, group `verification`, `mainClass = 'fr.beelot.game.arena.EstimateFit'`, args `--stage`, `--deals` (default `20000`), `--seed` (default `1`) from `-Pstage` (default `offence`), `-Pdeals`, `-Pseed`)
- Test: `src/test/java/fr/beelot/game/bot/HandFeaturesTest.java`, `src/test/java/fr/beelot/game/arena/LeastSquaresTest.java`

**Interfaces:**
- Produces:
  - `public record HandFeatures.Feature(String name, ToIntFunction<List<GameCard>> count)`.
  - `public static List<Feature> offence(Trump trump)` for `ALL_TRUMPS` / `NO_TRUMPS` (throws `IllegalArgumentException` for a suit) and `public static List<Feature> defence(Trump trump)`.
  - SA offence, in order: `aces`, `tens with their ace`, `tens without their ace`, `kings with the ace and ten`, `long cards` (cards beyond the fourth in a suit holding the ace).
  - TA offence: `jacks`, `nines with their jack`, `nines without their jack`, `aces`, `tens`, `belotes` (K and Q of the same suit).
  - Defence (both contracts): `aces`, `tens`, `jacks`, `nines`.
  - `LeastSquares.Fit fit(double[][] rows, double[] targets)` — rows without the intercept column; `record Fit(double intercept, double[] weights, double[] standardErrors, double rSquared)`.

- [ ] **Step 1: Write the failing tests.**
  - `HandFeaturesTest`: for the hand A♠ 10♠ K♠ 9♠ 8♠ J♥ 9♥ Q♦: SA offence counts `[1, 1, 0, 1, 1]`; TA offence counts `[1, 1, 1, 1, 1, 0]`; with K♦ instead of 8♠, TA `belotes` = 1. `offence(Trump.of(GameCard.Suit.HEARTS))` throws.
  - `LeastSquaresTest`: 500 rows of two features drawn from `new SplittableRandom(1)` (0–4), target `50 + 7·x1 − 3·x2` plus noise uniform in ±1; the fit gives intercept within 0.5 of 50, weights within 0.1 of 7 and −3, `rSquared > 0.95`; a feature that is constant zero throws `IllegalArgumentException("Singular fit")`.

- [ ] **Step 2: Run** `./gradlew test --tests '*HandFeaturesTest' --tests '*LeastSquaresTest'` — Expected: compile FAIL.

- [ ] **Step 3: Implement `HandFeatures` and `LeastSquares`.** Least squares: build XᵀX and Xᵀy with a leading 1-column, solve by Gaussian elimination with partial pivoting (pivot below `1e-9` → "Singular fit"), standard errors from `σ²·(XᵀX)⁻¹` diagonal with `σ² = RSS / (n − p)` (invert by solving against the identity).

- [ ] **Step 4: Run** the two tests — Expected: PASS.

- [ ] **Step 5: Implement `EstimateFit` offence stage.** For each deal `i < deals`: shuffle a 32-card deck with `new SplittableRandom(seed).split()`-derived seeds (one `nextLong()` per deal, then `Collections.shuffle(deck, new Random(dealSeed))`), deal 8 cards per seat, and for each of `ALL_TRUMPS`, `NO_TRUMPS` build `GameBoard.fromContract(players, hands, trump, 0, 80, false, 3, true)`, play it to the end with `BotTurns.playTurn(board, new RuleBasedStrategy())` and `continueAfterTrick()`. Target: North–South card points + dix de der + Belote bonus from the `RoundResult`. Row: `HandFeatures.offence(trump)` counts of seat 0's hand. Print, per contract: each feature's name, weight and standard error, the intercept, R², and the sample count. Class Javadoc gives the command and says the output is copied into the spec's Results.

- [ ] **Step 6: Run** `./gradlew fitEstimates -Pstage=offence -Pdeals=20000 -Pseed=1` and then `-Pseed=2`. Drop any feature whose weight lies within two standard errors of zero from `HandFeatures.offence` (adjusting `HandFeaturesTest`), rerun, and paste both runs' final weights into the spec's **Results** under "Offence fit".

- [ ] **Step 7: Run** `./gradlew test` — Expected: all green.

- [ ] **Step 8: Commit** `feat(arena): fit the tout-atout and sans-atout estimates (US-064)` (code, `build.gradle`, spec results).

---

### Task 4: Bid tout-atout and sans-atout

**Files:**
- Modify: `src/main/java/fr/beelot/game/bot/AuctionDecision.java` (`Bid(int value, Trump trump)`)
- Modify: `src/main/java/fr/beelot/game/bot/BotTurns.java`, `src/main/java/fr/beelot/game/arena/BaselineBot.java` (wrap suits with `Trump.of`)
- Modify: `src/main/java/fr/beelot/game/bot/RuleBasedStrategy.java`
- Test: `src/test/java/fr/beelot/game/bot/RuleBasedStrategyBiddingTest.java` (existing `Bid(…, Suit)` assertions become `Trump.of(…)`; `bid.suit()` becomes `bid.trump()`)

**Interfaces:**
- Consumes: `HandFeatures.offence(Trump)` (Task 3).
- Produces: `static int contractEstimate(List<GameCard> hand, Trump trump)` (suit contracts delegate to the existing suit formula, which stays as `contractEstimate(List<GameCard>, GameCard.Suit)`); `static final int ALL_TRUMPS_MARGIN` (start at `20`, tuned in Task 6).

- [ ] **Step 1: Write the failing tests.**
  - `estimatesANoTrumpsHandWithTheFittedWeights` and `estimatesAnAllTrumpsHandWithTheFittedWeights`: one hand each, expected value computed by hand from the weights committed in Task 3 (write the sum in a comment).
  - `bidsNoTrumpsWithAcesAndTens`: hand A♠ 10♠ A♥ 10♥ A♦ 10♦ A♣ K♣, opening (`highestBid` 0, `allTrumps` true) → `Bid` with `Trump.NO_TRUMPS`.
  - `bidsAllTrumpsWithFourJacks`: hand J♠ 9♠ J♥ 9♥ J♦ J♣ A♣ 7♦, opening, option on → `Bid` with `Trump.ALL_TRUMPS`.
  - `neverBidsTheNewContractsWhenTheTableDoesNotPlayThem`: both hands above with `allTrumps` false → the decision is `Pass` or a `Bid` whose `trump().suitContract()` is true.
  - `countsACarreOfAcesAtItsNoTrumpsValue`: with announcements on, a hand with four aces gets `announcementPoints(hand, Trump.NO_TRUMPS) == 200` and its SA decision level is higher than with announcements off.
  - `botGamesStillFinishWithTheOptionOn`: no new test; in Step 4 run `./gradlew test --tests '*BotGameServiceTest' --tests '*BotGameControllerTest'` ten times (`--rerun`), all green (Review Focus).

- [ ] **Step 2: Run** `./gradlew test --tests '*RuleBasedStrategyBiddingTest'` — Expected: FAIL.

- [ ] **Step 3: Implement.** `contractEstimate(hand, Trump)`: for a suit contract, the suit formula; for TA/SA, `intercept + Σ weight·count` over `HandFeatures.offence(trump)`, rounded, with the weights as `private static final` arrays aligned with the feature lists and the Javadoc listing them in words. `contreeDecision` loops over the four suits then, when `view.allTrumps()`, `ALL_TRUMPS` and `NO_TRUMPS`; it compares `estimate + announcementBonus − margin` (margin `biddingMargin` for a suit, `ALL_TRUMPS_MARGIN` otherwise) with strict `>`, so a tie keeps the suit, then applies today's level rule to the best one. The partner branch builds `Bid(…, Trump.of(supported))`. Update the method's Javadoc for the six contracts.

- [ ] **Step 4: Run** `./gradlew test` — Expected: all green (service and controller tests that rely on bot bids may now see TA/SA contracts; fix only tests that assumed a suit, not the code).

- [ ] **Step 5: Run** `./gradlew botArena -Pfirst=current -Psecond=no-all-trumps-bidding -Pvariant=contree -Pdeals=2000` — Expected: TA/SA contracts appear in the report; note the point difference (no acceptance yet).

- [ ] **Step 6: Commit** `feat(bot): bid tout-atout and sans-atout (US-064)`.

---

### Task 5: Coinche tout-atout and sans-atout

**Files:**
- Modify: `src/main/java/fr/beelot/game/arena/EstimateFit.java` (defence stage)
- Modify: `src/main/java/fr/beelot/game/bot/RuleBasedStrategy.java`, `HandFeatures.java` (if features are dropped)
- Test: `RuleBasedStrategyBiddingTest`

**Interfaces:**
- Consumes: `HandFeatures.defence(Trump)`, `LeastSquares.fit`, `contractEstimate(hand, Trump)` (Tasks 3–4).
- Produces: `static int defensiveEstimate(List<GameCard> hand, Trump trump, int contract)`; the suit version keeps its formula. `expectsToDefeat` calls it with `view.highestBidTrump()`.

- [ ] **Step 1: Implement the defence stage.** `--stage defence`: for each seeded deal, a `BiddingState(players, CONTREE, dealer, new Random(dealSeed)).announcements(true).allTrumps(true)` auction played by `RuleBasedStrategy` (via `BotTurns.apply` with each view), coinches disabled by playing `Coinche` decisions as `pass` (so samples are not biased by the coinche being fitted); when the contract is TA or SA, play it out, and add one row per defender: `HandFeatures.defence(trump)` counts of that defender's hand plus the contract value as the last column; target: the defence's card points + dix de der + Belote. Print per contract as in the offence stage.

- [ ] **Step 2: Run** `./gradlew fitEstimates -Pstage=defence -Pdeals=100000 -Pseed=1`, then seed 2. Drop noise features as in Task 3; paste the results into the spec under "Defence fit".

- [ ] **Step 3: Write the failing tests.**
  - `estimatesTheDefenceAgainstNoTrumpsWithTheFittedWeights` and `…AgainstAllTrumps…`: one hand each, expected value from the fitted weights.
  - `coinchesAnAllTrumpsContractWithThreeJacks`: opponent holds 120 TA (`highestBidTrump` `ALL_TRUMPS`, `coincheAllowed` true); hand J♠ 9♠ J♥ 9♥ J♦ A♣ 10♣ A♦ → `Coinche` (adjust the contract value so the fitted estimate clears `162 − contract − 4`; write the arithmetic in a comment).
  - `doesNotCoincheAnAllTrumpsContractWithoutHonours`: hand of sevens and eights against 160 TA → `Pass`.

- [ ] **Step 4: Implement** `defensiveEstimate(hand, Trump, contract)` and switch `expectsToDefeat` to it; Javadoc lists the fitted weights.

- [ ] **Step 5: Run** `./gradlew test` — Expected: all green.

- [ ] **Step 6: Commit** `feat(bot): coinche tout-atout and sans-atout (US-064)`.

---

### Task 6: Partner raise, tuning, acceptance and docs

**Files:**
- Modify: `src/main/java/fr/beelot/game/bot/RuleBasedStrategy.java`
- Modify: `docs/superpowers/specs/2026-10-24-bot-all-trumps-design.md` (Results), `README.md` (arena section), `docs/product-backlog.md` (US-064 delivered)
- Maybe delete: `src/main/java/fr/beelot/game/arena/PreviousAllTrumpsPlayBot.java` and `withoutAllTrumpsTactics()` with their tests
- Test: `RuleBasedStrategyBiddingTest`

**Interfaces:**
- Produces: `static int allTrumpsSupportRaise(List<GameCard> hand, Trump trump)`: 10 per ace under `NO_TRUMPS`, 10 per jack under `ALL_TRUMPS`, at most 20.

- [ ] **Step 1: Write the failing tests.** `raisesItsPartnersNoTrumpsByTenPerAce` (two aces → 20, one → 10, none → 0); `raisesItsPartnersAllTrumpsByTenPerJack`; `raisesItsPartnersAllTrumpsOnlyUpTo160` (partner holds 150 TA, hand with two jacks → `Bid(160, ALL_TRUMPS)`; partner holds 160 → `Pass`); `raisesItsPartnersContractOnlyOnce` (the bot has bid → `Pass`). Rename `readsAnAllTrumpsAuctionWithoutASuitToSupport` to `raisesItsPartnersAllTrumpsWithItsJack`: its hand holds one jack (J♥), so the partner view at 100 expects `Bid(110, Trump.ALL_TRUMPS)`.

- [ ] **Step 2: Run** the bidding tests — FAIL; **implement** the raise in the partner branch of `contreeDecision` (same once-only and 160 rules as `supportRaise`); run `./gradlew test` — PASS.

- [ ] **Step 3: Tune `ALL_TRUMPS_MARGIN`.** For margins 10, 15, 20, 25, 30: `./gradlew botArena -Pfirst=current -Psecond=no-all-trumps-bidding -Pvariant=contree -Pdeals=5000 -Pseed=1` (seed 2 for the best two). Keep the best; record a table (margin, seed 1, seed 2, TA/SA contracts made) in the spec.

- [ ] **Step 4: Measure the raise.** With the chosen margin, the same run with the raise disabled (temporarily return 0). Keep the raise only if seed 1 and seed 2 both improve; otherwise remove it and its tests. Record both figures.

- [ ] **Step 5: Measure the play fixes.** `-Pfirst=current -Psecond=previous-all-trumps-play`, 5,000 pairs, seed 1. Record. Keep the reference bot only if the difference is beyond its margin, otherwise delete it, `withoutAllTrumpsTactics()` and its test.

- [ ] **Step 6: Acceptance.** `current` vs `no-all-trumps-bidding`, 5,000 pairs, seeds 1 and 2: both point differences minus their margins are above 0. If not, stop and report to the human. Check run: `-Pfirst=challenging -Psecond=no-all-trumps-bidding -Pdeals=1000`. Record all in the spec's Results (replace the "Filled in during the implementation" sentence).

- [ ] **Step 7: Docs.** README arena section: `-PallTrumps=on|off` in the example command and sentence ("Contrée deals play with tout-atout and sans-atout, as public tables do; `-PallTrumps=off` plays without them"), the new bots in the list (`no-all-trumps-bidding`, and `previous-all-trumps-play` if kept), the `fitEstimates` command in one short paragraph. Backlog: US-064 gets a "Delivered:" bullet like US-063's (design path, the margin, the acceptance figure); US-063's "Bots play both contracts but do not bid them yet (US-064)." stays as history.

- [ ] **Step 8: Run** `./gradlew test` — Expected: all green.

- [ ] **Step 9: Commit** `feat(bot): raise the partner's tout-atout and sans-atout (US-064)` (if kept) and `docs(backlog): US-064 delivered` with the README and spec results.
