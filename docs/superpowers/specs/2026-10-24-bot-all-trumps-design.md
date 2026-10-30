# Bots bid and play all trumps and no trumps (US-064) — design

## Goal

In Contrée, bots bid tout-atout (TA) and sans-atout (SA) when the table plays
them (US-063) and their hand is worth it, coinche them when they expect to
defeat them, and play tout-atout as well as a suit contract. The arena plays
with the option on by default, as public tables do, and shows that bots that
bid the new contracts beat bots that don't.

## Decisions

- **A fitted linear estimate**, like the suit estimate. A fitting tool plays
  seeded deals with the current bots and fits the points made to features of
  the hand by least squares; the rounded weights go into the code. Bidding by
  solving sampled deals was rejected: a 32-card solve is too slow at bid time,
  and the Relaxed bot would still need a formula.
- **The Relaxed and Challenging bots share the change.** The Challenging bot
  bids through `RuleBasedStrategy` and plays its first two tricks with it.
- **TA and SA have their own bidding margin**, tuned in the arena. They swing
  more than a suit contract; the suit margin (15) is unchanged.
- **The tout-atout play fixes come first**, because the fit measures how the
  bots play.
- **The solver needs no change.** US-063 already checks it against an
  exhaustive search in every contract.
- No change to the UI, the API or the database.

## Fitting tool

`fr.beelot.game.arena.EstimateFit`, run with `./gradlew fitEstimates`
(options `-Pdeals`, `-Pseed`, `-Pstage=offence|defence`), prints each
feature's weight with its standard error and the fit's R².

- **Least squares**: a small `LeastSquares` class solves the normal equations
  (Gaussian elimination with partial pivoting); no new dependency.
- **Offence** (`stage=offence`): each seeded deal is played twice, once as a
  TA and once as an SA contract at 80, taken by seat 0, with announcements on.
  The current bots play it out. The target is the taker's team's points: card
  points, dix de der, and Belote.
  - SA features: aces; tens with their ace; tens without; kings with the ace
    and ten of their suit; cards beyond the fourth in a suit headed by the ace.
  - TA features: jacks; nines with their jack; nines without; aces; tens;
    Belotes (king and queen of the same suit).
- **Defence** (`stage=defence`): natural arena auctions with the new bidding;
  on each TA or SA contract, each defender's hand is one sample: the target
  is 162 less the taker's total (see the results), fitted to that hand plus
  the contract value, the shape of today's `150 − 1.3 × contract + …`. Features: aces,
  tens, jacks and nines, each counted separately for each contract.
- A feature whose weight lies within two standard errors of zero is dropped
  and the fit is run again. The final weights are rounded and written into the
  estimates and into the results below.

## Bidding (`RuleBasedStrategy`)

- `contractEstimate(hand, Trump)` covers the six contracts: suits as today,
  TA and SA with the fitted weights. The Javadoc lists the weights in the
  style of today's.
- `contreeDecision`: with `view.allTrumps()` on, the bot compares the four
  suits, TA and SA, each with the announcement bonus of its own `Trump` (the
  SA carré values come from `Announcements.find`), and keeps the best
  estimate less its margin: `biddingMargin` for a suit, `ALL_TRUMPS_MARGIN`
  for TA and SA. The level rules are unchanged (multiple of 10, opens at 80,
  overcalls when higher). With the option off, the bot bids as today.
- `AuctionDecision.Bid` carries a `Trump`; `BotTurns.apply` passes it on.
- **Coinche**: `defensiveEstimate(hand, Trump, contract)` uses the fitted
  defence weights under TA and SA and today's formula for a suit; the
  `COINCHE_MARGIN` test is unchanged.
- **Partner's TA/SA contract**: the bot passes, as before. A raise of 10 per
  ace in SA and per jack in TA (at most 20) was tried and lost points in the
  arena (see the results).

## Tout-atout play (`RuleBasedStrategy`)

The tactics that set trumps apart only make sense in a suit contract. A
helper `trumpTactics(card, trump)`, true when `trump.suitContract()` and the
card is a trump, replaces `trump.isTrump` in:

- keeping trumps out of the first trick in defence;
- the master-card lead (a defender in TA now cashes its masters);
- the card added to a trick the partner is sure to win;
- `lowest`'s preference for keeping trumps;
- `trumpHonour` (in TA, a J or 9 may win a cheap trick).

The declarer's lead of a master trump while an opponent may hold one stays
as it is: in TA it leads a master card. Sans-atout play is unchanged.

## Arena

- `BotArena.run` takes `allTrumps` and sets it on each Contrée deal's
  `BiddingState`; Classic ignores it.
- `BotArenaMain` reads `--all-trumps on|off` (on by default); Gradle passes
  `-PallTrumps`. Reports made before US-064 stay comparable only with
  `--all-trumps off`.
- `ArenaReport` prints the option in its header and a new line: each bot's TA
  and SA contracts with their made rate.
- New reference bot **`no-all-trumps-bidding`**: plays like `current`, bids
  as though the option were off (like `NoAnnouncementBiddingBot`).
- **`previous-tout-atout-play`** keeps the US-063 play while the play fixes
  are measured. It is kept, like `PreviousPlayBot`, only if it stays useful as
  a reference.

## Tuning and acceptance

- Play fixes: `current` against `previous-tout-atout-play`, Contrée, option
  on, 5,000 deal pairs (once the bots bid TA and SA).
- `ALL_TRUMPS_MARGIN` and the support raise: `current` against
  `no-all-trumps-bidding`, Contrée, announcements and option on, 5,000
  duplicate deal pairs, seeds 1 and 2. The margin is kept where the point
  difference is highest.
- **Acceptance**: `current` beats `no-all-trumps-bidding` in point difference,
  with a 95% margin above zero on both seeds.
- Check: `challenging` against `no-all-trumps-bidding`, 1,000 deal pairs.

## Testing

- `LeastSquares`: recovers known weights from synthetic data.
- `contractEstimate` and `defensiveEstimate` on hand-picked TA and SA hands.
- Bidding: four aces and tens bid SA with the option on, and a suit or a pass
  with it off; four jacks bid TA; a carré of aces counts at its SA value; the
  partner's TA/SA raise (if kept).
- Play: in TA a defender leads a master; a bot adds points to its partner's
  sure trick; a bot wins a cheap trick with a jack.
- `BotArena`: a short seeded Contrée run with the option on produces TA or SA
  contracts and counts them in the report.
- The existing bot, service and controller tests run as before; bot games
  already play with the option on by default.

## Results

The fitted weights, the tuning of the margin, the raise and the coinche
margin, and the acceptance runs. All arena runs are Contrée, 5,000 duplicate
deal pairs, announcements and the option on, unless stated.

### Offence fit

`./gradlew fitEstimates -Pstage=offence -Pdeals=20000`, the current bots
(with the tout-atout play of this story), announcements on. Points the
taker's team makes (cards, dix de der, Belote) for each feature of the
taker's hand, with their standard errors. Every feature lies beyond two
standard errors of zero on both seeds, so all are kept; the code uses the
average of the two seeds, rounded.

Tout-atout (R² 0.188 and 0.191):

| Feature | Seed 1 | Seed 2 | Weight |
| --- | --- | --- | --- |
| Intercept | 57.67 | 57.91 | **58** |
| Jacks | 14.87 ± 0.33 | 15.12 ± 0.34 | **15** |
| Nines with their jack | 15.19 ± 0.56 | 15.33 ± 0.57 | **15** |
| Nines without their jack | 5.69 ± 0.33 | 5.33 ± 0.33 | **6** |
| Aces | 2.42 ± 0.30 | 1.78 ± 0.30 | **2** |
| Tens | 0.66 ± 0.30 | 1.02 ± 0.30 | **1** |
| Belotes | 19.05 ± 0.54 | 19.60 ± 0.55 | **19** |

Sans-atout (R² 0.177 and 0.188):

| Feature | Seed 1 | Seed 2 | Weight |
| --- | --- | --- | --- |
| Intercept | 58.01 | 57.69 | **58** |
| Aces | 15.09 ± 0.31 | 15.63 ± 0.31 | **15** |
| Tens with their ace | 10.47 ± 0.59 | 10.86 ± 0.59 | **11** |
| Tens without their ace | 3.95 ± 0.31 | 4.41 ± 0.31 | **4** |
| Kings with the ace and ten | 4.13 ± 1.19 | 4.89 ± 1.20 | **5** |
| Long cards | 9.34 ± 1.19 | 11.97 ± 1.21 | **11** |

The low R² is expected: the partner's hand, which the taker doesn't see,
makes most of the difference.

### Defence fit

`./gradlew fitEstimates -Pstage=defence -Pdeals=100000`: natural auctions
with the new bidding and the bots' coinches read as passes. The target is
**162 less the taker's total** (cards, dix de der, Belote and scored
announcements) rather than the defence's card points: the contract fails
exactly when it exceeds 162 − contract, which is what the coinche test
asks, and many high tout-atout bids stand on a carré of jacks that the
defence's card points don't see. (A first fit on the defence's points
had a jack-less hand coinche 160 tout-atout.) Each defender's hand is one
sample, so the two hands of a deal share a target and the errors below
are about √2 too small; a feature is kept only when it lies beyond two
standard errors on both seeds. Fitted with aces, tens, jacks and nines,
tout-atout dropped the aces (0.39 ± 0.26) and sans-atout the nines
(0.64 ± 0.43). The slopes keep two decimals: rounding them moves a 160
estimate by up to 6 points.

Tout-atout (39,748 and 39,980 defender hands; R² 0.482 and 0.476):

| Feature | Seed 1 | Seed 2 | Weight |
| --- | --- | --- | --- |
| Intercept | 204.44 | 201.77 | **203** |
| Tens | −1.25 ± 0.24 | −0.91 ± 0.24 | **−1** |
| Jacks | 13.11 ± 0.30 | 13.04 ± 0.29 | **13** |
| Nines | 2.42 ± 0.25 | 2.70 ± 0.25 | **3** |
| Contract value | −1.85 ± 0.01 | −1.82 ± 0.01 | **−1.84** |

Sans-atout (11,710 and 11,696 defender hands; R² 0.760 and 0.768):

| Feature | Seed 1 | Seed 2 | Weight |
| --- | --- | --- | --- |
| Intercept | 236.06 | 233.36 | **235** |
| Aces | 24.54 ± 0.56 | 23.76 ± 0.54 | **24** |
| Tens | −2.33 ± 0.49 | −2.46 ± 0.47 | **−2** |
| Jacks | 1.74 ± 0.44 | 1.44 ± 0.43 | **2** |
| Contract value | −2.31 ± 0.01 | −2.27 ± 0.01 | **−2.29** |


### Bidding margin

`current` against `no-all-trumps-bidding`, with the partner raise. Point
difference per deal and the made rate of `current`'s TA and SA contracts.

| `ALL_TRUMPS_MARGIN` | Seed 1 | Seed 2 | TA/SA contracts made (seed 1) |
| --- | --- | --- | --- |
| 10 | −9.16 ± 2.10 | | 66.9% of 3,301 |
| 15 | −2.61 ± 1.69 | | 73.4% of 2,212 |
| 20 | +1.06 ± 1.36 | | 79.4% of 1,433 |
| **25** | **+2.21 ± 1.01** | **+2.41 ± 1.04** | 84.6% of 850 |
| 30 | +1.99 ± 0.75 | +2.26 ± 0.80 | 88.1% of 496 |
| 35 | +1.66 ± 0.59 | +1.72 ± 0.61 | 92.7% of 301 |
| 40 | +1.17 ± 0.43 | | 97.6% of 164 |

The margin is **25**.

### Partner raise

With the margin at 25, the raise of the partner's TA/SA contract lost points
on both seeds: +2.21 ± 1.01 and +2.41 ± 1.04 with it, +2.40 ± 0.99 and
+2.72 ± 1.02 without. It was removed.

### Coinche margin

The reference bot coinches `current`'s TA and SA contracts with the fitted
estimate, and wins fewer of those coinches (about 42%) than on suits. A
separate TA/SA coinche margin was tried head to head (a bot with that margin
against `current`, both bidding TA and SA, seed 1): −20 gave −0.25 ± 0.34,
−12 +0.06 ± 0.23, 0 +0.00 ± 0.10, 8 +0.02 ± 0.11, 16 +0.03 ± 0.11. No value
beats the shared `COINCHE_MARGIN` of −4 beyond noise, so it is kept for every
contract.

### Tout-atout play

`current` against `previous-all-trumps-play`, seed 1: **+1.06 ± 0.64**
(95% CI +0.43 to +1.70), over 803 TA and SA contracts each. The reference
bot is kept.

### Acceptance

`current` against `no-all-trumps-bidding`, margin 25, no raise:

| Seed | Point difference | Matches won | TA/SA contracts made |
| --- | --- | --- | --- |
| 1 | **+2.40 ± 0.99** (CI +1.41 to +3.39) | 51.2% of 1,175 | 86.6% of 833 |
| 2 | **+2.72 ± 1.02** (CI +1.70 to +3.75) | 51.2% of 1,165 | 87.8% of 812 |

Both margins lie above zero. Check: `challenging` against
`no-all-trumps-bidding`, 1,000 deal pairs, seed 1: +21.13 ± 3.42, 91.4% of
162 TA and SA contracts made (the Challenging bot's card play accounts for
most of the difference).

## Out of scope

- Bidding by solving sampled deals, and the Challenging bot searching the
  first two tricks.
- Reading the opponents' or partner's TA/SA bid into the play.
- Overcalling the partner's TA/SA contract with a suit.
- Any UI, API or database change.
