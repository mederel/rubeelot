# Watch the score move (US-091) — design

## Goal

The scores of the score bar (US-096) count from the old value to the new one,
and a progress bar shows each team's distance to the match target. From the
backlog (`docs/product-backlog.md`):

- Round and match scores count from the old value to the new one.
- A progress bar shows each team's distance to the match target (US-062).

Every effect honours the effects level of US-094
(`2026-10-31-effects-level-design.md`). Frontend only
(`crates/beelot-server/static/`): no server change, no new dependency, no new
Settings option.

## Decisions

- **Scope is the score bar only.** The round-result breakdown text and the
  scores elsewhere do not animate. Decided with the user.
- **Progress bar: one thin bar under each team chip (option A).** Each team's
  distance to the target reads on its own. Rejected: one shared bar with both
  teams filling toward the middle (harder to read as a distance to the target).
  Decided with the user.
- **Only rises animate.** A value that goes down snaps to its new value: round
  points back to 0 at a new round, or the match score back to 0 at a rematch.
- **The first value shown for a game does not animate**, so opening or reloading
  a game mid-match shows the score at once. The bar remembers the numbers it last
  displayed, per game, using the same game key as the live region of US-096.
- **The count runs when a new score arrives**, so a match score counts while the
  last trick of the round is still on view, not at the later reveal of the round
  result. This keeps "update as soon as a round ends" (US-096); syncing with the
  reveal is possible later without changing the module.
- **Screen readers still hear the final match score only**, through the live
  region of US-096. The counting numbers and the progress bars are hidden from
  assistive technology (`aria-hidden`) so no intermediate value is read.
- **The count is not movement**, but it is an animation of state, so Reduced and
  Off show the final number at once (no count); the progress bar follows the
  pattern for one-shot movement.

## The pure functions, in `static/score-bar.js`

- `scoreBarModel` adds `progress` to each team: `matchScore / target` clamped to
  0 to 1 (0 when `target` is missing or not positive). Scores above the target
  give 1.
- `countValue(from, to, t)` returns the integer shown at progress `t` (0 to 1) of
  a count from `from` to `to`, with an ease-out curve: `from` at `t <= 0`, `to`
  at `t >= 1`, and a value in between that never overshoots `to` and never
  decreases as `t` grows. `from >= to` returns `to`.

Both are exported with the existing `module.exports` guard and unit tested.

## What changes

### `app.js`

- `showScoreBar(match, board, myTeam)` keeps, per game, the match score and round
  points last displayed for each team. For each of the four numbers:
  - no previous value (first show of the game), or the new value is not above the
    previous one, or the effects level is not Full: write the number at once;
  - otherwise count from the displayed value to the new one with
    `requestAnimationFrame` and `countValue`, over 700 ms for the match score and
    300 ms for the round points (they change on every trick).
  - A new target while a count is running restarts from the value currently shown.
  - Counts are cancelled when the bar is hidden or the game changes (the game-key
    reset of US-096 also clears the remembered numbers).
- The progress bar of each chip is set to `progress` through a CSS custom
  property (`--progress`, 0 to 1) on the chip.
- Counting numbers get `aria-hidden="true"` while a count runs and lose it when it
  ends, so assistive technology never reads an intermediate value; the live region
  and the group are untouched.
- Pass-and-play and private tables use the same `showScoreBar`, so they inherit
  the behaviour. Nothing else calls the animation.

### `index.html`

A `<span class="score-progress" aria-hidden="true"><i></i></span>` in each
`.score-team`, after the existing content.

### `styles.css`

- `.score-progress`: a 6 px high track, full width of the chip, rounded and
  outlined in the cartoon style of the bar; its `i` fills with the team colour and
  is scaled with `transform: scaleX(var(--progress))` (origin left), so only
  `transform` animates.
- Full: `transition: transform 700ms ease-out`. Reduced: `transition: transform
  var(--effect-fast)` (150 ms). Off: no transition. Written under the existing
  `body[data-effects]` rules and `--effect-fast` token.
- Phone (≤ 600 px): the bar stays at or below 64 px (56 px today); the track is
  inside the chip, so it adds about 8 px at most. The bar must still not overlap
  the seats or the hand at 390 × 844.
- A team's bar has a minimum visible width at 0 so the track is recognisable
  (the track outline shows even when `--progress` is 0).

### `i18n.js`, `i18n-nl.js`

No new visible text. The progress bars are decoration and hidden from assistive
technology; the target is already spoken in the live region sentence.

## Testing

- **Node tests** `frontend-tests/score-bar.test.cjs` (extended):
  `scoreBarModel` progress (0, half, exactly at the target, above it, a missing or
  zero target gives 0); `countValue` (starts at `from`, ends at `to`, never
  exceeds `to`, is non-decreasing over 0, 0.1, … 1, `from >= to` returns `to`,
  `t` outside 0 to 1 is clamped).
- **Browser** (Playwright, French, then English and Dutch once; 1280 × 800 and
  390 × 844): a bot Contrée round end counts both match scores and the progress
  bars grow; round points count on a trick and snap back to 0 at the next round; a
  rematch snaps to 0 without a count; reloading mid-match shows the score at once;
  effects Reduced (final numbers at once, 150 ms bar) and Off (instant); the
  progress bar is full for the winner at the match victory; pass-and-play and a
  private table behave the same; the accessibility tree has the same single live
  region and no intermediate number is announced; phone bar ≤ 64 px; no console
  errors.
- Project checks from `AGENTS.md`: `cargo fmt --check`, `cargo clippy
  --all-targets -- -D warnings`, `cargo test --workspace` (Docker running), plus
  `node --test crates/beelot-server/frontend-tests/`.

## Out of scope

- Animating the round-result breakdown, the history page or any other score.
- Syncing the count with the round-result reveal.
- Celebration effects for the match victory (US-090 and US-093).
- Any server or API change.

## Known limits

- The count of the match score starts as soon as the snapshot arrives, so it may
  run while the last trick of the round is still on view.
- The count is skipped at Reduced and Off by design, not by a separate setting.
- Verification (2026-11-01) found the phone bar's height depended on the
  platform font: the team name, round points and "You" mark used `line-height:
  normal`, and the "You" mark set the second row. They now use 1.2, and the bar
  measures 61.41 px at 390 and 320 px wide (it was 62.86 px).
- At 320 px wide with four-digit scores ("1 980"), the viewer's chip content is
  1 px wider than the chip. The page does not scroll sideways and the bar keeps
  its height; this comes from the US-096 chip layout, not the progress track.
- Leaving a game mid-count stops the count where it is; coming back shows the
  final score at once, since the game key resets what the bar has shown.
