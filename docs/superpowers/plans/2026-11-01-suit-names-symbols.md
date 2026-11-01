# Suit Symbols Next To Suit Names Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the US-095 suit symbol before every suit name in the text (trump and contract lines, bids and trump calls, hints, table messages, tutorial), and symbol-only in the call bubbles.

**Architecture:** Strings stay plain. A pure splitter (`splitSuitText`) finds suit words in a sentence; one renderer (`setSuitText`) turns the parts into text and `suitSymbol()` nodes. The word list for the active language comes from `suitWordMap()` in `i18n.js`.

**Tech Stack:** vanilla JS/CSS in `crates/beelot-server/static/`; local Node tests (`node --test <files>`); Playwright MCP for browser checks.

**Spec:** `docs/superpowers/specs/2026-11-01-suit-names-symbols-design.md`

## Global Constraints

- Frontend only; no server change, no new Settings option, no new translatable text, no new dependency.
- The symbol comes first: `<symbol> <word>`. The symbol is `suitSymbol()` from US-095 (class `suit suit-<name>`, `aria-hidden`), with the extra class `suit-inline`.
- Suit words: the active language's `locale.suits` values plus the English names (`clubs`, `diamonds`, `hearts`, `spades`), matched case-insensitively and only as whole words (not preceded or followed by a letter, `\p{L}`).
- Call bubbles show the symbol only after the bid value ("80 ♣", "♣"), with the name in a `.visually-hidden` span.
- Strings, `suitName()`, `t()` and the server-message patterns are not changed.
- AGENTS.md: run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running) before finishing; report what ran.

## Review Focus

- "Pique" inside "piquant" / "club" inside "clubhouse" stay text (whole words only), also next to accented letters ("trèfle", "cœur").
- A sentence with two suits ("Spades are trump and clubs were led.") shows two symbols; one at the start and one at the end both work.
- Re-rendering the same message twice replaces the content instead of appending.
- The call-bubble "already shown" check still prevents a duplicate bubble for the same call, and hearts/diamonds bubbles keep their red tint.
- The accessibility tree reads each suit name once (no symbol text, no duplicate name), including in symbol-only bubbles.
- Dutch and French sentences with a suit word that is also a common word are not mangled (check "Harten is troef", "Cœur est atout").

---

### Task 1: The pure splitter and the word map

**Files:**
- Create: `crates/beelot-server/static/suit-text.js`
- Create: `crates/beelot-server/frontend-tests/suit-text.test.cjs`
- Modify: `crates/beelot-server/static/i18n.js` (add `suitWordMap`; ~line 622 next to `suitName`)
- Modify: `crates/beelot-server/static/index.html` (add `<script src="/suit-text.js">` before `/app.js`)

**Interfaces:**
- Produces: `splitSuitText(text: string, words: Record<string, string>): Array<string | { suit: string, word: string }>` — `words` maps a lower-case word to a suit key (`"cœur" → "hearts"`). Concatenating the parts (suit parts as their `word`) always reproduces `text`. Adjacent plain text stays in one string part; no empty string parts.
- Produces: `suitWordMap(): Record<string, string>` in `i18n.js` — the active language's suit names plus the English ones, lower-cased, each mapped to `clubs|diamonds|hearts|spades`.

- [ ] **Step 1: Write the failing tests** in `suit-text.test.cjs` (`require("../static/suit-text.js")`). A shared `FR = { "trèfle": "clubs", "carreau": "diamonds", "cœur": "hearts", "pique": "spades" }`, `EN`, `NL` (`klaveren, ruiten, harten, schoppen`) fixtures. One test each:
  - `splits an English sentence`: `splitSuitText("Hearts are trump", EN)` deep-equals `[{ suit: "hearts", word: "Hearts" }, " are trump"]`.
  - `splits French and Dutch sentences`: `"Cœur est atout"` → `[{suit:"hearts",word:"Cœur"}," est atout"]`; `"Harten is troef"` → `[{suit:"hearts",word:"Harten"}," is troef"]`.
  - `matches whole words only`: `"piquant"` with FR and `"clubhouse"` with EN come back as `["piquant"]` / `["clubhouse"]`.
  - `is not fooled by accented neighbours`: `"un trèfle"` → `["un ",{suit:"clubs",word:"trèfle"}]`; `"trèflerie"` stays `["trèflerie"]`.
  - `keeps the original case`: `"CLUBS"` → `[{suit:"clubs",word:"CLUBS"}]`.
  - `handles two suits and edges`: `"Spades are trump and clubs were led."` → five parts, suit at index 0 and at index 3, ending with `" were led."`.
  - `returns plain text unchanged`: `"Your turn"` → `["Your turn"]`; `""` → `[]`.

- [ ] **Step 2: Run** `node --test crates/beelot-server/frontend-tests/suit-text.test.cjs`. Expected: FAIL (module not found).

- [ ] **Step 3: Implement `splitSuitText` in `suit-text.js`** with a single regex built from the words (longest first, regex-escaped), flags `giu`, wrapped in `(?<![\p{L}])(...)(?![\p{L}])`; end the file with `if (typeof module !== "undefined") module.exports = { splitSuitText };`.

- [ ] **Step 4: Run the tests.** Expected: 7/7 PASS.

- [ ] **Step 5: Add `suitWordMap()` to `i18n.js`** next to `suitName` (build from `locale?.suits` and the English list; keys lower-cased) and add the script tag in `index.html` before `/app.js`. `node --check crates/beelot-server/static/i18n.js`.

- [ ] **Step 6: Commit** `feat: add the suit word splitter (US-098)` (end with the session's `Co-Authored-By` line).

### Task 2: The renderer and the text lines

**Files:**
- Modify: `crates/beelot-server/static/app.js` (add `setSuitText`; sites below)
- Modify: `crates/beelot-server/static/styles.css` (`.suit-inline`, `.visually-hidden` if absent)

**Interfaces:**
- Consumes: `splitSuitText`, `suitWordMap()`, `suitSymbol()` (US-095).
- Produces: `setSuitText(element: Element, text: string, options?: { symbolOnly?: boolean }): string[]` — replaces the element's content with the parts of `text` (strings as text nodes; each suit part as `suitSymbol(suit)` with the extra class `suit-inline`, then a space and the word as text, or, with `symbolOnly`, the symbol followed by `<span class="visually-hidden">word</span>`); returns the suit keys found, in order. Re-rendering replaces the content (`replaceChildren`).

- [ ] **Step 1: Check the renderer by hand first** (no DOM test harness exists): in the browser console on a running server, define nothing yet and confirm `suitWordMap()` returns the French map (`cœur` → `hearts`) once Task 1 is loaded. Expected: the four French names plus the four English ones.

- [ ] **Step 2: Implement `setSuitText`** in `app.js` next to `suitSymbol`, and the CSS: `.suit-inline { height: 1.05em; margin-right: .2em; vertical-align: -.15em; width: 1.05em; }`, and `.visually-hidden` (the standard clip pattern) if `styles.css` has none.

- [ ] **Step 3: Switch the display sites to `setSuitText(element, text)`**, keeping the text each site already computes: the game and bidding message elements (`#bidding-message` lines ~1559 and ~1634, `#private-game-message` ~714 and ~734, `#private-table-message` ~330 for server events, the bot-game game message and `#declaration-message` ~962), the hint lines (`line` at ~263, which covers `#hint-reason` and `#bid-hint-reason`), `#current-contract` (~1592) and `#private-current-contract` (~694), `#trump-suit` (~955), `#private-trump` (~743), `#contract-summary` (~953), `#tutorial-prompt` and `#tutorial-feedback` (~378, ~379, ~396). Find any other message element that receives `t(…message…)` with `grep -n 'message").textContent = t(' app.js` and convert those that can contain a suit; leave error messages and non-suit text alone.

- [ ] **Step 4: `node --check`**, then **verify in the browser** in French (default), English and Dutch (`localStorage` key `beelot.language`; bust the cache for `app.js`, `i18n.js`, `i18n-nl.js`, `suit-text.js`, `styles.css`, `index.html`): a bot game through bidding in classic Belote (upturned-suit message, second-round trump choice, accepted-contract message, trump line), a Contrée game (bid and coinche messages, current contract), a hint at bidding, and the tutorial's "Hearts are trump…" and "Spades are trump and clubs were led…" lessons. Expected: symbol then name in each, two symbols in the two-suit sentence, no stray text, no console errors. Read the accessibility snapshot: each suit name once.

- [ ] **Step 5: Commit** `feat: show suit symbols next to suit names in the text (US-098)`.

### Task 3: Call bubbles and cleanup

**Files:**
- Modify: `crates/beelot-server/static/app.js` (`callText` ~1437, `showPlayerCall` ~1453, the seat-call render ~1313, the already-shown check ~1507, `data-red` at ~1316 and ~1466)
- Modify: `crates/beelot-server/static/i18n.js` (remove `suitSymbols`, `symbolBySuit`, `suitTail`, ~597–603)

**Interfaces:**
- Consumes: `setSuitText(element, text, { symbolOnly: true })`.
- Produces: `callText(call: string): string` returns the plain translated call text only (no glyph); bubbles carry `data-call` = that plain text.

- [ ] **Step 1: Make `callText` return `t(call)`** and render every call bubble with `setSuitText(bubble, text, { symbolOnly: true })`, setting `bubble.dataset.call = text`. Set `bubble.dataset.red = suits.some((s) => s === "hearts" || s === "diamonds")` from its return value (replacing the glyph regex at both places). Replace the "already shown" comparisons (`?.textContent === text`, ~1457 and ~1507) with `?.dataset.call === text`. Bubbles keep their existing `aria-label`/live-region behaviour; the hidden span carries the name.

- [ ] **Step 2: Delete the unused glyph tables** in `i18n.js` (`suitSymbols`, `symbolBySuit`, `suitTail`) after confirming with `grep -n 'suitSymbols\|symbolBySuit\|suitTail' crates/beelot-server/static/*.js` that nothing else uses them.

- [ ] **Step 3: Verify in the browser** (French, English, Dutch; 1280 × 800 and 390 × 844): bids and a chosen trump appear as "80 ♣" / "♣", red tint on hearts and diamonds bubbles, the same call is not shown twice, bubbles fit on phone seats, accessibility tree reads "80 Trèfle" once (hidden span), no console errors.

- [ ] **Step 4: Run the project checks**: `node --check` on `app.js`, `i18n.js`, `suit-text.js`; `node --test crates/beelot-server/frontend-tests/*.test.cjs` (expected: all PASS, 26 tests); `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --workspace` (Docker running). Record the counts.

- [ ] **Step 5: Commit** `feat: show the suit symbol in call bubbles (US-098)`.

## Self-review

- **Spec coverage:** splitter and word map (T1); renderer, styles, the six display sites and tutorial (T2); call bubbles, `data-call`, red tint, glyph-table removal (T3); testing list (T1 tests, T2 step 4, T3 steps 3–4).
- **Ruling carried from the spec:** the spec says the red tint comes from the symbol's class; the plan keeps `data-red` on the bubble, computed from the suits `setSuitText` returns, because the existing `.player-call[data-red="true"]` rule already colours the whole bubble and `suit-inline` inherits `currentColor`. Same result, smaller change.
- **Consistency:** `splitSuitText`, `suitWordMap`, `setSuitText`, `suit-inline` and `data-call` are named the same in every task.
