# Phase 1: Game Engine and Arena in Rust — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port Beelot's rules engine, bots, double-dummy solver and bot arena from Java to Rust, proving through recorded fixtures that the Rust code makes the same decisions as the Java code.

**Architecture:** First, a fixture recorder in the Java repository (`beelot`) plays seeded deals and writes every view, decision and result as JSON. Then a Cargo workspace in `rubeelot` gets `beelot-game` (a synchronous library with no I/O) and `beelot-arena` (a library plus a CLI). Each engine and bot layer is ported from its Java source, its Java unit tests are ported alongside it, and a replay test checks it against the fixtures.

**Tech Stack:** Rust stable (edition 2024), `thiserror`, `serde` + `serde_json`, `clap` (derive), `criterion`. On the Java side: Java 21, Gradle, Jackson 3 (`tools.jackson.databind`).

**Spec:** `docs/superpowers/specs/2026-10-23-rust-backend-design.md` (phase 1 sections).

**Repositories:**
- `beelot`: `~/programming/rust/beelot`, the Java backend at `https://github.com/mederel/beelot`. Only Task 1 changes it.
- `rubeelot`: `~/programming/rust/rubeelot`, the target at `https://github.com/mederel/rubeelot`. Every other task.

The Java source each task ports is `beelot/src/main/java/fr/beelot/game/<File>.java`, and its tests are in `beelot/src/test/java/fr/beelot/game/`. **The Java source is the behavioural spec.** Port each method's logic in the same order of operations, keeping the same iteration orders and tie-breaks (first maximum wins, stable sorts), because fixture parity depends on them.

## Global Constraints

- Seats are `pub type Seat = usize;`, numbered 0–3 in play order. North–South hold the even seats.
- No UUIDs, player names or English messages in `beelot-game`. Errors are `GameError` variants, each with a stable kebab-case `code()`.
- `beelot-game` dependencies: `thiserror` and `serde` only. Dev-dependencies: `serde_json`, `criterion`.
- Every `i64`/`i32` arithmetic that Java lets overflow uses `wrapping_*`. Tests run with `overflow-checks = true`.
- Serde formats, used by the fixtures:
  - Field names are camelCase (`#[serde(rename_all = "camelCase")]`).
  - Enum values are the Java names (`CLUBS`, `CLASSIC`, `PASS`, `TIERCE`), via `rename_all = "SCREAMING_SNAKE_CASE"`.
  - A `Team` is the string `"North–South"` or `"East–West"`, with an en dash (U+2013).
  - A `Card` is a string: the rank label (`7 8 9 10 J Q K A`) followed by the suit initial (`C D H S`), for example `"10H"` or `"JS"`.
- Unknown JSON fields are ignored when deserializing (the serde default), because the Java records carry fields such as `player` that the Rust types drop.
- CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`. All three pass at the end of every task.
- Commit messages follow `beelot`'s style, `type(scope): summary` (for example `feat(game): …`, `test(arena): …`), and end with:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`

## Review Focus

1. **The Challenging bot's time budget.** With the default 200 ms budget, its decisions depend on machine speed. Fixtures and parity tests must build it with `budget: None`; only the arena's `challenging` bot uses 200 ms. Pinned in Task 8 by `sampling_without_budget_is_repeatable`.
2. **Java's number formatting.** `String.format("%.2f")` rounds the *shortest* decimal form of a double half-up (`1.005` → `"1.01"`, `0.125` → `"0.13"`), whereas Rust's `{:.2}` rounds the exact binary value half-to-even. Arena reports have to match Java byte for byte. Pinned in Task 10 by `fixed_rounds_shortest_form_half_up`.
3. **Java's compensated summation.** `DoubleStream.sum()` and `average()` use Kahan summation, so a naive `iter().sum()` can change the last bit of the variance and flip a rounded digit. Pinned in Task 10 by `java_sum_compensates`.
4. **Redeals and the last trick.** A redeal (everyone passes, twice in Classic) re-shuffles with the same RNG and moves the dealer on; the arena stops when `deals() == 2`. `continue_after_trick` after the 7th trick plays the last trick automatically, using `hand[0]` for each seat. Pinned in Task 5 by `classic_double_pass_redeals_and_moves_dealer` and in Task 4 by `last_trick_plays_itself`.
5. **Hand order.** Hands keep the order they were dealt in, cards are removed with `Vec::remove` (never `swap_remove`), and `Announcements::find` and the bots iterate in that order. Pinned in Task 4 by `play_view_keeps_dealt_order`, and across the board by the fixture replay.

---

### Task 1: Fixture recorder (in `beelot`)

**Files (in `beelot`):**
- Create: `src/test/java/fr/beelot/game/arena/FixtureRecorder.java`
- Create: `src/test/java/fr/beelot/game/arena/FixtureRecorderTest.java`
- Modify: `build.gradle`, adding the `recordFixtures` task after `botArena`

**Interfaces:**
- Produces: a directory with the following files, which every later task reads from `rubeelot/fixtures/`:
  - `rng.json`
  - `deals-classic-on.jsonl`, `deals-classic-off.jsonl`, `deals-contree-on.jsonl`, `deals-contree-off.jsonl`
  - `arena/<first>-vs-<second>-<classic|contree>-<on|off>.txt`

**Formats:**

```jsonc
// rng.json. Seeds: 0, 1, -1, 20261023, Long.MIN_VALUE. Bounds cycle through [32, 31, 7, 65536, 1000, 1] for 24 draws.
{
  "javaRandom":  [{"seed": 1, "draws": [[32, 17], [31, 4] /* … */]}],     // new Random(seed).nextInt(bound)
  "javaShuffle": [{"seed": 1, "deck": ["9H", "7C" /* … 32 cards */]}],    // Collections.shuffle(fresh deck, new Random(seed))
  "splitMix":    [{"seed": 1, "longs": [/* 8 × nextLong() */],
                   "draws": [[32, 5] /* …, then 24 × nextInt(bound) on the same instance */]}]
}
// A fresh deck is suits CLUBS, DIAMONDS, HEARTS, SPADES × ranks 7 8 9 10 J Q K A, as in BiddingState.dealAgain.

// deals-*.jsonl: one line per deal
{"seed": -4962768465676381896, "variant": "CLASSIC", "announcements": true, "dealer": 0,
 "northSouth": "current", "eastWest": "random",
 "steps": [
   {"auction": {"seat": 1, "view": {/* AuctionView record */}, "decision": {"type": "PASS"}}},
   {"play": {"seat": 1, "view": {/* PlayView record */}, "card": "JS"}},
   {"continue": {}}
 ],
 "result": {/* RoundResult record */}}          // null when the deal was redealt
```

- Decisions are written as `{"type": "PASS"}`, `{"type": "BID", "value": 90, "suit": "HEARTS"}`, `{"type": "COINCHE"}` or `{"type": "TRUMP", "suit": "SPADES"}`.

- [ ] **Step 1: Write the failing test** `FixtureRecorderTest.recordsOneDealPerLine`. It calls `FixtureRecorder.record(tempDir, 2)` (2 deals per configuration), then asserts:
  - each `deals-*.jsonl` file has exactly 2 lines, and every line parses as JSON with a non-empty `steps` array;
  - `rng.json` has 5 `javaRandom` entries, each with 24 draws.

- [ ] **Step 2: Run it to make sure it fails**

Run: `./gradlew test --tests 'fr.beelot.game.arena.FixtureRecorderTest'`
Expected: compilation fails, because `FixtureRecorder` doesn't exist.

- [ ] **Step 3: Implement `FixtureRecorder`**, with `public static void record(Path out, int dealsPerConfiguration)` and `main(String[] args)` (`--out <dir>`, default 24 deals per configuration).
  - Deals mirror `BotArena.play`: `new BiddingState(players, variant, dealer, new Random(dealSeed)).announcements(on)`, plus one `new SplittableRandom(dealSeed)` shared by the bots for the whole deal. The loop condition is `completedBoard() == null && deals() == 1`, then `continueAfterTrick` whenever the board is reviewing a completed trick.
  - Configurations in this order: CLASSIC on, CLASSIC off, CONTREE on, CONTREE off. In each one, the deal seeds come from `new SplittableRandom(20261023L).nextLong()`.
  - Deal `i` (from 0) uses dealer `i % 4`, North–South `AVAILABLE[i % 8]` and East–West `AVAILABLE[(i + 1 + i / 8) % 8]`. `AVAILABLE` is `ArenaBot.AVAILABLE` in its declared order, except that `challenging` is replaced by an `ArenaBot` named `"challenging"` that wraps `new SamplingStrategy(1, 50, Duration.ofDays(1), 3)`. That's the unbounded budget from Review Focus 1.
  - JSON goes through Jackson 3's `JsonMapper`, with a `SimpleModule` serializer that writes a `GameCard` as `rank + suit.name().charAt(0)`. Records (`AuctionView`, `PlayView`, `RoundResult`, `Declaration`, `Announcement`, `SeatCall`) serialize with their component names.
  - Arena reports: `new BotArena(ArenaBot.named(first), ArenaBot.named(second)).run(variant, 100, 1, on).format()` for these 6 configurations:
    - `current` vs `random`, CLASSIC, on
    - `current` vs `random`, CONTREE, on
    - `current` vs `baseline`, CLASSIC, off
    - `current` vs `baseline`, CONTREE, off
    - `previous-play` vs `no-coinche`, CONTREE, on
    - `previous-trumps` vs `no-announcement-bidding`, CLASSIC, on

  In `build.gradle`, register `recordFixtures` as a `JavaExec` task with `classpath = sourceSets.test.runtimeClasspath`, `mainClass = 'fr.beelot.game.arena.FixtureRecorder'` and `args '--out', layout.buildDirectory.dir('fixtures').get().asFile.path`.

- [ ] **Step 4: Run the test to make sure it passes**

Run: `./gradlew test --tests 'fr.beelot.game.arena.FixtureRecorderTest'`
Expected: PASS.

- [ ] **Step 5: Check determinism across JVMs**

Run: `./gradlew recordFixtures && cp -r build/fixtures /tmp/fx1 && ./gradlew recordFixtures && diff -r -I '^Decision time' build/fixtures /tmp/fx1`
Expected: no output. (Only the decision-time line of the arena reports may differ.)

- [ ] **Step 6: Run the whole Java build**

Run: `./gradlew build`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 7: Commit (in `beelot`)**

```bash
git add src/test/java/fr/beelot/game/arena/FixtureRecorder*.java build.gradle
git commit -m "test(arena): record fixtures for the Rust port"
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
  - `enum Suit { Clubs, Diamonds, Hearts, Spades }` (Java ordinal order), with `Suit::ALL`, `fn ordinal(self) -> usize`, `fn display_name(self) -> &'static str` and `fn symbol(self) -> &'static str`.
  - `enum Rank { Seven, Eight, Nine, Ten, Jack, Queen, King, Ace }`, with `Rank::ALL`, `fn label(self) -> &'static str` and `fn java_hash(self) -> i32`. `java_hash` returns Java's `String.hashCode` of the label: 55, 56, 57, 1567, 74, 81, 75, 65.
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
  - `pub struct JavaRandom`, with `JavaRandom::new(seed: i64)`; implements `RandomSource`.
  - `pub struct SplitMix64`, with `SplitMix64::new(seed: i64)` and `fn next_long(&mut self) -> i64`; implements `RandomSource`.
  - `pub fn java_shuffle<T>(items: &mut [T], rng: &mut dyn RandomSource)`
- Produces (in `error.rs`): `#[derive(thiserror::Error, Debug, PartialEq, Eq)] pub enum GameError`, with `fn code(&self) -> &'static str`. Its variants are added by Tasks 4 and 5.

- [ ] **Step 1: Copy the fixtures.** Copy `beelot/build/fixtures/*` into `rubeelot/fixtures/` and write `fixtures/SOURCE` containing the `beelot` commit hash from Task 1 (`git -C ../beelot rev-parse HEAD`).

- [ ] **Step 2: Write the failing tests** in `tests/rng.rs`, which load `fixtures/rng.json` through `env!("CARGO_MANIFEST_DIR")/../../fixtures`:
  - `java_random_matches_java`: for every entry, `JavaRandom::new(seed)` returns the recorded `draws` in order.
  - `java_shuffle_matches_java`: `java_shuffle(&mut Card::deck(), &mut JavaRandom::new(seed))` equals the recorded `deck`.
  - `split_mix_matches_java`: `next_long` returns the recorded `longs`, then `next_int` the recorded `draws`, on the same instance.
  - `cards_round_trip`: `"10H".parse::<Card>()` gives `Card::new(Rank::Ten, Suit::Hearts)`, its `to_string()` is `"10H"`, and `"1H"` fails to parse.

  Also port `GameCardDisplayOrderTest` into `card.rs` as `#[cfg(test)]` tests.

- [ ] **Step 3: Run them to make sure they fail**

Run: `cargo test -p beelot-game`
Expected: compilation errors (the types don't exist yet).

- [ ] **Step 4: Implement the workspace, `card.rs`, `rng.rs` and `error.rs`.**
  - Workspace `Cargo.toml`: `members = ["crates/*"]`, `resolver = "3"`, and `[profile.test] opt-level = 3` with `overflow-checks = true`, so the solver is fast enough under `cargo test`.
  - CI: `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`, `Swatinem/rust-cache@v2`, then the three commands from the Global Constraints.
  - The random generators are Java's exact algorithms, with all arithmetic wrapping:

```text
JavaRandom: seed = (s ^ 0x5DEECE66D) & ((1<<48)-1)
  next(bits): seed = (seed * 0x5DEECE66D + 0xB) & ((1<<48)-1); return (seed >> (48 - bits)) as i32
  next_int(bound): r = next(31); m = bound - 1
    if bound & m == 0: return ((bound as i64 * r as i64) >> 31) as i32
    u = r; loop { r = u % bound; if u - r + m >= 0 (i32 wrapping) break; u = next(31) }; return r
SplitMix64: seed = s; GAMMA = 0x9E3779B97F4A7C15
  next_seed(): seed += GAMMA
  next_long(): z = next_seed(); z = (z ^ (z >>> 30)) * 0xBF58476D1CE4E5B9; z = (z ^ (z >>> 27)) * 0x94D049BB133111EB; z ^ (z >>> 31)
  raw_int(): z = next_seed(); z = (z ^ (z >>> 33)) * 0x62A9D9ED799705F5; (((z ^ (z >>> 28)) * 0xCB24D0A5C88C35B3) >>> 32) as i32
  next_int(bound): r = raw_int(); m = bound - 1
    if bound & m == 0: return r & m
    u = r >>> 1; loop { r = u % bound; if u + m - r >= 0 (i32 wrapping) break; u = raw_int() >>> 1 }; return r
java_shuffle: for i in (2..=len).rev() { items.swap(i - 1, rng.next_int(i as i32) as usize) }
```

`>>>` means an unsigned shift: cast to `u64`/`u32`, shift, then cast back.

- [ ] **Step 5: Run the tests to make sure they pass**

Run: `cargo test -p beelot-game && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .github crates fixtures AGENTS.md README.md
git commit -m "feat(game): cards and Java-exact random generators"
```

---

### Task 3: Rules, announcements, round scoring and match score

**Files:**
- Create: `crates/beelot-game/src/{rules.rs, announcements.rs, scoring.rs, match_score.rs}`
- Port from Java: `BeloteRules`, `Announcement`, `Announcements`, `RoundScoring`, `MatchScore`
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
  - `struct Declaration { seat: Seat, announcements: Vec<Announcement>, scored: bool }`. Java's `player` and `team` are dropped; the team is `Team::of(seat)`.
  - `struct RoundResult` with every `GameBoard.RoundResult` field in snake_case (`i32` points, `contract_made: bool`, `capot_team: Option<Team>`, `announcements: Vec<Declaration>`).
  - `struct Totals` mirroring `RoundScoring.Totals`.
  - `fn score(totals: &Totals, declarations: &[Declaration]) -> RoundResult`
  - `struct RoundSummary { declaring_team: Team, trump: Suit, contract_value: Option<i32>, coinched: bool, result: RoundResult }`
- Produces (in `match_score.rs`):
  - `struct MatchScore`, with:
    - `MatchScore::new()`
    - `fn record_summary(&mut self, round: RoundSummary) -> bool` (Java's `record(RoundSummary)`)
    - `fn record(&mut self, result: &RoundResult)`
    - `north_south()`, `east_west()`, `complete()`, `fn winner(&self) -> Option<Team>` and `fn rounds(&self) -> &[RoundSummary]`
  - `WINNING_SCORE = 1000`. Java's `UUID id` moves to the server.
- All of these types derive serde in the formats from the Global Constraints.

- [ ] **Step 1: Write the failing tests.** Port the three Java test classes method by method, keeping each `@Test` name in snake_case and the same hands, values and assertions. Add `rules_legal_cards_follow_overtrump`, built from one of `GameBoardRulesTest`'s hands, to test `legal_cards` directly.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game`; expect compilation errors.
- [ ] **Step 3: Implement the four modules**, porting the Java line by line. `Announcements.find` sorts with `strength(null).reversed()`; use a stable sort with the same comparator.
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(game): rules, announcements and scoring`.

---

### Task 4: Card play

**Files:**
- Create: `crates/beelot-game/src/board.rs`; modify `error.rs`
- Port from Java: `GameBoard` (without `GameBoardView`, `GameBoardSeat`, `declarationMessage` or player names)
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

      Both `assert!` the invariants Java checks with `IllegalArgumentException`.
    - `fn play(&mut self, seat: Seat, card: Card, declare: bool) -> Result<(), GameError>`
    - `fn continue_after_trick(&mut self) -> Result<(), GameError>`
    - `fn play_view(&self, seat: Seat) -> PlayView` and `fn legal_cards(&self, seat: Seat) -> Vec<Card>`
    - accessors `active_seat()`, `reviewing_completed_trick()`, `round_result() -> Option<&RoundResult>`, `round_summary() -> Option<RoundSummary>`, `trump()`, `variant()`, `contract_value()`, `coinched()`, `declarer()`, `dealer()`, `hand(seat) -> &[Card]`, `visible_trick() -> &[SeatCard]`, `completed_tricks()`, `next_leader()`, `scores() -> (i32, i32)`, `belote_seat() -> Option<Seat>`, `belote_cards_played() -> u32`, `belote_bonus_awarded()`, `declared() -> &[Declared]` and `revealed() -> &[Declaration]`
  - `GameError` variants, with codes in brackets:
    - `NotYourTurn` (`not-your-turn`)
    - `IllegalCard` (`illegal-card`)
    - `RoundEnded` (`round-ended`)
    - `NoCompletedTrick` (`no-completed-trick`)

- [ ] **Step 1: Write the failing tests.** Port the five Java test classes, replacing the player UUIDs with seats 0–3 and `assertThrows(PrivateTableConflictException…)` with `assert_eq!(…, Err(GameError::…))`. Add:
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
- Port from Java: `BiddingState` (without `BiddingView`, `PlayerCall`, `AuctionCall`, `message` or `auctionId`; those are player-facing and move to phase 2)
- Port tests: `BiddingStateTest` and `ContreeBiddingStateTest`, as `crates/beelot-game/tests/bidding.rs`

**Interfaces:**
- Consumes: Task 2 (`RandomSource`, `java_shuffle`, `Card::deck`) and Task 4 (`GameBoard::from_bidding`, `GameBoard::from_contract`).
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
    - `fn apply(&mut self, seat: Seat, decision: AuctionDecision) -> Result<(), GameError>` (Java's `BotTurns.apply`)
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

- [ ] **Step 1: Write the failing tests.** Port both Java test classes as in Task 4. Add:
  - `classic_double_pass_redeals_and_moves_dealer`: in Classic with dealer 3, eight passes give `deals() == 2`, `dealer() == 0` and `active_seat() == 1` (Review Focus 4).
  - `same_seed_same_hands`: two `BiddingState::new(Classic, 0, Box::new(JavaRandom::new(7)))` give equal `hand(s)` for every seat.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game --test bidding`; expect compilation errors.
- [ ] **Step 3: Implement `bidding.rs`**, porting `BiddingState` and dealing exactly as `dealAgain` does: `java_shuffle` of `Card::deck()`, then hands taken from the front in seat order 0–3, then the upturned card in Classic.
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
  - `fn replay(deal: &Deal, check: &mut dyn FnMut(&Step, &mut SplitMix64))`. For each step, it asserts that the engine's view equals the recorded view, then calls `check`, then applies the recorded decision. `check` receives the deal's single `SplitMix64::new(seed)` so bot tests can draw from it exactly as Java's arena did. At the end it asserts that the result equals the recorded result (`None` ⇔ redeal, that is `deals() == 2`).

- [ ] **Step 1: Write the test** `engine_matches_every_recorded_deal`: for every deal, `replay(deal, &mut |_, _| {})`. A failure message names the file, the line, the step index and both values.
- [ ] **Step 2: Run it.** Run `cargo test -p beelot-game --test deals`. Expected: PASS. Any failure is a porting bug in Tasks 2–5: fix it in that module, add a unit test there that reproduces it, and note it in the commit message.
- [ ] **Step 3: Commit** with the message `test(game): replay the Java fixtures through the engine`.

---

### Task 7: Rule-based bot

**Files:**
- Create: `crates/beelot-game/src/bot/{mod.rs, memory.rs, rule_based.rs, turns.rs}`
- Port from Java: `BotStrategy`, `CardMemory`, `RuleBasedStrategy` and `BotTurns`
- Port tests: `CardMemoryTest`, `RuleBasedStrategyBiddingTest` and `RuleBasedStrategyPlayTest` (with `RecordingStrategy` as a test helper)

**Interfaces:**
- Produces:
  - `pub trait BotStrategy: Send + Sync { fn decide_auction(&self, view: &AuctionView) -> AuctionDecision; fn choose_card(&self, view: &PlayView) -> Card; }`
  - `struct RuleBasedStrategy`, with `RuleBasedStrategy::new()`, `RuleBasedStrategy::without_trump_control()` and `pub fn support_raise(hand: &[Card], suit: Suit) -> i32`. Java's other package-private statics become `pub(crate)`.
  - `CardMemory` (`pub(crate)`). Its `known_cards(seat)` returns `Vec<Card>` in suit-then-rank order.
  - `turns::take_auction_turn(bidding: &mut BiddingState, strategy: &dyn BotStrategy) -> Result<(), GameError>`
  - `turns::play_turn(board: &mut GameBoard, strategy: &dyn BotStrategy) -> Result<(), GameError>` (always with `declare = true`)
- Consumes: Task 6's `replay`.

- [ ] **Step 1: Write the failing tests.**
  - Port the three Java test classes.
  - In `tests/deals.rs`, add `current_bot_matches_java`. It replays every deal and, for steps whose seat is played by `"current"`, asserts that `RuleBasedStrategy::new()` gives the recorded decision. The seat's bot is `north_south` for even seats and `east_west` for odd ones.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game`; expect compilation errors.
- [ ] **Step 3: Implement the bot modules.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS.
- [ ] **Step 5: Commit** with the message `feat(bot): rule-based bot`.

---

### Task 8: Double-dummy solver and Challenging bot

**Files:**
- Create: `crates/beelot-game/src/bot/{solver.rs, sampling.rs, strategies.rs}`
- Create: `crates/beelot-game/benches/solver.rs`
- Port from Java: `DoubleDummySolver`, `SamplingStrategy`, `BotStrategies` and `SolverBenchmark` (as a criterion benchmark)
- Port tests: `DoubleDummySolverTest`, `SamplingStrategyTest` and `BotStrategiesTest`

**Interfaces:**
- Produces:
  - `struct Position { hands: [Vec<Card>; 4], trump: Suit, tricks: Vec<Vec<SeatCard>>, current_trick: Vec<SeatCard>, first_leader: Seat }`, with `Position::start(hands, trump, leader)`.
  - `struct Solution { north_south_points: i32, east_west_points: i32, best_card: Option<Card> }`
  - `struct DoubleDummySolver`, with `DoubleDummySolver::new()`, `fn solve(&mut self, position: &Position) -> Solution` and `fn nodes(&self) -> u64`. Its tables are `Vec`s of `1 << 20` entries on the heap, never stack arrays.
  - `struct SamplingStrategy`, with:
    - `SamplingStrategy::new(seed: i64)`: 50 samples, `Some(200 ms)`, first searched trick 3
    - `SamplingStrategy::with_settings(seed: i64, samples: usize, budget: Option<Duration>, first_searched_trick: usize)`

    `None` means no deadline. Solvers are pooled in a `Mutex<Vec<DoubleDummySolver>>`. The fingerprint uses `Rank::java_hash` and wrapping `i64` arithmetic, exactly as `SamplingStrategy.fingerprint` and `mix` do.
  - `strategies::for_difficulty(d: Difficulty) -> &'static dyn BotStrategy` and `strategies::for_tables()`. The Challenging seed is drawn once from the OS, through `std::hash::RandomState` hashing, so there's no new dependency.

- [ ] **Step 1: Write the failing tests.**
  - Port the Java tests. Where `SamplingStrategyTest` relies on the budget never cutting the search, use `budget: None`.
  - Add `sampling_without_budget_is_repeatable`: one late-round `PlayView` taken from the fixtures gives the same card from two separate `with_settings(1, 50, None, 3)` instances (Review Focus 1).
  - In `tests/deals.rs`, add `challenging_bot_matches_java`, as Task 7 does, with `SamplingStrategy::with_settings(1, 50, None, 3)` for `"challenging"` seats.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-game`; expect compilation errors.
- [ ] **Step 3: Implement the modules and the `solver` benchmark.** The benchmark ports `SolverBenchmark.run`'s deal generation: `JavaRandom::new(1)`, `java_shuffle`, 8 cards per seat, trump `Suit::ALL[next_int(4)]`, leader `(deal % 4 + 1) % 4`. It times `solve(Position::start(…))` over 20 deals.
- [ ] **Step 4: Run the tests and the benchmark.** Run `cargo test -p beelot-game && cargo bench -p beelot-game --bench solver -- --quick`. Expect PASS, and the benchmark printing its timings. The whole `cargo test --workspace` should finish in under 2 minutes; if it doesn't, mention it in the commit message rather than marking the fixtures `#[ignore]`.
- [ ] **Step 5: Commit** with the message `feat(bot): double-dummy solver and Challenging bot`.

---

### Task 9: Arena bots

**Files:**
- Create: `crates/beelot-game/src/bot/arena.rs`
- Port from Java: `ArenaBot` and the bots `CurrentBot`, `ChallengingBot`, `BaselineBot`, `PreviousPlayBot`, `PreviousTrumpsBot`, `NoCoincheBot`, `NoAnnouncementBiddingBot` and `RandomCardBot`

**Interfaces:**
- Produces:
  - `pub trait ArenaBot: Send + Sync { fn name(&self) -> &'static str; fn decide_auction(&self, view: &AuctionView, rng: &mut dyn RandomSource) -> AuctionDecision; fn choose_card(&self, view: &PlayView, rng: &mut dyn RandomSource) -> Card; }`
  - `pub const ARENA_BOTS: [&str; 8] = ["current", "challenging", "baseline", "previous-play", "previous-trumps", "no-coinche", "no-announcement-bidding", "random"];`
  - `pub fn arena_bot(name: &str) -> Result<Box<dyn ArenaBot>, UnknownBot>`. `UnknownBot`'s `Display` is `Unknown bot <name>. Available: [current, challenging, …]`.
  - `pub fn arena_bot_unbounded(name: &str) -> Result<Box<dyn ArenaBot>, UnknownBot>`: the same, except `"challenging"` uses `budget: None`. It's used by the fixture replay.

- [ ] **Step 1: Write the failing test.** In `tests/deals.rs`, replace the per-bot tests from Tasks 7 and 8 with `every_bot_matches_java`. For every step, it gets the seat's bot through `arena_bot_unbounded(name)` and asserts that its decision, drawn from the replay's shared `SplitMix64`, equals the recorded one. A failure names the bot.
- [ ] **Step 2: Run it to make sure it fails.** Run `cargo test -p beelot-game --test deals`; expect compilation errors.
- [ ] **Step 3: Implement `arena.rs`.**
- [ ] **Step 4: Run the tests to make sure they pass.** Run `cargo test -p beelot-game`; expect PASS. Every recorded decision of every bot now matches Java.
- [ ] **Step 5: Commit** with the message `feat(bot): arena bots`.

---

### Task 10: Arena runner, report and CLI

**Files:**
- Create: `crates/beelot-arena/Cargo.toml` (lib + bin) and `crates/beelot-arena/src/{lib.rs, arena.rs, report.rs, java_format.rs, main.rs}`
- Create: `crates/beelot-arena/tests/reports.rs`
- Port from Java: `BotArena`, `ArenaReport` and `BotArenaMain`. Port test: `BotArenaTest`.
- Modify: `README.md`, adding a "Bot arena" section with the CLI usage and the solver benchmark command.

**Interfaces:**
- Consumes: Task 9 (`arena_bot`), Task 5 (`BiddingState`), Task 4 (`GameBoard`), Task 3 (`MatchScore`) and Task 2 (`JavaRandom`, `SplitMix64`).
- Produces:
  - `pub struct BotArena`, with `BotArena::new(first: Box<dyn ArenaBot>, second: Box<dyn ArenaBot>)` and `fn run(&self, variant: Variant, deal_pairs: usize, seed: i64, announcements: bool) -> ArenaReport`. It panics when `deal_pairs < 2`, as Java does.
  - `pub struct ArenaReport` with Java's fields, plus `fn format(&self) -> String`.
  - `java_format::fixed(x: f64, decimals: usize, plus: bool) -> String`: Java's `%.Nf` / `%+.Nf`. Take the shortest round-trip decimal digits of `|x|` (from Rust's `{:e}`), round them half-up to `decimals` places, and prefix `-` when `x < 0.0` or `x` is `-0.0`, or `+` when `plus` is set and `x` isn't negative.
  - `java_format::java_sum(values: impl IntoIterator<Item = f64>) -> f64`: the Kahan summation of `Collectors.sumWithCompensation`. Return `sum + compensation`, or the simple sum when that is NaN and the simple sum is infinite.
- Port details:
  - The arena's RNG order is `SplitMix64::new(seed)`, then one `next_long()` per pair to get `deal_seed`. Each of the pair's two plays uses `JavaRandom::new(deal_seed)` for the bidding and its own `SplitMix64::new(deal_seed)` for the bots.
  - Averages and variance use `java_sum`. Keep the Wilson formula's operation order.
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
  - `fixed_rounds_shortest_form_half_up`: `fixed(1.005, 2, false) == "1.01"`, `fixed(0.125, 2, false) == "0.13"`, `fixed(2.5, 0, false) == "3"`, `fixed(-0.001, 2, true) == "-0.00"` and `fixed(3.0, 2, true) == "+3.00"` (Review Focus 2).
  - `java_sum_compensates`: `java_sum(std::iter::repeat(0.1).take(10)) == 1.0` (Review Focus 3).
  - `reports_match_java`: for each file in `fixtures/arena/`, parse the configuration from its name, run `BotArena` with 100 pairs and seed 1, and assert that `format()` equals the file, line for line, skipping lines that start with `Decision time per card:`.
  - Port `BotArenaTest`.
- [ ] **Step 2: Run them to make sure they fail.** Run `cargo test -p beelot-arena`; expect compilation errors.
- [ ] **Step 3: Implement the crate.**
- [ ] **Step 4: Run the tests and the CLI.** Run `cargo test --workspace && cargo run --release -p beelot-arena -- --deals 200 --variant contree`. Expect PASS, and a Contrée report ending with `Report written to …/target/bot-arena/report.txt`.
- [ ] **Step 5: Run every check.** Run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`; expect all clean.
- [ ] **Step 6: Commit** with the message `feat(arena): bot arena runner, report and CLI`.
