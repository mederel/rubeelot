# Keep the match score in view (US-096) — design

## Goal

A score bar stays on screen for the whole game, so a player always knows how
close each team is to winning. From the backlog (`docs/product-backlog.md`):

- It stays visible while dealing, bidding, playing tricks and between rounds,
  in bot games, at private tables and in pass-and-play. It never scrolls out of
  view or hides the cards, on a laptop screen (US-039) and on a phone.
- It shows each team's match score, the match target (US-062), and names my team.
- It also shows each team's points so far in the round. The contract is not
  repeated there: US-099 shows it at the bidder's seat.
- Both values update as soon as a round ends.
- Screen readers get it as a live region that announces changes without
  interrupting play (US-074).
- New texts are translated into French and Dutch.

Frontend only (`crates/beelot-server/static/`): no server change, no new
dependency, no new Settings option.

## Decisions

- **One sticky bar, replacing three scoreboards.** Decided with the user. Today
  the bot game has `.scoreboard` + `#match-score`, the private table has
  `#private-scoreboard` + `#private-match-score`, and each is inside its own
  panel, so the match score is a line of text that scrolls away. One
  `#score-bar` with `position: sticky; top: 0` replaces them all.
- **The count-up is not part of this story.** The bar sets the numbers directly.
  The count-up and progress bar belong to US-091, which can later animate the
  bar's numbers through the effects pattern of
  `2026-10-31-effects-level-design.md`. The "update as soon as a round ends"
  criterion is met by the direct update; the count-up clause of the backlog is
  deferred to US-091 and the backlog text is amended to say so.
- **Pure rendering function, as for the contract badge.** A new module builds the
  bar from plain data, so it is unit tested in Node without a browser.
- **"My team" is the viewer's team.** In bot games and at private tables the
  viewer is fixed. In pass-and-play the viewer changes with each hand-off, so the
  bar names the team of the player now at the screen (`board.currentPlayerIndex`
  parity, as `loadBotGame` already uses for sounds). During the hand-off cover
  the bar shows no "you" mark, so it does not reveal who is next.
- **Round points are hidden before the board exists.** During dealing and
  bidding there is no board, so the bar shows the match scores and the target
  only. Round points appear when play starts and reset to zero with each new
  round.

## The pure module: `static/score-bar.js`

Loaded before `app.js`, ends with the same `module.exports` guard as
`contract-badge.js`.

- `scoreBarModel({ match, board, myTeam })` returns `null` when there is no
  `match`, else
  `{ teams: [{ name, matchScore, roundPoints, mine }, { … }], target,
  complete }`:
  - `name` is `"North–South"` / `"East–West"` (the server's names, translated by
    the caller with `t()`);
  - `matchScore` is `match.northSouth` / `match.eastWest`;
  - `roundPoints` is `board.northSouthScore` / `board.eastWestScore`, or `null`
    when `board` is absent;
  - `mine` is true for the team at `myTeam` (0 = North–South, 1 = East–West,
    `null` = no mark);
  - `target` is `match.target`.
- `scoreBarAnnouncement(model, t)` returns the sentence for screen readers, e.g.
  "Match score: North–South 420, East–West 310, to 1,000." Round points are left
  out, because they change on every trick and would interrupt play.

## What changes

### `index.html`

- Add `<div id="score-bar" class="score-bar" hidden>` once, at the top of the
  page content, outside the three game views, with the two team chips, the
  target in the middle, and a visually hidden `<span id="score-bar-live"
  role="status" aria-live="polite">`.
- Remove `.scoreboard` / `#north-south-score` / `#east-west-score` /
  `#match-score` from the bot game, and `#private-scoreboard`,
  `#private-north-south-score`, `#private-east-west-score` and
  `#private-match-score` from the private table. Pass-and-play uses the bot-game
  view and needs no markup of its own.

### `app.js`

- `showScoreBar(match, board, myTeam)` fills the chips and the target from
  `scoreBarModel`, unhides the bar, and rewrites `#score-bar-live` **only when
  the announcement text changed** (compared with the last one), so tricks never
  trigger an announcement.
- `hideScoreBar()` hides it when the player leaves a game view (home, results,
  settings, leaving a table), through the existing view-switching code.
- Callers, replacing the writers listed above:
  - `loadBotGame` (bot game and pass-and-play): `showScoreBar(match, board,
    handedOff ? null : board.currentPlayerIndex % 2)`; the bidding render in the
    same flow passes `board = null`;
  - private table bidding: `showScoreBar(match, null, you % 2)`
    (replaces `renderPrivateMatchScore(match)`);
  - `renderPrivateBoard`: `showScoreBar(match, board, you % 2)`.
- When `match.complete`, the bar shows the final scores and keeps them until the
  player leaves.
- `renderPrivateMatchScore` and its call sites go. Now-unused translation keys
  ("Match score — North–South {0} · East–West {1} · to {2}", unless the
  announcement reuses it) go with them.

### `styles.css`

- `.score-bar`: `position: sticky; top: 0; z-index` above the table and below
  modals and the hand-off cover; same cartoon tokens as the rest of the table
  (outlines, saturated team colours, rounded chips).
- Laptop: one line, `team · match score · +round points`, the target centred.
- Phone (≤ 600 px): two lines, scores on the first and the target with round
  points on the second; total height ≤ 64 px so it does not push the hand off
  screen at 390 × 844. The table's top margin accounts for the bar so the
  seats are not covered.
- "Mine" is a bold outline plus the translated word "You" (not colour alone).
- Honours the effects level (US-094): no transition at Off, a short fade at
  Reduced, none needed at Full since the numbers are not animated in this story.

### `i18n.js`, `i18n-nl.js`

French and Dutch for: "You" (team mark), "to {0}" or the target label, the
screen-reader sentence, and the "round points" label (used only as `title` /
visually hidden text next to the "+N").

## Testing

- **Node tests** `frontend-tests/score-bar.test.cjs`: `scoreBarModel` (null
  match, auction with no board → `roundPoints` null, play with board, my team
  0 / 1 / null, complete match, large scores), `scoreBarAnnouncement` (wording,
  no round points, thousands separator through the formatter passed in).
- **Browser** (Playwright, French, then English and Dutch once each; 1280 × 800
  and 390 × 844): a bot Contrée game from dealing to the end of a round and to
  the match victory; a classic game; a private table (auction, play, round end);
  pass-and-play across the hand-off cover (the "You" mark follows the viewer
  and is absent on the cover); reload mid-round; scroll the page and check the
  bar stays; check it never overlaps the hand or the seats on both sizes;
  effects Reduced and Off; accessibility tree shows one live region that speaks
  only when a round ends; no console errors.
- Project checks from `AGENTS.md`: `cargo fmt --check`, `cargo clippy
  --all-targets -- -D warnings`, `cargo test --workspace` (Docker running).

## What the verification found

Checked in the browser (Playwright, 2026-11-01) in bot Contrée and classic
games, pass-and-play and a private table started with bots.

- **Round points are the current round's points.** `board.northSouthScore`
  and `board.eastWestScore` start at 0 with each deal (the bar showed `+0`
  for both teams at the first card of every round) and grow trick by trick
  with card points, the last trick's 10, announcements and Belote. They are
  the round's raw points, not what the round adds to the match score: in a
  classic round where the contract is made the two are equal (`+88`/`+74`,
  match 88–74), but in Contrée the match adds the contract's result
  instead (a capot showed `+182` while East–West's match score went from 0
  to 370; another round showed `+92` while North–South's rose by 182). The
  breakdown under the table explains the difference, so `scoreBarModel`
  keeps the field as it is.
- **Bot and pass-and-play snapshots carry `match` during bidding.** The bar
  appeared with both match scores and the target, and no round points, on
  the first bidding render of a bot game and of a pass-and-play game, as it
  does at a private table. The local table starts its match when it is
  created (`Table::begin_match`), before the first deal.

## Out of scope

- The count-up and the progress bar (US-091).
- The contract (US-099) and the dealer (US-097), which stay at the seats.
- Any server or API change.

## Known limits

- During bidding the bar shows no round points, as there are none yet.
- Between the end of a round and the next deal the bar shows the finished
  round's points beside the new match score until the next round starts.
