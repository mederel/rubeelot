# Plan: Better bot play (US-038)

- Status: Proposed
- Date: 2026-10-02

## Goal

Make games against bots more challenging and bot partners more reliable, and
prove each improvement with a measured win rate instead of judging it from a
few hands.

## Current bot

When this plan was written, bot decisions lived in two places:

- **Bidding** — `BotPlayers.takeAuctionTurn`.
- **Card play** — `GameBoard.automatedCard`, inside the rules engine.

Since US-048, both go through `BotStrategy`, and the current rules live in
`RuleBasedStrategy` (package `fr.beelot.game.bot`).

Bots follow a short list of fixed rules (US-033 to US-037). Both difficulty
levels (`BotDifficulty.RELAXED` and `CHALLENGING`) play the same way.

## Main weaknesses

### Bidding

1. **In classic Belote, bots never take.** `takeAuctionTurn` always passes
   outside Contrée. The human must take every contract, or the cards are
   redealt. Bots never defend against a bot contract either.
2. **In Contrée, bots open at 80 whatever their hand.** The first bot to speak
   opens 80 in its longest suit, even with no jack, no nine and no aces. It
   never bids at a level that matches a strong hand.
3. **Bots never compete in the auction.** They never overcall an opponent's
   contract. They only raise their partner's contract, and only once (US-035).
4. **Bots never coinche,** even against a contract they will clearly defeat.

### Card play

5. **The default card is arbitrary.** When no rule applies, the bot plays
   `candidates.getFirst()`, the first legal card in the order the hand was
   dealt. This decides most plays, including most leads.
6. **No trump management.** A declaring bot does not draw trumps, and it does
   not keep its jack and nine of trumps to take control.
7. **No attempt to win or score tricks.** A bot does not:
   - win a trick with its cheapest winning card;
   - add points to a trick its partner is sure to win;
   - keep its master cards for later tricks.
8. **Little card memory.** Bots only count cards to judge the risk to an ace
   (US-037). They do not track which cards are masters, how many trumps
   remain, or which suits each player is void in.
9. **No cooperation with the partner.** Bots ignore their partner's bid and
   their partner's leads.

### Structure

10. **Strategy and rules are mixed.** The bot's logic sits in `GameBoard`,
    which can see all four hands. So far, the bot only reads its own hand and
    the cards already played, but nothing in the code enforces this.
11. **Nothing measures strength.** Deals use an unseeded `SecureRandom` inside
    `BiddingState`. There is no way to replay deals, run bots against each
    other, or compare two versions of a bot.

## Possible algorithms

| Approach | Strength | Cost | Fit |
| --- | --- | --- | --- |
| Better hand-written rules | Moderate | Low; easy to explain and test | Fixes weaknesses 1–9 quickly. Becomes the Relaxed bot. |
| Perfect-information Monte Carlo (PIMC) | High | Medium: needs a solver, deal sampling, and a time budget | Main candidate for the Challenging bot. |
| Information-set MCTS (ISMCTS) | High; handles hidden cards more soundly than PIMC | High; hard to tune | Consider later, if PIMC hits its limits. |
| Learned models (supervised or reinforcement learning) | Potentially highest | Very high; needs training infrastructure and data | Out of scope for now. |

### PIMC

PIMC chooses a card in three steps:

1. Deal the unseen cards at random many times, consistent with what the bot
   knows: its own hand, the cards played, the voids players have shown and,
   later, the auction.
2. Solve each sampled deal with all cards visible, using alpha-beta search
   over the remaining tricks (a *double-dummy* solver).
3. Play the card with the best average result over all samples.

A Belote round has only 32 cards, and the rules force players to follow suit
and to trump or overtrump. Each player therefore has few legal cards, which
keeps the search small. Bridge and Skat programs use the same method
successfully.

Its known weakness is *strategy fusion*: each sample is solved as if the bot
could see every card, so the bot can overrate plays that only work when you
know where the cards are. In practice it still plays much better than simple
rules. The solver's speed must be benchmarked (US-052) before relying on it.

The same solver can later drive bidding: deal the other three hands at
random, solve each deal, and estimate how many points each trump suit would
make.

### Constraints

- **No cheating.** A bot decides using only what its player can see: its own
  hand, the auction, and the tricks played so far.
- **Response time.** Bot turns run in the HTTP request
  (`BotGameService.play`), so a bot must choose a card within a fixed time
  budget, on the JVM and in the native image. A first target is 200 ms per
  card on a laptop.
- **Repeatable tests.** Given the same seed, a bot makes the same decisions.

## Measuring the win rate

A headless **bot arena** (US-047) plays bots against each other with no UI and
no HTTP.

- **Seeded deals.** Every deal comes from a seed, so a run can be repeated.
- **Duplicate format.** Each deal is played twice, with the two teams swapping
  seats. Both strategies get the same cards, which cancels out most of the
  luck of the deal.
- **Primary metric: average point difference per deal**, with a 95%
  confidence interval. It needs far fewer games than counting match wins.
- **Secondary metrics:**
  - win rate in matches to 1,000 points, with a Wilson 95% interval;
  - contract success rate;
  - coinche success rate;
  - redeal rate in classic Belote;
  - decision time per card (average and 99th percentile).
- **Acceptance rule.** A new strategy replaces the old one only when the lower
  bound of its 95% interval for point difference against the old one is above
  zero, in both classic Belote and Contrée.
- **Sample size.** Start with 10,000 duplicate deal pairs per variant. Adjust
  it once the first run shows how much results vary.
- **Benchmark.** Each run is compared with the previous strategy and with a
  random legal-card bot. The results are written to a report file.

The arena runs through a dedicated Gradle task (for example
`./gradlew botArena`), not as part of `./gradlew test`. A small arena test
with a few hundred deals stays in the normal test suite to check that the
arena still works.

## Order of work

1. **Foundations:**
   - US-047 — bot arena;
   - US-048 — separate strategy from the rules engine;
   - US-049 — difficulty levels play differently.

   Every later story can then be measured.
2. **Bidding** (US-050, US-051, US-055): these fix the most visible problems
   — bots that never take, never coinche, and open blindly at 80.
3. **Card-play rules** (US-053, US-054): together with the bidding stories,
   these make up the improved Relaxed bot.
4. **Search** (US-052, US-056): the solver and PIMC, which become the
   Challenging bot.
5. **Later, measured against the Challenging bot:**
   - PIMC bidding;
   - using the auction to bias the deals PIMC samples;
   - ISMCTS.

Each story records its arena results, against the previous strategy, in its
commit message.

The stories are listed in `docs/product-backlog.md` under **Bot play**.
