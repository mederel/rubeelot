# Play all trumps and no trumps in Contrée (US-063) — design

## Goal

In Contrée, a player can bid tout-atout (every suit is trump) or sans-atout (no
suit is trump) besides the four suits, as in the variant of the official FFB
rules (`REGLES-DE-LA-BELOTE-COINCHEE.pdf`, §11). The rule engine plays and
scores both contracts; the bots play them correctly but don't bid them yet
(US-064).

## Decisions

- **The rule source is the FFB rules, §11.** Their scales keep a deal at 162
  points, so the 80–160 bids, the 162 of a failed contract and the capot's 250
  are unchanged.
- **No ranking at the same value.** The FFB only asks a bid to be higher than
  the previous one (§4.1 b), so 100 sans-atout doesn't overcall 100 ♥. This
  replaces the backlog criterion "each ranks above the suits at the same
  level".
- **A table option, on by default** (FFB: a variant the organiser turns on).
  It is offered in Contrée only, in the solo and pass-and-play setup and in a
  private table's lobby; public matchmaking always plays with it.
- **A `Trump` enum replaces the nullable `GameCard.Suit trump`** (approach
  1): `CLUBS, DIAMONDS, HEARTS, SPADES, ALL_TRUMPS, NO_TRUMPS`. Every rule asks
  the `Trump`, so the compiler finds every caller; the two new names fit the
  existing `VARCHAR(16)` trump columns.
- **Bots never bid the new contracts** in this story; they play them when a
  human takes one.

## Rules

| | Order, strongest first | Points | Total |
|---|---|---|---|
| Suit contract | unchanged | unchanged | 162 |
| Sans-atout | A, 10, K, Q, J, 9, 8, 7 | A 19, 10 10, K 4, Q 3, J 2, 9/8/7 0 | 4 × 38 + 10 = 162 |
| Tout-atout | J, 9, A, 10, K, Q, 8, 7 | J 13, 9 9, A 6, 10 5, K 3, Q 2, 8/7 0 | 4 × 38 + 10 = 162 |

- **Playing a card.** A player must follow the suit led.
  - Sans-atout: no obligation to go higher; a player who can't follow
    discards any card.
  - Tout-atout: a player who follows must play a card higher than the one
    winning the trick when they can. A player who can't follow discards any
    card.
  - In both, there is no cutting: a card of another suit never wins, so the
    trick goes to the highest card of the suit led.
- **Belote.** Sans-atout has none. In tout-atout, each player's K and Q of the
  same suit are a Belote worth 20, declared on the first card and scored on
  the second, as today; there can be up to four (80 points). Belote always
  stays with its team.
- **Announcements.** Sequences are unchanged. Carrés in sans-atout: aces 200,
  tens 150, kings, queens, jacks and nines 100 (eights and sevens don't count).
  Carrés in tout-atout keep their usual values. The "trump sequence" tie-break
  applies to a suit contract only; a full tie still goes to the first
  declarer (US-060).
- **Scoring** is unchanged (`RoundScoring`): the taker's total must reach the
  bid, a failed contract gives the defence 162 and the announcements, a capot
  is 250, a coinche doubles.

## Engine (`fr.beelot.game`)

- `Trump` (new, top-level): `of(GameCard.Suit)`; `suit()`, null for
  `ALL_TRUMPS` and `NO_TRUMPS`; `isTrump(GameCard.Suit)`, true for every suit
  under `ALL_TRUMPS` and none under `NO_TRUMPS`; `suitContract()`;
  `displayName()` ("Clubs" … "All trumps", "No trumps").
- `BeloteRules.strength/points/beats/legalCards/winningIndex` take a `Trump`.
  `legalCards` under `ALL_TRUMPS` keeps the overtaking rule within the suit
  led and lets a player who can't follow play any card; under `NO_TRUMPS`
  it returns the cards of the suit led, or the whole hand.
- `GameCard.displayOrder(Trump)`: trump order for every suit under
  `ALL_TRUMPS`, plain order for every suit under `NO_TRUMPS`, the suits in
  their natural order; a suit contract is unchanged.
- `Announcements.find(hand, Trump)` uses the sans-atout carré values under
  `NO_TRUMPS`; `strength(Trump)` and `bestSeat(…, Trump)` apply the trump
  tie-break only to a suit contract.
- `GameBoard` holds a `Trump`. The single `belotePlayerId` becomes the list
  of Belotes (player and suit): one per K+Q pair held in a trump suit. Each is
  declared and scored on its own; the messages stay "{0} declares Belote." and
  "{0} declares Rebelote: 20 bonus points.". `RoundSummary.trump`,
  `PlayView.trump` and `GameBoardView.trump` follow.
- `BiddingState`:
  - `allTrumps(boolean)` / `allTrumps()`, off unless set, like
    `announcements`;
  - `bid(playerId, value, Trump)`: `ALL_TRUMPS` or `NO_TRUMPS` while the
    option is off is refused with "Tout-atout and sans-atout are not played
    at this table." (`PrivateTableConflictException`, 409); the value rules
    are unchanged;
  - `chooseTrump(playerId, GameCard.Suit)` in Classic is unchanged and builds
    the board with `Trump.of(suit)`;
  - `highestBidSuit` becomes `highestBidTrump` (`Trump`) in the views;
    `AuctionView` gains `allTrumps`; `SeatCall.suit` becomes `trump`.
- `BotGame` gains `boolean allTrumps`. `PrivateTable` gains `allTrumps` (true
  by default) and `setAllTrumps(playerId, enabled)`, with the guards of
  `setAnnouncements`:
  - public table: "Public tables always play with tout-atout and
    sans-atout.";
  - not the owner: "Only the table owner can change tout-atout and
    sans-atout.";
  - game started: "Tout-atout and sans-atout cannot change after the game
    starts.".
  Each deal's `BiddingState` gets the option.

## Services and API

- `BotGameService.create` and `createPassAndPlay` take `allTrumps`; `rematch`
  keeps it. `PrivateTableService.setAllTrumps(tableId, token, enabled)`.
- `POST /api/bot-games` takes an optional `allTrumps` (`Boolean`, true when
  absent).
- `POST /api/private-tables/{id}/all-trumps` takes `{playerToken, enabled}`
  and returns the table.
- The Contrée bid requests' `suit` becomes a `Trump`: it reads `"HEARTS"` as
  today, and `"ALL_TRUMPS"` or `"NO_TRUMPS"`. The Classic trump request keeps
  a `GameCard.Suit`.
- The game and private table responses carry `allTrumps`. The board's
  `trump` and the bidding's `highestBidSuit` may hold `ALL_TRUMPS` or
  `NO_TRUMPS` (the field names stay, so the client changes little).

## History and export

- `V11__all_trumps.sql`:
  `ALTER TABLE match_record ADD COLUMN all_trumps BOOLEAN NOT NULL DEFAULT FALSE;`
  Matches recorded before read as played without the option.
- `FinishedMatch` and `MatchRecord` carry `allTrumps`; both recorders pass it.
  Round records already store `trump.name()`.
- The data export adds `allTrumps` to each match and stays `beelot-export-2`
  (fields are only added). Its rounds' `trump` may be `ALL_TRUMPS` or
  `NO_TRUMPS`.

## Bots

- `RuleBasedStrategy`, `CardMemory`, `SamplingStrategy`, `DoubleDummySolver`
  and `SolverBenchmark` switch to `Trump`: legal moves, trick winners and
  points come from `BeloteRules`, and "is this card a trump" asks
  `Trump.isTrump`. No new tactics.
- Bots still bid suits only (US-064), so the arena's results don't move.

## Client (`app.js`, `i18n.js`, `i18n-nl.js`)

- A "Tout-atout and sans-atout" on/off button group (US-028), for Contrée
  only, next to the announcements option: in the solo and pass-and-play setup
  (sent with the create request) and in the private-table lobby (the owner
  changes it through the new endpoint; others see it read-only; a public
  table doesn't show it).
- The Contrée auction's suit choice gains "All trumps" and "No trumps"
  buttons, hidden when the option is off; the bid sends the choice as `suit`.
- `suitName` names the new contracts: the contract line reads "100 All
  trumps", and the trump line, "{0} are trump" for a suit, reads "All trumps"
  or "No trumps". `orderHandForDisplay` follows `GameCard.displayOrder`.
- The rules reference (US-011) gains a "Tout-atout and sans-atout" section
  with the table above, the playing rules, Belote and the sans-atout carrés;
  the tutorial glossary gains both terms.
- French: "Tout atout", "Sans atout". Dutch: "Alles troef", "Geen troef".

## Testing

- `BeloteRulesTest`: for each new contract, legal cards (follow, overtake in
  tout-atout only, discard when void), trick winner (another suit never
  wins), and a full deck totalling 152 card points.
- `GameBoardTest`: a tout-atout deal with two Belotes scores 40 to their
  teams; sans-atout has none; a full deal totals 162.
- `AnnouncementsTest`: sans-atout carrés, and no trump tie-break under either
  contract.
- `BiddingStateTest`: tout-atout and sans-atout bids with the option on, the
  409 with it off, no overcall at the same value, Classic unchanged.
- `RuleBasedStrategy` and `SamplingStrategy` tests: bots finish a deal of each
  contract as the defence and as the human's partner; the solver scores a
  deal at 162.
- Service and controller tests for the option, a tout-atout bid,
  the private table guards; a history test for V11 and the export.
- The UI tests that follow the US-060 and US-062 patterns, and a headless
  browser check of one tout-atout and one sans-atout deal.

## Backlog

US-063's criteria are corrected (no ranking at the same value, the option),
its functional task is ticked with the FFB scales, the roadmap decision on the
scales is struck through, and the story is marked delivered.

## Out of scope

- Bots bidding tout-atout and sans-atout (US-064).
- Differences between today's Contrée scoring and the FFB rules, noted for a
  later story: the FFB also asks the taker to beat the defence, scores a made
  contract as total plus bid and a failed one as 160 plus bid, and has the
  capot bid, the surcoinche and rounding to the nearest ten.
