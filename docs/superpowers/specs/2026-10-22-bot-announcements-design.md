# Bots declare and use announcements (US-061) — design

## Goal

Bots play by the announcement rules of US-060: they count their announcements
when they bid, they remember the cards that others reveal, and the Challenging
bot scores its samples as the engine scores the round. The arena plays with
announcements by default, as public tables do.

## Decisions

- **Bots keep declaring everything** (`BotTurns.playTurn` already passes
  `declare = true`). They never keep an announcement quiet.
- **Bidding: announcement value times a weight tuned in the arena**
  (approach A). The estimates stay as fitted; announcements add a term to them.
  A full-value term was rejected: a declared tierce often loses to the
  opponents' better announcement, and overbidding costs 162 points.
- **Scoring the samples with `RoundScoring`**, so bots judge a contract by the
  US-060 rule: in Classic the taker's total, Belote and announcements included,
  must beat the defence's; in Contrée it must reach the bid. This replaces the
  old "card points without Belote ≥ 82" check in `SamplingStrategy.outcome`.
- **The arena plays with announcements on by default**; `--announcements off`
  turns them off. Reports made before US-061 stay comparable only with
  `--announcements off`.
- **Capot stays out of the bots' scoring**, as today: the solver counts points,
  not tricks.

## What bots see

- `BiddingState.AuctionView` gains `announcements` (whether the round plays
  with them). Bids come before anyone declares, so the bot learns nothing else.
- `GameBoard.PlayView` gains `announcements` and `revealedAnnouncements`: the
  declarations every player sees once the first trick is over (US-060), with
  their seats, cards and whether they scored. During the first trick the list
  is empty, as on the board.

## Bidding

`RuleBasedStrategy` counts the bot's own announcements only when
`view.announcements()` is true. `Announcements.find` gives them; their total
value is `A`.

- **Contrée:** `contractEstimate` adds `ANNOUNCEMENT_WEIGHT × A`. The weight
  starts at 0.5 and is tuned in the arena. The coinche test
  (`defensiveEstimate`) doesn't count announcements: the defence's
  announcements never lower the taker's total (see the arena results).
- **Classic:** `handScore` (counting the upturned card, as today) adds one
  hand point per `ANNOUNCEMENT_STEP` points of `A`, rounded down. The step
  starts at 50 and is tuned in the arena.
- The partner's support raise (US-035) doesn't change.

The arena gains a reference bot **`no-announcement-bidding`**: it plays like
the current bot and bids as though announcements were off.

## Play

- **Card memory.** `CardMemory` records, for each other seat, the revealed
  cards it still holds (revealed, not yet played). `mayBeat` no longer counts
  a card that is known to be in another seat's hand, and `master` is
  unchanged (a revealed card is still unseen and may still beat). The
  rule-based bot uses this from the second trick on.
- **Samples.** `SamplingStrategy.deal` gives each seat its known cards first,
  then deals the remaining unseen cards as today (capacities, voids, most
  constrained first). A seat's capacity counts its known cards.
- **Scoring a sample.** `outcome` builds `RoundScoring.Totals` from the solved
  points (card points with dix de der, minus Belote, which `Totals` keeps
  apart), the sample's Belote seat, and the scored announcements of
  `revealedAnnouncements`, and returns the bot's team's awarded score less the
  opponents'. The coinche doubling is left out, as it multiplies both scores
  and doesn't change which card is best.

## Arena

- `BotArena` takes the announcements switch, deals with it
  (`BiddingState` constructor with announcements), and plays every card with
  `declare = true`.
- `BotArenaMain` reads `--announcements on|off` (default `on`), and
  `ArenaReport.format` prints it in the header.
- The tuning run compares `current` against `no-announcement-bidding` over
  5,000 deal pairs per variant. The weight and the step are kept where the
  point difference is highest, and the result is written below.

## Testing

- `RuleBasedStrategy`: estimates and `handScore` with and without
  announcements; with announcements off the bot bids as before; a carré makes a
  Contrée bot bid higher and a Classic bot take a hand it would pass.
- `CardMemory`: a revealed card held by the partner doesn't make an opponent
  "may beat"; once played it is forgotten.
- `SamplingStrategy.deal`: over many samples, every revealed card lands with
  its owner and capacities hold.
- `SamplingStrategy.outcome`: matches `RoundScoring.score` on made, failed and
  tied Classic contracts; a Classic contract made only thanks to Belote
  scores as made; announcements move a borderline contract.
- `GameBoard.playViewFor`: `revealedAnnouncements` is empty during the first
  trick and filled after it; `AuctionView.announcements` follows the table.
- `BotArena`: a short seeded run with announcements on records announcement
  points in the round results.

## Arena results

`current` against `no-announcement-bidding`, 5,000 duplicate deal pairs per
run, announcements on. Point difference per deal, with its 95% margin.

Contrée, `ANNOUNCEMENT_WEIGHT`:

| Weight | Seed 1 | Seed 2 | Contracts made (seed 1) |
| --- | --- | --- | --- |
| 0.25 | +6.47 ± 1.11 | | 88.0% |
| 0.5 | +8.48 ± 1.38 | +8.38 ± 1.39 | 87.2% |
| **0.75** | **+9.95 ± 1.62** | **+9.96 ± 1.66** | 85.7% |
| 1.0 | +7.44 ± 1.76 | +7.66 ± 1.77 | 83.9% |

The weight is **0.75**: `current` wins 59.2% of matches to 1,000 points
(95% CI 56.4%–62.0%). The figures above counted the announcements in the
coinche test too. That term was then removed: it didn't change the point
difference (+9.96 ± 1.58 without it, seed 1, against +9.95 ± 1.62), but bots
coinched more often and won fewer coinches (37.5% against 51.1%), and the
extra coinches made several tests flaky.

Classic, `ANNOUNCEMENT_STEP`:

| Step | Seed 1 | Seed 2 |
| --- | --- | --- |
| 20 | +0.03 ± 0.53 | |
| 30 | +0.03 ± 0.16 | |
| 40 | +0.00 ± 0.15 | |
| **50** | **+0.00 ± 0.15** | **+0.13 ± 0.15** |
| 70 | +0.10 ± 0.11 | +0.08 ± 0.11 |

No step makes a difference beyond noise. With five cards and the upturned
card, a Classic hand rarely holds an announcement worth a hand point, so the
step stays at its starting value of **50**.

## Out of scope

- Bots keeping announcements quiet, or reading the partner's announcement into
  their play beyond the revealed cards.
- Bidding on the chance of the partner's announcements.
- Capot in the bots' scoring.
