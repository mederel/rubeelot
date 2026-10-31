# Phase 1: Game Engine and Arena in Rust — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Beelot's rules engine, bots, double-dummy solver and bot arena in Rust, checking the bots' decisions against recorded deals.

**Architecture:** A Cargo workspace in `rubeelot` gets `beelot-game` (a synchronous library with no I/O) and `beelot-arena` (a library plus a CLI). Each engine and bot layer comes with its unit tests and a replay test against the recorded deals in `fixtures/`.

**Tech Stack:** Rust stable (edition 2024), `thiserror`, `serde` + `serde_json`, `clap` (derive), `criterion`.

**Spec:** `docs/superpowers/specs/2026-10-23-rust-backend-design.md` (phase 1 sections).

**Repository:** `rubeelot`, at `https://github.com/mederel/rubeelot`.

Port each method's logic in the same order of operations, keeping the same iteration orders and tie-breaks (first maximum wins, stable sorts), because the recorded deals depend on them.

## Global Constraints

- Seats are `pub type Seat = usize;`, numbered 0–3 in play order. North–South hold the even seats.
- No UUIDs, player names or English messages in `beelot-game`. Errors are `GameError` variants, each with a stable kebab-case `code()`.
- `beelot-game` dependencies: `thiserror` and `serde` only. Dev-dependencies: `serde_json`, `criterion`.
- Every `i64`/`i32` arithmetic that may overflow uses `wrapping_*`. Tests run with `overflow-checks = true`.
- Serde formats, used by the fixtures:
  - Field names are camelCase (`#[serde(rename_all = "camelCase")]`).
  - Enum values are upper-case names (`CLUBS`, `CLASSIC`, `PASS`, `TIERCE`), via `rename_all = "SCREAMING_SNAKE_CASE"`.
  - A `Team` is the string `"North–South"` or `"East–West"`, with an en dash (U+2013).
  - A `Card` is a string: the rank label (`7 8 9 10 J Q K A`) followed by the suit initial (`C D H S`), for example `"10H"` or `"JS"`.
- Unknown JSON fields are ignored when deserializing (the serde default), because the recorded views carry fields such as `player` that the Rust types drop.
- CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`. All three pass at the end of every task.
- Commit messages follow the style `type(scope): summary` (for example `feat(game): …`, `test(arena): …`), and end with:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`

## Review Focus

1. **The Challenging bot's time budget.** With the default 200 ms budget, its decisions depend on machine speed. Fixtures and parity tests must build it with `budget: None`; only the arena's `challenging` bot uses 200 ms. Pinned in Task 8 by `sampling_without_budget_is_repeatable`.
2. **Number formatting.** Arena reports print through `report_format::fixed`, which uses Rust's `{:.N}`: it rounds the exact binary value half-to-even, so reports are stable from run to run. Pinned in Task 10 by `fixed_pads_and_signs_to_the_decimals`.
3. **Compensated summation.** Averages and variances use Kahan summation, because a naive `iter().sum()` can change the last bit of the variance and flip a rounded digit. Pinned in Task 10 by `compensated_sum_compensates`.
4. **Redeals and the last trick.** A redeal (everyone passes, twice in Classic) re-shuffles with the same RNG and moves the dealer on; the arena stops when `deals() == 2`. `continue_after_trick` after the 7th trick plays the last trick automatically, using `hand[0]` for each seat. Pinned in Task 5 by `classic_double_pass_redeals_and_moves_dealer` and in Task 4 by `last_trick_plays_itself`.
5. **Hand order.** Hands keep the order they were dealt in, cards are removed with `Vec::remove` (never `swap_remove`), and `Announcements::find` and the bots iterate in that order. Pinned in Task 4 by `play_view_keeps_dealt_order`, and across the board by the fixture replay.

---

### Task 1: Recorded deals

**Files:**
- Create: `fixtures/rng.json`, `fixtures/deals-classic-on.jsonl`, `fixtures/deals-classic-off.jsonl`, `fixtures/deals-contree-on.jsonl`, `fixtures/deals-contree-off.jsonl`, `fixtures/SOURCE`

**Interfaces:**
- Produces: the files above, which every later task reads from `fixtures/`.

**Formats:**

```jsonc
// rng.json: test vectors of the random generator. Seeds: 0, 1, -1, 20261023, i64::MIN.
{
  "splitMix": [{"seed": 1, "longs": [/* 8 × next_long() */],
                "draws": [[32, 5] /* …, then 24 × next_int(bound) on the same instance */]}]
}

// deals-*.jsonl: one line per deal
{"seed": -4962768465676381896, "variant": "CLASSIC", "announcements": true, "dealer": 0,
 "northSouth": "current", "eastWest": "random",
 "steps": [
   {"auction": {"seat": 1, "view": {/* AuctionView */}, "decision": {"type": "PASS"}}},
   {"play": {"seat": 1, "view": {/* PlayView */}, "card": "JS"}},
   {"continue": {}}
 ],
 "result": {/* RoundResult */}}          // null when the deal was redealt
```

- Decisions are written as `{"type": "PASS"}`, `{"type": "BID", "value": 90, "suit": "HEARTS"}`, `{"type": "COINCHE"}` or `{"type": "TRUMP", "suit": "SPADES"}`.
- Deals cover both variants, announcements on and off, and every arena bot. The Challenging bot is recorded without a time budget (Review Focus 1).
- `fixtures/SOURCE` says when the deals were recorded.

- [ ] **Step 1: Add the files** to `fixtures/`, about 100 deals in all.

- [ ] **Step 2: Commit**

```bash
git add fixtures
git commit -m "test: recorded deals"
```

---

### Task 2: Workspace, CI, cards and random generators

**Files (in `rubeelot`):**
- Create: `Cargo.toml` (the workspace), `rust-toolchain.toml` (`channel = "stable"`), `.github/workflows/build.yml`
- Create: `crates/beelot-game/Cargo.toml` and `crates/beelot-game/src/{lib.rs, card.rs, rng.rs, error.rs}`
- Create: `crates/beelot-game/tests/rng.rs`
- Create: `fixtures/` (copied from `beelot/build/fixtures`) and `fixtures/SOURCE`
- Modify: `AGENTS.md` (replace "Project status" with the build, test and format commands); `README.md` (add a "Develop" section with the same commands)

**Interfaces:**
- Produces (in `card.rs`):
  - `enum Suit { Clubs, Diamonds, Hearts, Spades }` (in this order), with `Suit::ALL`, `fn ordinal(self) -> usize`, `fn display_name(self) -> &'static str` and `fn symbol(self) -> &'static str`.
  - `enum Rank { Seven, Eight, Nine, Ten, Jack, Queen, King, Ace }`, with `Rank::ALL`, `fn label(self) -> &'static str` and `fn label_hash(self) -> i32`. `label_hash` returns a fixed hash of the label: 55, 56, 57, 1567, 74, 81, 75, 65.
  - `struct Card { pub rank: Rank, pub suit: Suit }` (`Copy`, `Eq`, `Hash`), with:
    - `Card::new(rank, suit)`
    - `Card::deck() -> Vec<Card>`: suits in order, then ranks 7 to A
    - `fn display_key(self, trump: Option<Suit>) -> (usize, usize)`: the order of `GameCard.displayOrder`
    - `FromStr` and `Display` in the card notation, and serde through them.
  - `enum Variant { Classic, Contree }` with `display_name` ("Classic Belote", "Contrée"); `enum Difficulty { Relaxed, Challenging }`.
  - `enum Team { NorthSouth, EastWest }`, with `Team::of(seat) -> Team` and `fn display_name(self) -> &'static str`.
  - `pub type Seat = usize`.
- Produces (in `rng.rs`):
  - `pub trait RandomSource { fn next_int(&mut self, bound: i32) -> i32; }`
  - `pub struct SplitMix64`, with `SplitMix64::new(seed: i64)` and `fn next_long(&mut self) -> i64`; implements `RandomSource`.
  - `pub fn shuffle<T>(items: &mut [T], rng: &mut dyn RandomSource)`
- Produces (in `error.rs`): `#[derive(thiserror::Error, Debug, PartialEq, Eq)] pub enum GameError`, with `fn code(&self) -> &'static str`. Its variants are added by Tasks 4 and 5.

- [ ] **Step 1: Check the fixtures** from Task 1 are in `fixtures/`.

- [ ] **Step 2: Write the failing tests** in `tests/rng.rs`, which load `fixtures/rng.json` through `env!("CARGO_MANIFEST_DIR")/../../fixtures`:
  - `split_mix_matches_its_reference_draws`: `next_long` returns the recorded `longs`, then `next_int` the recorded `draws`, on the same instance.
  - `cards_round_trip`: `"10H".parse::<Card>()` gives `Card::new(Rank::Ten, Suit::Hearts)`, its `to_string()` is `"10H"`, and `"1H"` fails to parse.

  Also test the cards' display order in `card.rs` as `#[cfg(test)]` tests.

- [ ] **Step 3: Run them to make sure they fail**

Run: `cargo test -p beelot-game`
Expected: compilation errors (the types don't exist yet).

- [ ] **Step 4: Implement the workspace, `card.rs`, `rng.rs` and `error.rs`.**
  - Workspace `Cargo.toml`: `members = ["crates/*"]`, `resolver = "3"`, and `[profile.test] opt-level = 3` with `overflow-checks = true`, so the solver is fast enough under `cargo test`.
  - CI: `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`, `Swatinem/rust-cache@v2`, then the three commands from the Global Constraints.
  - The random generator is a SplitMix64, with all arithmetic wrapping:

```text
SplitMix64: seed = s; GAMMA = 0x9E3779B97F4A7C15
  next_seed(): seed += GAMMA
  next_long(): z = next_seed(); z = (z ^ (z >>> 30)) * 0xBF58476D1CE4E5B9; z = (z ^ (z >>> 27)) * 0x94D049BB133111EB; z ^ (z >>> 31)
  raw_int(): z = next_seed(); z = (z ^ (z >>> 33)) * 0x62A9D9ED799705F5; (((z ^ (z >>> 28)) * 0xCB24D0A5C88C35B3) >>> 32) as i32
  next_int(bound): r = raw_int(); m = bound - 1
    if bound & m == 0: return r & m
    u = r >>> 1; loop { r = u % bound; if u + m - r >= 0 (i32 wrapping) break; u = raw_int() >>> 1 }; return r
shuffle: for i in (2..=len).rev() { items.swap(i - 1, rng.next_int(i as i32) as usize) }
```

`>>>` means an unsigned shift: cast to `u64`/`u32`, shift, then cast back.

- [ ] **Step 5: Run the tests to make sure they pass**

Run: `cargo test -p beelot-game && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .github crates fixtures AGENTS.md README.md
git commit -m "feat(game): cards and the random generator"
```

---

### Task 3: Rules, announcements, round scoring and match score

**Files:**
- Create: `crates/beelot-game/src/{rules.rs, announcements.rs, scoring.rs, match_score.rs}`
- Build: `BeloteRules`, `Announcement`, `Announcements`, `RoundScoring`, `MatchScore`
- Port tests: `AnnouncementsTest`, `RoundScoringTest`, `MatchScoreTest`, as `#[cfg(test)]` modules in the same files

**Interfaces:**
- Produces (in `rules.rs`):
  - `fn strength(card: Card, trump: Suit) -> i32`
  - `fn points(card: Card, trump: Suit) -> i32`
  - `fn beats(contender: Card, winner: Card, lead: Suit, trump: Suit) -> bool`
  - `fn legal_cards(hand: &[Card], trick: &[Card], trump: Suit) -> Vec<Card>` (in hand order)
  - `fn winning_index(trick: &[Card], trump: Suit) -> usize`
- Produces (in `announcements.rs`):
  - `enum AnnouncementKind { Tierce, Cinquante, Cent, Carre }`
  - `struct Announcement { kind, cards: Vec<Card>, value: i32, suit: Option<Suit> }`, with `fn top_card(&self) -> Card`
  - `struct Declared { seat: Seat, announcements: Vec<Announcement> }`
  - `fn find(hand: &[Card]) -> Vec<Announcement>`
  - `fn compare_strength(a: &Announcement, b: &Announcement, trump: Option<Suit>) -> Ordering`
  - `fn best_seat(in_declaration_order: &[Declared], trump: Suit) -> Option<Seat>`
  - `fn rank_height(rank: Rank) -> i32`
- Produces (in `scoring.rs`):
  - `struct Declaration { seat: Seat, announcements: Vec<Announcement>, scored: bool }`. The team is `Team::of(seat)`.
  - `struct RoundResult` with every `GameBoard.RoundResult` field in snake_case (`i32` points, `contract_made: bool`, `capot_team: Option<Team>`, `announcements: Vec<Declaration>`).
  - `struct Totals` mirroring `RoundScoring.Totals`.
  - `fn score(totals: &Totals, declarations: &[Declaration]) -> RoundResult`
  - `struct RoundSummary { declaring_team: Team, trump: Suit, contract_value: Option<i32>, coinched: bool, result: RoundResult }`
- Produces (in `match_score.rs`):
  - `struct MatchScore`, with:
    - `MatchScore::new()`
    - `fn record_summary(&mut self, round: RoundSummary) -> bool`
    - `fn record(&mut self, result: &RoundResult)`
    - `north_south()`, `east_west()`, `complete()`, `fn winner(&self) -> Option<Team>` and `fn rounds(&self) -> &[RoundSummary]`
  - `WINNING_SCORE = 1000`. The match id lives on the server.
- All of these types derive serde in the formats from the Global Constraints.

- [ ] **Step 1: Write the failing tests.** Write the tests for the rules, announcements, scoring and match score, with a snake_case name per behaviour. Add `rules_legal_cards_follow_overtrump` to test `legal_cards` directly.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game`; expect compilation errors.
- [ ] **Step 3: Implement the four modules**, in this order. `Announcements.find` sorts with `strength(null).reversed()`; use a stable sort with the same comparator.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(game): rules, announcements and scoring`.

---

### Task 4: Card play

**Files:**
- Create: `crates/beelot-game/src/board.rs`; modify `error.rs`
- Build: `GameBoard` (without `GameBoardView`, `GameBoardSeat`, `declarationMessage` or player names)
- Port tests: `GameBoardRulesTest`, `GameBoardBeloteTest`, `GameBoardCapotTest`, `GameBoardLastTrickTest` and `GameBoardAnnouncementsTest`, as `crates/beelot-game/tests/board.rs`

**Interfaces:**
- Consumes: Task 3 (`rules`, `announcements`, `scoring`).
- Produces:
  - `struct SeatCard { seat: Seat, card: Card }`
  - `struct PlayView`, with these fields:
    - `seat`, `hand: Vec<Card>` (in dealt order), `legal_cards: Vec<Card>`, `trump: Suit`, `variant`
    - `contract_value: i32`, `coinched: bool`, `declaring_seat: Seat`
    - `tricks: Vec<Vec<SeatCard>>`, `current_trick: Vec<SeatCard>`
    - `announcements: bool`, `revealed_announcements: Vec<Declaration>`

    Its method is `fn declaring(&self) -> bool`.
  - `struct GameBoard` with:
    - `GameBoard::from_bidding(hands: [Vec<Card>; 4], trump: Suit, declarer: Seat, dealer: Seat, announcements: bool) -> Self`
    - `GameBoard::from_contract(hands: [Vec<Card>; 4], trump: Suit, declarer: Seat, contract_value: i32, coinched: bool, dealer: Seat, announcements: bool) -> Self`

      Both `assert!` their invariants.
    - `fn play(&mut self, seat: Seat, card: Card, declare: bool) -> Result<(), GameError>`
    - `fn continue_after_trick(&mut self) -> Result<(), GameError>`
    - `fn play_view(&self, seat: Seat) -> PlayView` and `fn legal_cards(&self, seat: Seat) -> Vec<Card>`
    - accessors `active_seat()`, `reviewing_completed_trick()`, `round_result() -> Option<&RoundResult>`, `round_summary() -> Option<RoundSummary>`, `trump()`, `variant()`, `contract_value()`, `coinched()`, `declarer()`, `dealer()`, `hand(seat) -> &[Card]`, `visible_trick() -> &[SeatCard]`, `completed_tricks()`, `next_leader()`, `scores() -> (i32, i32)`, `belote_seat() -> Option<Seat>`, `belote_cards_played() -> u32`, `belote_bonus_awarded()`, `declared() -> &[Declared]` and `revealed() -> &[Declaration]`
  - `GameError` variants, with codes in brackets:
    - `NotYourTurn` (`not-your-turn`)
    - `IllegalCard` (`illegal-card`)
    - `RoundEnded` (`round-ended`)
    - `NoCompletedTrick` (`no-completed-trick`)

- [ ] **Step 1: Write the failing tests.** Write the board tests with seats 0–3, asserting rejected moves with `assert_eq!(…, Err(GameError::…))`. Add:
  - `last_trick_plays_itself`: after 7 tricks, `continue_after_trick()` leaves `round_result().is_some()` and every hand empty (Review Focus 4).
  - `play_view_keeps_dealt_order`: after a seat plays its 3rd card, `play_view(seat).hand` equals the dealt hand without that card, in dealt order (Review Focus 5).
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game --test board`; expect failures.
- [ ] **Step 3: Implement `board.rs`.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game --test board`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(game): card play, belote, capot and announcements`.

---

### Task 5: Bidding

**Files:**
- Create: `crates/beelot-game/src/bidding.rs`; modify `error.rs`
- Build: `BiddingState` (without `BiddingView`, `PlayerCall`, `AuctionCall`, `message` or `auctionId`; those are player-facing and move to phase 2)
- Port tests: `BiddingStateTest` and `ContreeBiddingStateTest`, as `crates/beelot-game/tests/bidding.rs`

**Interfaces:**
- Consumes: Task 2 (`RandomSource`, `shuffle`, `Card::deck`) and Task 4 (`GameBoard::from_bidding`, `GameBoard::from_contract`).
- Produces:
  - `struct SeatCall { seat: Seat, r#type: CallType, value: i32, suit: Option<Suit> }` (`type` in JSON), and `enum CallType { Pass, Bid, Trump, Coinche }`.
  - `struct AuctionView`, with these fields:
    - `seat`, `hand: Vec<Card>`, `upturned_card: Option<Card>`, `round: i32`, `variant`, `dealer_index: Seat`
    - `auction: Vec<SeatCall>`, `highest_bid: i32`, `highest_bid_suit: Option<Suit>`
    - `highest_bidder_seat: Option<Seat>`: serialized as `-1` for `None`, with a `serde(with = "minus_one_none")` helper in `bidding.rs`
    - `coinche_allowed: bool`, `announcements: bool`

    Its methods are `partner_holds_contract()` and `has_bid()`.
  - `enum AuctionDecision { Pass, Bid { value: i32, suit: Suit }, Coinche, Trump { suit: Suit } }`, with `#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]`.
  - `struct BiddingState` with:
    - `BiddingState::new(variant: Variant, dealer: Seat, rng: Box<dyn RandomSource + Send>) -> Self`
    - `fn with_announcements(self, on: bool) -> Self` and `fn announcements(&self) -> bool`
    - `pass`, `choose_trump(seat, trump)`, `bid(seat, value: i32, suit)` and `coinche`, each `-> Result<(), GameError>`
    - `fn apply(&mut self, seat: Seat, decision: AuctionDecision) -> Result<(), GameError>`
    - `fn auction_view(&self, seat: Seat) -> AuctionView`
    - accessors `active_seat()`, `dealer()`, `deals() -> u32`, `round()`, `hand(seat) -> &[Card]`, `upturned_card()`, `highest_bid()`, `highest_bid_suit()`, `highest_bidder()`, `auction() -> &[SeatCall]` and `can_coinche(seat)`
    - `fn completed_board(&self) -> Option<&GameBoard>` and `fn into_board(self) -> Option<GameBoard>`
  - `GameError` variants for each `PrivateTableConflictException` thrown in `BiddingState` (`NotYourTurn` comes from Task 4). Each variant's code is in brackets:
    - `ContractOnlyInContree` (`contract-only-in-contree`)
    - `TrumpChoiceOnlyInClassic` (`trump-choice-only-in-classic`)
    - `AuctionEnded` (`auction-ended`)
    - `MustTakeUpturnedSuit` (`must-take-upturned-suit`)
    - `MustChooseAnotherSuit` (`must-choose-another-suit`)
    - `InvalidBid` (`invalid-bid`)
    - `BidTooLow` (`bid-too-low`)
    - `CoincheNotAllowed` (`coinche-not-allowed`)

- [ ] **Step 1: Write the failing tests.** Write the Classic and Contrée auction tests as in Task 4. Add:
  - `classic_double_pass_redeals_and_moves_dealer`: in Classic with dealer 3, eight passes give `deals() == 2`, `dealer() == 0` and `active_seat() == 1` (Review Focus 4).
  - `same_seed_same_hands`: two `BiddingState::new(Classic, 0, Box::new(SplitMix64::new(7)))` give equal `hand(s)` for every seat.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game --test bidding`; expect compilation errors.
- [ ] **Step 3: Implement `bidding.rs`**, dealing as follows: `shuffle` of `Card::deck()`, then hands taken from the front in seat order 0–3, then the upturned card in Classic.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(game): bidding for Classic and Contrée`.

---

### Task 6: Replay the fixtures through the engine

**Files:**
- Create: `crates/beelot-game/tests/deals.rs`
- Create: `crates/beelot-game/tests/common/mod.rs` (fixture loading and deal replay, shared with Tasks 7–9)

**Interfaces:**
- Consumes: Tasks 2–5.
- Produces (in `tests/common/mod.rs`):
  - `fn load_deals() -> Vec<(String, Deal)>`: the file name and the parsed line, for every `deals-*.jsonl`.
  - `struct Deal { seed: i64, variant: Variant, announcements: bool, dealer: Seat, north_south: String, east_west: String, steps: Vec<Step>, result: Option<RoundResult> }`
  - `enum Step { Auction { seat, view: AuctionView, decision: AuctionDecision }, Play { seat, view: PlayView, card: Card }, Continue {} }`, externally tagged with the lowercase names used in the JSON.
  - `fn replay(deal: &Deal, check: &mut dyn FnMut(&Step, &mut SplitMix64))`. For each step, it asserts that the engine's view equals the recorded view, then calls `check`, then applies the recorded decision. `check` receives the deal's single `SplitMix64::new(seed)` so bot tests can draw from it as the arena does. At the end it asserts that the result equals the recorded result (`None` ⇔ redeal, that is `deals() == 2`).

- [ ] **Step 1: Write the test** `engine_matches_every_recorded_deal`: for every deal, `replay(deal, &mut |_, _| {})`. A failure message names the file, the line, the step index and both values.
- [ ] **Step 2: Run it.** Run `cargo test -p beelot-game --test deals`. Expected: PASS. Any failure is a porting bug in Tasks 2–5: fix it in that module, add a unit test there that reproduces it, and note it in the commit message.
- [ ] **Step 3: Commit** with the message `test(game): replay the recorded deals through the engine`.

---

### Task 7: Rule-based bot

**Files:**
- Create: `crates/beelot-game/src/bot/{mod.rs, memory.rs, rule_based.rs, turns.rs}`
- Build: `BotStrategy`, `CardMemory`, `RuleBasedStrategy` and `BotTurns`
- Port tests: `CardMemoryTest`, `RuleBasedStrategyBiddingTest` and `RuleBasedStrategyPlayTest` (with `RecordingStrategy` as a test helper)

**Interfaces:**
- Produces:
  - `pub trait BotStrategy: Send + Sync { fn decide_auction(&self, view: &AuctionView) -> AuctionDecision; fn choose_card(&self, view: &PlayView) -> Card; }`
  - `struct RuleBasedStrategy`, with `RuleBasedStrategy::new()`, `RuleBasedStrategy::without_trump_control()` and `pub fn support_raise(hand: &[Card], suit: Suit) -> i32`. Its other helpers are `pub(crate)`.
  - `CardMemory` (`pub(crate)`). Its `known_cards(seat)` returns `Vec<Card>` in suit-then-rank order.
  - `turns::take_auction_turn(bidding: &mut BiddingState, strategy: &dyn BotStrategy) -> Result<(), GameError>`
  - `turns::play_turn(board: &mut GameBoard, strategy: &dyn BotStrategy) -> Result<(), GameError>` (always with `declare = true`)
- Consumes: Task 6's `replay`.

- [ ] **Step 1: Write the failing tests.**
  - Write the strategy's bidding, play and memory tests.
  - In `tests/deals.rs`, add `current_bot_matches_the_recorded_deals`. It replays every deal and, for steps whose seat is played by `"current"`, asserts that `RuleBasedStrategy::new()` gives the recorded decision. The seat's bot is `north_south` for even seats and `east_west` for odd ones.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game`; expect compilation errors.
- [ ] **Step 3: Implement the bot modules.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(bot): rule-based bot`.

---

### Task 8: Double-dummy solver and Challenging bot

**Files:**
- Create: `crates/beelot-game/src/bot/{solver.rs, sampling.rs, strategies.rs}`
- Create: `crates/beelot-game/benches/solver.rs`
- Build: `DoubleDummySolver`, `SamplingStrategy`, `BotStrategies` and a solver benchmark (criterion)
- Port tests: `DoubleDummySolverTest`, `SamplingStrategyTest` and `BotStrategiesTest`

**Interfaces:**
- Produces:
  - `struct Position { hands: [Vec<Card>; 4], trump: Suit, tricks: Vec<Vec<SeatCard>>, current_trick: Vec<SeatCard>, first_leader: Seat }`, with `Position::start(hands, trump, leader)`.
  - `struct Solution { north_south_points: i32, east_west_points: i32, best_card: Option<Card> }`
  - `struct DoubleDummySolver`, with `DoubleDummySolver::new()`, `fn solve(&mut self, position: &Position) -> Solution` and `fn nodes(&self) -> u64`. Its tables are `Vec`s of `1 << 20` entries on the heap, never stack arrays.
  - `struct SamplingStrategy`, with:
    - `SamplingStrategy::new(seed: i64)`: 50 samples, `Some(200 ms)`, first searched trick 3
    - `SamplingStrategy::with_settings(seed: i64, samples: usize, budget: Option<Duration>, first_searched_trick: usize)`

    `None` means no deadline. Solvers are pooled in a `Mutex<Vec<DoubleDummySolver>>`. The fingerprint uses `Rank::label_hash` and wrapping `i64` arithmetic for its fingerprint and mix.
  - `strategies::for_difficulty(d: Difficulty) -> &'static dyn BotStrategy` and `strategies::for_tables()`. The Challenging seed is drawn once from the OS, through `std::hash::RandomState` hashing, so there's no new dependency.

- [ ] **Step 1: Write the failing tests.**
  - Write the solver and sampling tests. Where a sampling test relies on the budget never cutting the search, use `budget: None`.
  - Add `sampling_without_budget_is_repeatable`: one late-round `PlayView` taken from the fixtures gives the same card from two separate `with_settings(1, 50, None, 3)` instances (Review Focus 1).
  - In `tests/deals.rs`, add `challenging_bot_matches_the_recorded_deals`, as Task 7 does, with `SamplingStrategy::with_settings(1, 50, None, 3)` for `"challenging"` seats.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game`; expect compilation errors.
- [ ] **Step 3: Implement the modules and the `solver` benchmark.** The benchmark generates deals with `SplitMix64::new(1)`, `shuffle`, 8 cards per seat, trump `Suit::ALL[next_int(4)]`, leader `(deal % 4 + 1) % 4`. It times `solve(Position::start(…))` over 20 deals.
- [ ] **Step 4: Run the tests and the benchmark.** Run `cargo test -p beelot-game && cargo bench -p beelot-game --bench solver -- --quick`. Expect PASS, and the benchmark printing its timings. The whole `cargo test --workspace` should finish in under 2 minutes; if it doesn't, mention it in the commit message rather than marking the fixtures `#[ignore]`.
- [ ] **Step 5: Commit** with the message `feat(bot): double-dummy solver and Challenging bot`.

---

### Task 9: Arena bots

**Files:**
- Create: `crates/beelot-game/src/bot/arena.rs`
- Build: `ArenaBot` and the bots `CurrentBot`, `ChallengingBot`, `BaselineBot`, `PreviousPlayBot`, `PreviousTrumpsBot`, `NoCoincheBot`, `NoAnnouncementBiddingBot` and `RandomCardBot`

**Interfaces:**
- Produces:
  - `pub trait ArenaBot: Send + Sync { fn name(&self) -> &'static str; fn decide_auction(&self, view: &AuctionView, rng: &mut dyn RandomSource) -> AuctionDecision; fn choose_card(&self, view: &PlayView, rng: &mut dyn RandomSource) -> Card; }`
  - `pub const ARENA_BOTS: [&str; 8] = ["current", "challenging", "baseline", "previous-play", "previous-trumps", "no-coinche", "no-announcement-bidding", "random"];`
  - `pub fn arena_bot(name: &str) -> Result<Box<dyn ArenaBot>, UnknownBot>`. `UnknownBot`'s `Display` is `Unknown bot <name>. Available: [current, challenging, …]`.
  - `pub fn arena_bot_unbounded(name: &str) -> Result<Box<dyn ArenaBot>, UnknownBot>`: the same, except `"challenging"` uses `budget: None`. It's used by the fixture replay.

- [ ] **Step 1: Write the failing test.** In `tests/deals.rs`, replace the per-bot tests from Tasks 7 and 8 with `every_bot_matches_the_recorded_deals`. For every step, it gets the seat's bot through `arena_bot_unbounded(name)` and asserts that its decision, drawn from the replay's shared `SplitMix64`, equals the recorded one. A failure names the bot.
- [ ] **Step 2: Run it to make sure it fails.** Run `cargo test -p beelot-game --test deals`; expect compilation errors.
- [ ] **Step 3: Implement `arena.rs`.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS. Every recorded decision of every bot now matches the recording.
- [ ] **Step 5: Commit** with the message `feat(bot): arena bots`.

---

### Task 10: Arena runner, report and CLI

**Files:**
- Create: `crates/beelot-arena/Cargo.toml` (lib + bin) and `crates/beelot-arena/src/{lib.rs, arena.rs, report.rs, report_format.rs, main.rs}`
- Create: `crates/beelot-arena/tests/reports.rs`
- Build: `BotArena`, `ArenaReport` and the CLI. Test: `BotArenaTest`.
- Modify: `README.md`, adding a "Bot arena" section with the CLI usage and the solver benchmark command.

**Interfaces:**
- Consumes: Task 9 (`arena_bot`), Task 5 (`BiddingState`), Task 4 (`GameBoard`), Task 3 (`MatchScore`) and Task 2 (`SplitMix64`).
- Produces:
  - `pub struct BotArena`, with `BotArena::new(first: Box<dyn ArenaBot>, second: Box<dyn ArenaBot>)` and `fn run(&self, variant: Variant, deal_pairs: usize, seed: i64, announcements: bool) -> ArenaReport`. It panics when `deal_pairs < 2`.
  - `pub struct ArenaReport` with its counters and averages, plus `fn format(&self) -> String`.
  - `report_format::fixed(x: f64, decimals: usize, plus: bool) -> String`: `{:.N}` or `{:+.N}` formatting.
  - `report_format::compensated_sum(values: impl IntoIterator<Item = f64>) -> f64`: Kahan summation. Return `sum + compensation`, or the simple sum when that is NaN and the simple sum is infinite.
- Port details:
  - The arena's RNG order is `SplitMix64::new(seed)`, then one `next_long()` per pair to get `deal_seed`. Each of the pair's two plays uses `SplitMix64::new(deal_seed)` for the bidding and its own `SplitMix64::new(deal_seed)` for the bots.
  - Averages and variance use `compensated_sum`. Keep the Wilson formula's operation order.
  - The report format string is copied verbatim, including "−", "±", "µs" and the en dashes.
- CLI (clap derive) options and their defaults:
  - `--first` current
  - `--second` random
  - `--deals` 10000
  - `--seed` 1
  - `--variant` classic|contree|both (default both)
  - `--announcements` on|off (default on)
  - `--report` target/bot-arena/report.txt

  It prints each section, writes the report file (creating its parent directory), and prints `Report written to <absolute path>`.

- [ ] **Step 1: Write the failing tests.**
  - `fixed_pads_and_signs_to_the_decimals`: `fixed(3.0, 2, true) == "+3.00"`, `fixed(-0.001, 2, true) == "-0.00"` and `fixed(2.5, 0, false) == "2"` (Review Focus 2).
  - `compensated_sum_compensates`: `compensated_sum(std::iter::repeat(0.1).take(10)) == 1.0` (Review Focus 3).
  - `reports_are_repeatable`: two runs of `BotArena` with 100 pairs and seed 1 give the same `format()`, apart from the line that starts with `Decision time per card:`.
  - Write the `BotArenaTest` cases.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-arena`; expect compilation errors.
- [ ] **Step 3: Implement the crate.**
- [ ] **Step 4: Run the tests and the CLI.** Run `cargo test --workspace && cargo run --release -p beelot-arena -- --deals 200 --variant contree`. Expect PASS, and a Contrée report ending with `Report written to …/target/bot-arena/report.txt`.
- [ ] **Step 5: Run every check.** Run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`; expect all clean.
- [ ] **Step 6: Commit** with the message `feat(arena): bot arena runner, report and CLI`.
