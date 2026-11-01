# Score Count-Up and Progress Bar (US-091) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The score bar's numbers count up to their new values and a thin progress bar under each team chip shows its distance to the match target, at every effects level.

**Architecture:** Two pure functions in `score-bar.js` (`progress` in the model, `countValue`) tested in Node. `showScoreBar` in `app.js` remembers the numbers last displayed per game and animates rises with `requestAnimationFrame` at Full only. The progress bar is CSS (`transform: scaleX(var(--progress))`) under the existing `body[data-effects]` rules.

**Tech Stack:** Vanilla JS and CSS in `crates/beelot-server/static/`, Node `node:test`, Playwright MCP for browser checks.

**Spec:** `docs/superpowers/specs/2026-11-01-score-count-up-design.md` (stacked on `feat/score-bar`, PR #22; effects patterns in `docs/superpowers/specs/2026-10-31-effects-level-design.md`).

## Global Constraints

- Frontend only: no server change, no dependency, no new Settings option, no new visible text (no new translations).
- Only `transform` and `opacity` animate; Full bar transition `700ms ease-out`, Reduced `var(--effect-fast)` (150ms), Off none.
- Count durations: 700 ms for match scores, 300 ms for round points; only rises animate, drops snap; the first value shown for a game does not animate; Reduced and Off write the final number at once.
- Counting numbers and progress bars are `aria-hidden`; the live region of US-096 is unchanged and never announces intermediate values.
- Phone bar (≤ 600 px) stays ≤ 64 px at 390 × 844 and does not overlap seats or hand.
- Project checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running), `node --test crates/beelot-server/frontend-tests/`.

## Review Focus

- Target missing, 0 or negative, or a score above the target: progress is clamped to 0–1, never NaN. Test: Task 1 progress cases.
- A new score arriving while a count runs (two fast round updates, bidding → board): restarts from the value shown, never from the stale one, never overshoots. Test: Task 1 `countValue` bounds; browser in Task 3.
- Rematch / new round (values drop): snap, no backwards count. Test: Task 2 browser; Task 3.
- Leaving the game or changing game mid-count: no timer keeps writing into the hidden or next game's bar. Test: Task 3 browser (navigate home during a count).
- Reload mid-match and the first show of a game: shows the score at once. Test: Task 3 browser.

---

### Task 1: Pure functions in `score-bar.js`

**Files:**
- Modify: `crates/beelot-server/static/score-bar.js`
- Test: `crates/beelot-server/frontend-tests/score-bar.test.cjs`

**Interfaces:**
- Produces:
  - `scoreBarModel` teams gain `progress: number` = `matchScore / target` clamped to [0, 1]; `0` when `target` is missing, not finite or ≤ 0, or `matchScore` is not finite.
  - `countValue(from: number, to: number, t: number) -> number`: integer shown at progress `t` of a count. `t` is clamped to [0, 1]; returns `to` when `from >= to` or `t >= 1`, `from` when `t <= 0`; otherwise `from + Math.round((to - from) * ease(t))` with an ease-out curve (e.g. `1 - (1 - t) ** 3`), never above `to`, non-decreasing in `t`.
  - Both exported in the existing `module.exports` line.

- [ ] **Step 1: Write the failing tests** appended to `score-bar.test.cjs`: `progress is the share of the target`: match `{northSouth: 500, eastWest: 250, target: 1000}` → progress 0.5 and 0.25; `progress is clamped`: 1200 with target 1000 → 1; negative score → 0; `progress without a usable target is 0`: target `0`, `null`, `undefined` → 0 for both teams; `countValue endpoints`: `countValue(100, 250, 0) === 100`, `countValue(100, 250, 1) === 250`, `t` of -1 and 2 are clamped; `countValue never decreases`: over t = 0, 0.1, … 1 the values are non-decreasing and ≤ `to`; `countValue with no rise returns to`: `countValue(300, 300, 0.5) === 300` and `countValue(300, 100, 0.5) === 100`; `countValue returns integers`.
- [ ] **Step 2: Run** `node --test crates/beelot-server/frontend-tests/score-bar.test.cjs`. Expected: FAIL (`countValue` not a function / progress undefined).
- [ ] **Step 3: Implement** `progress` in `scoreBarModel` and `countValue` per the Interfaces block; update the header comment of the model shape.
- [ ] **Step 4: Run** the same command, then the whole folder. Expected: all PASS.
- [ ] **Step 5: Commit** `feat: model the score progress and count (US-091)` (message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`).

---

### Task 2: Progress bars and count-up in the page

**Files:**
- Modify: `crates/beelot-server/static/index.html` (the `.score-team` chips in `#score-bar`)
- Modify: `crates/beelot-server/static/app.js` (`showScoreBar`, `hideScoreBar`, the game-key reset in `renderView`; `effectsLevel()` at ~1541 is the level helper)
- Modify: `crates/beelot-server/static/styles.css` (`.score-bar`/`.score-team` rules ~376-434; effects rules ~1215-1283)

**Interfaces:**
- Consumes: `scoreBarModel(...).teams[i].progress`, `countValue(from, to, t)` (Task 1); `effectsLevel()` returning `"full" | "reduced" | "off"`; `formatNumber`.
- Produces: no new public names beyond module-level helpers in `app.js`.

- [ ] **Step 1: Markup.** Add `<span class="score-progress" aria-hidden="true"><i></i></span>` as the last child of each `.score-team` in `index.html`.
- [ ] **Step 2: Style the bar** in `styles.css`: `.score-progress` (6 px high track, full chip width, rounded, thin outline in the cartoon tokens, so the empty track is visible); its `i` fills the track with the team colour and uses `transform: scaleX(var(--progress, 0)); transform-origin: left`; chip custom property `--progress` set from JS. Transition: Full `transform 700ms ease-out`; `body[data-effects="reduced"]` uses `var(--effect-fast)`; `body[data-effects="off"]` none (follow the patterns and selectors around lines 1215-1283). Make the chip layout hold the track as an extra row without raising the phone bar above 64 px (check the ≤ 600 px block ~422-434; East–West chip is mirrored like the existing rules).
- [ ] **Step 3: Remember displayed numbers and animate.** In `app.js`, keep module state `shownScores` = per team `{ match, round }` last displayed (or none), reset together with `scoreBarSentence` in the game-key reset of `renderView`. In `showScoreBar`, for each of the four numbers: if there is no previous value, the new value is not above it, or `effectsLevel() !== "full"`, write the number at once; else count from the currently shown value to the new one with `requestAnimationFrame` and `countValue`, 700 ms for the match score and 300 ms for round points, writing `formatNumber(...)` (and `+` for round points). A new target while counting cancels the running count and restarts from the value currently shown. Keep one running-animation handle per number so `hideScoreBar()` and the game-key reset cancel them. Set `--progress` on each chip from `team.progress`. While a count runs set `aria-hidden="true"` on that number's element and remove it when it ends. Do not touch the live-region logic.
- [ ] **Step 4: Check** `node --check crates/beelot-server/static/app.js` and `node --test crates/beelot-server/frontend-tests/`. Expected: pass.
- [ ] **Step 5: Quick browser check** on the running server (port 8080 is started by the user; do not start or stop it): in a bot game the bars show progress, a trick counts the round points, and the numbers are correct after a round end; report what you saw. The full matrix is Task 3.
- [ ] **Step 6: Commit** `feat: count up the score bar and show progress to the target (US-091)`.

---

### Task 3: Verification and wrap-up

**Files:**
- Modify (if findings need recording): `docs/superpowers/specs/2026-11-01-score-count-up-design.md`

- [ ] **Step 1: Browser matrix** (Playwright, French, then English and Dutch once; 1280 × 800 and 390 × 844): a bot Contrée round end counts both match scores and the bars grow; round points count on a trick and snap to 0 at the next round; a rematch snaps to 0; reload mid-match shows the score at once; navigate to home during a count and back (no stale writes, no console errors); effects Reduced (final numbers at once, 150 ms bar) and Off (instant); the bar is full for the winner at match victory (use a 500-point target); pass-and-play and a private table with bots behave the same; the accessibility tree still has one live region and no intermediate number is announced; phone bar ≤ 64 px and no overlap. Fix any defect with a focused commit.
- [ ] **Step 2: Run the project checks:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running; if Testcontainers cannot run in the sandbox, say so exactly), `node --test crates/beelot-server/frontend-tests/`. Expected: all pass.
- [ ] **Step 3: Record findings** in the spec's "Known limits" if anything unexpected was seen; commit `docs: record what the score count-up verification found (US-091)`. Do not push.
