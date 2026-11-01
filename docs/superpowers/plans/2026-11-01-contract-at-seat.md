# Contract At The Bidder's Seat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Print the ongoing contract as a badge next to the avatar of the player who holds it, and remove every other place that prints the contract or the trump.

**Architecture:** A pure module derives the contract from the calls shown so far or from the board. One function, `showContractBadge`, draws the badge on the holder's seat; both `renderTableSeats` and `showPlayerCall` call it, so the badge never runs ahead of the bubbles. Frontend only.

**Tech Stack:** vanilla JS/CSS in `crates/beelot-server/static/`; local Node tests (`node --test <files>`); Playwright MCP for browser checks.

**Spec:** `docs/superpowers/specs/2026-11-01-contract-at-seat-design.md`

## Global Constraints

- Frontend only; no server change, no new dependency, no new Settings option.
- A contract is `null` or `{ player: string, value: number | null, suit: string, coinched: boolean }`; `value` is `null` in classic Belote; `suit` is kept as the server gives it ("Hearts", "All trumps") and shown through `suitName()`.
- Server call strings (`call_label`): `Pass`, `90 Hearts`, `Hearts` (chosen trump), `Coinche!`. `callKind()` already matches them.
- The badge is `aria-hidden`; the contract is read through the seat's `aria-label`. The existing call bubbles stay unchanged.
- The badge bounces only when the contract changes (player, value, suit or coinched), and not at effects level Reduced or Off.
- Only the coinche is marked ("×2"); surcoinche does not exist in the game.
- New texts are translated into French and Dutch. Keys that stay in use elsewhere are not deleted.
- AGENTS.md: run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running) before finishing; report what ran.

## Review Focus

- Re-rendering the same contract (reload mid-auction, the switch from the auction to the board, a seat re-render) draws the badge without bouncing again.
- A raise by another player moves the badge: the old holder's seat loses it; passes after a bid never clear it.
- A new deal starts with no badge: the previous round's contract does not leak into the next auction.
- A Contrée contract in tout atout or sans atout ("100 All trumps") reads correctly: no symbol exists, the translated name stays as text.
- The viewer's own call shows its badge as soon as its bubble does (no wait for the server), and a failed call (409) leaves no stale badge.

---

### Task 1: The pure contract module

**Files:**
- Create: `crates/beelot-server/static/contract-badge.js`
- Create: `crates/beelot-server/frontend-tests/contract-badge.test.cjs`
- Modify: `crates/beelot-server/static/index.html` (add `<script src="/contract-badge.js"></script>` after `/suit-text.js`, before `/app.js`)

**Interfaces:**
- Produces: `contractAfterCall(contract, player, call): Contract | null` — `call` is a server string. `"<integer> <suit>"` gives `{ player, value, suit, coinched: false }`; `Pass` leaves `contract` unchanged; `^coinche` (case-insensitive) gives `{ ...contract, coinched: true }`, or `null`/unchanged when there is no contract; any other non-empty string is a chosen trump: `{ player, value: null, suit: call, coinched: false }`; an empty `call` leaves `contract` unchanged.
- Produces: `contractFromAuction(calls: Array<{ playerName, call }>): Contract | null` — folds `contractAfterCall` from `null` over the list in order.
- Produces: `contractFromBoard(board): Contract | null` — `{ player: board.seats[board.declaringPlayerIndex].name, value: board.variant === "CONTREE" ? board.contractValue : null, suit: board.trump, coinched: Boolean(board.coinched) }`; `null` when `declaringPlayerIndex` is missing or negative.
- Ends with `if (typeof module !== "undefined") module.exports = { contractAfterCall, contractFromAuction, contractFromBoard };` like `suit-text.js`.

- [ ] **Step 1: Write the failing tests** in `contract-badge.test.cjs` (`require("../static/contract-badge.js")`, style of `suit-text.test.cjs`). Fixture `AUCTION` = the `auction` of `crates/beelot-server/tests/contract/contree_bidding.json` (`You 90 Hearts, Camille Pass, Luc 110 Hearts, Manon Pass`). One test each:
  - `a first bid makes the contract`: `contractAfterCall(null, "You", "90 Hearts")` deep-equals `{ player: "You", value: 90, suit: "Hearts", coinched: false }`.
  - `a raise by another player moves the holder`: from that contract, `contractAfterCall(c, "Luc", "110 Hearts")` has `player: "Luc"`, `value: 110`.
  - `passes keep the contract and a first pass gives none`: `contractAfterCall(c, "Camille", "Pass")` is `c`; `contractAfterCall(null, "Camille", "Pass")` is `null`; an empty call is unchanged.
  - `coinche marks the contract and keeps its holder`: `contractAfterCall(c, "Camille", "Coinche!")` deep-equals `{ ...c, coinched: true }`; `contractAfterCall(null, "Camille", "Coinche!")` is `null`.
  - `a chosen trump has no value`: `contractAfterCall(null, "Ana", "Spades")` deep-equals `{ player: "Ana", value: null, suit: "Spades", coinched: false }`.
  - `tout atout and sans atout bids keep their name`: `contractAfterCall(null, "Ana", "100 All trumps")` has `value: 100`, `suit: "All trumps"`; `"80 No trumps"` has `suit: "No trumps"`.
  - `an auction folds to its last bid`: `contractFromAuction(AUCTION)` is `{ player: "Luc", value: 110, suit: "Hearts", coinched: false }`; `contractFromAuction(AUCTION.slice(0, 2))` has `player: "You"`; `contractFromAuction([])` is `null`.
  - `a board gives its contract`: for `{ variant: "CONTREE", seats: [{name:"Ana"},{name:"Camille"},{name:"Luc"},{name:"Manon"}], declaringPlayerIndex: 2, contractValue: 100, trump: "Spades", coinched: true }` the result is `{ player: "Luc", value: 100, suit: "Spades", coinched: true }`; the same with `variant: "CLASSIC"` has `value: null`; without `declaringPlayerIndex` it is `null`.

- [ ] **Step 2: Run** `node --test crates/beelot-server/frontend-tests/contract-badge.test.cjs`. Expected: FAIL (module not found).

- [ ] **Step 3: Implement the three functions** in `contract-badge.js` as specified above (a bid is `/^(\d+)\s+(.+)$/`).

- [ ] **Step 4: Run the test file**. Expected: 8/8 PASS. Add the script tag to `index.html`; `node --check crates/beelot-server/static/contract-badge.js`.

- [ ] **Step 5: Commit** `feat: add the contract badge module (US-099)` (end with the session's `Co-Authored-By` line).

### Task 2: The badge on the seat

**Files:**
- Modify: `crates/beelot-server/static/app.js` (`renderTableSeats` ~1304, `showPlayerCall` ~1482)
- Modify: `crates/beelot-server/static/styles.css` (seat placements near `.dealer-chip` ~1019; effects rules ~1105–1136)
- Modify: `crates/beelot-server/static/i18n.js`, `crates/beelot-server/static/i18n-nl.js`

**Interfaces:**
- Consumes: `contractAfterCall`, `setSuitText(el, text, { symbolOnly: true })`, `suitName`, `effectsLevel()`.
- Produces: `renderTableSeats(prefix, seats, currentPlayerIndex = 0, contract = null)` — after rebuilding the stations it stores `shownContract[prefix] = contract` and calls `showContractBadge(prefix, contract)`.
- Produces: `showContractBadge(prefix, contract)` — the station whose `dataset.playerName` is `contract.player` gets `<span class="contract-badge" aria-hidden="true">` (after the dealer chip, before the call bubble), content `setSuitText(badge, "<value> <suitName(suit)>", { symbolOnly: true })` (value omitted when `null`) plus a `<span class="contract-mark">×2</span>` when coinched; every other station of the prefix loses its badge. `station.dataset.contractKey` holds `player|value|suit|coinched`; the badge gets class `badge-bounce` only when that key differs from the previous one. `null` removes all badges and keys. A suit that is not one of the four (All trumps, No trumps) stays as translated text (`setSuitText` leaves it alone).
- Produces: `shownContract: Record<string, Contract | null>` (module-level), advanced in `showPlayerCall` right after a bubble is shown: `shownContract[prefix] = contractAfterCall(shownContract[prefix] ?? null, station.dataset.playerName, call)`, then `showContractBadge`.
- Station `aria-label` gets the contract appended: `t(", contract {0}", "<value> <suit name>")` and `t(", coinched")` when coinched, only for the holder's station.

- [ ] **Step 1: Check the starting point** with `node --check` on `app.js`; there is no DOM test harness, so this task is verified in the browser in Task 4 and by reading the diff.

- [ ] **Step 2: Implement `showContractBadge` and the `renderTableSeats` parameter** as specified. In `renderTableSeats` the `aria-label` of the holder's station includes the contract; the badge is added through `showContractBadge` after `replaceChildren`.

- [ ] **Step 3: Advance the badge in `showPlayerCall`** after a new bubble is shown (the `return false` for an already shown call must not touch the badge). The player is `station.dataset.playerName` (the bottom station has it too).

- [ ] **Step 4: CSS.** `.contract-badge` beside the avatar for the four placements (`.station-top`, `.station-left`, `.station-right`, `.station-bottom`), clear of the dealer chip, the name and the call bubble; a rounded gold pill like the `[data-kind="bid"]` bubble, red text for hearts/diamonds (set a `data-red` on the badge from `setSuitText`'s returned suits, as the bubbles do); `.contract-mark` smaller and bold; keyframes `badge-bounce` (~.5 s) on `.contract-badge.badge-bounce`; and `body[data-effects="reduced"] .contract-badge, body[data-effects="off"] .contract-badge { animation: none; }` next to the other effects rules.

- [ ] **Step 5: Translations.** Add `", contract {0}"` and `", coinched"` to `i18n.js` (French: `", contrat {0}"`, `", coinché"`) and to `i18n-nl.js` (Dutch: `", contract {0}"`, `", gecoinched"`, matching the existing `" · coinched"`).

- [ ] **Step 6: `node --check`**, then commit `feat: draw the contract badge on the seat (US-099)`.

### Task 3: Feed the badge in every flow

**Files:**
- Modify: `crates/beelot-server/static/app.js` (the six `renderTableSeats` calls: ~529, ~641, ~741, ~997, ~1044, ~1610)

**Interfaces:**
- Consumes: `renderTableSeats(..., contract)`, `contractFromAuction`, `contractFromBoard`.

- [ ] **Step 1: Pass the contract at each call**:
  - bot auction (`loadBidding`, ~1610): `animateDeal ? null : contractFromAuction(bidding.auction)`. The opening reveal (`playOpeningDeal`) then advances the badge through `showPlayerCall`.
  - bot board (~997 and ~1044): `contractFromBoard(board)`.
  - private auction (~641): `contractFromAuction(shownCalls)` where `shownCalls` is the list the same function already uses for the bubbles (`revealed` plus the viewer's own call while `privateOwnCallShown`).
  - private board (~741): `contractFromBoard(board)`.
  - private lobby (~529): no contract (`null`).
  A new deal passes `null` (or an empty auction), which clears the previous round's badge and key.

- [ ] **Step 2: Check the viewer's own call.** In `submitBid` and `privateAction`, the own bubble comes from `showPlayerCall`, so the badge follows it. If the server then refuses the call (409), the next render passes the server's contract and replaces the badge; confirm that path in the browser (Task 4).

- [ ] **Step 3: `node --check`**, `node --test crates/beelot-server/frontend-tests/*.test.cjs` (expected: all PASS), commit `feat: show the contract badge at every table (US-099)`.

### Task 4: Remove the old contract lines and verify

**Files:**
- Modify: `crates/beelot-server/static/index.html` (lines ~142, 147, 211, 303, 339)
- Modify: `crates/beelot-server/static/app.js` (writers at ~694–696, ~731–733, ~743, ~953–955, ~1623–1625), `i18n.js`, `i18n-nl.js`

**Interfaces:**
- `#private-game-heading` at the board shows `board.variant === "CONTREE" ? t("Contrée") : t("Classic Belote")` (both keys exist), set with `textContent` (the `setSuitText` call is no longer needed there).

- [ ] **Step 1: Remove** `#trump-suit` (and its "Trump" `<span>`), `#contract-summary`, `#current-contract`, `#private-trump` (and its span) and `#private-current-contract` from `index.html`, and their writers from `app.js` (including the `hidden` toggle at ~696). Keep `updateAmountChoices` and the other bid controls. Run `grep -n 'trump-suit\|contract-summary\|current-contract\|private-trump"\|private-current-contract' crates/beelot-server/static/*.js crates/beelot-server/static/*.html crates/beelot-server/tests -r` to confirm nothing else reads them.

- [ ] **Step 2: Remove the translations only they used**, after `grep -c` shows no other use: `Contract: {0} {1}{2}`, `Current contract: {0} {1} by {2}`, `No contract yet`, `Trump` if unused, and `" · coinched"` if unused (French and Dutch).

- [ ] **Step 3: Browser check** (Playwright; server per the README with `docker compose up -d --wait && cargo run -p beelot-server`; French, then English and Dutch once each; 1280 × 800 and 390 × 844; bust the cache for the changed static files):
  - bot Contrée: nothing before the first bid; the badge appears with the first bid's bubble and bounces; a raise moves it; passes after it change nothing; coinche adds "×2"; it stays at the taker's seat through play; the removed lines are gone;
  - classic Belote: round 1 accept and round 2 suit choice give a symbol-only badge (confirm the take arrives as a bare suit); no badge while all pass;
  - a tout atout / sans atout Contrée contract shows its name as text;
  - a new deal starts without a badge; reload mid-auction and mid-play keeps the badge without bouncing;
  - private table: auction badge follows the reveals, then the board; heading shows the variant name; pass-and-play across the hand-off cover;
  - effects Reduced and Off: no bounce; the accessibility snapshot reads each seat's contract once; no console errors.
  Fix anything that differs and re-run the affected check.

- [ ] **Step 4: Run the project checks**: `node --check` on the three scripts; `node --test crates/beelot-server/frontend-tests/*.test.cjs`; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --workspace` (Docker running). Record the counts.

- [ ] **Step 5: Commit** `feat: print the contract only at the bidder's seat (US-099)`.

## Self-review

- **Spec coverage:** pure module and its tests (T1); badge, bounce, effects level, aria-label, translations, running contract through the bubbles (T2); the six render sites and the new-deal reset (T3); removed lines, heading, translations, browser matrix and project checks (T4).
- **Review Focus pinned:** re-render without bounce (the `contractKey` rule, T2; browser reload check, T4); a raise moves the badge and passes keep it (T1 tests); new deal clears it (T3 step 1, T4 check); tout atout / sans atout (T1 test, T4 check); the viewer's own call (T2 step 3, T3 step 2, T4).
- **Consistency:** `contractAfterCall`, `contractFromAuction`, `contractFromBoard`, `showContractBadge`, `shownContract`, `contractKey` and `badge-bounce` are named the same in every task.
