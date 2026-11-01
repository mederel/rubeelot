# Suit Symbols Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the font suit glyphs on the cards and the trump bidding controls with large, bold, shape-distinct SVG symbols, drawn once in a sprite.

**Architecture:** A new `static/suits.svg` sprite (four `currentColor` symbols) is referenced through `<use>`. `cardElement()` builds a single top-left index (rank above a `suitSymbol()` element); pips, the ace glyph and the bottom-right index are removed. Sizes are in `cqw` of the card (`container-type: inline-size`), so every card size scales alike. Court portraits shrink and move bottom-right. The static trump buttons and radio labels carry the same symbol.

**Tech Stack:** vanilla JS/CSS/SVG in `crates/beelot-server/static/`; local Node tests (`node --test <files>`); Playwright MCP for browser checks.

**Spec:** `docs/superpowers/specs/2026-11-01-suit-symbols-design.md`

## Global Constraints

- Frontend only: no server change, no new dependency, no new Settings option, no new storage key.
- Suit colours stay red (`--card-red`: hearts, diamonds) and black (`--card-black`: spades, clubs); symbols take them through `currentColor`.
- One symbol size: symbol 40cqw wide, rank font 24cqw. Court portrait: `right: 2%; bottom: 0; width: 76%; height: 76%`, bottom edge cropped.
- Symbols are `aria-hidden="true"`; the card `aria-label` and the button text name the suit (US-074).
- Sprite ids: `suit-hearts`, `suit-diamonds`, `suit-spades`, `suit-clubs`; each `viewBox="0 0 24 24"`, filled `currentColor`, no text, no external references.
- Out of scope: suit symbols in text lines (US-098), the contract at the bidder's seat (US-099), home-page glyphs, call-bubble text.
- AGENTS.md: run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running) before finishing; report what ran.

## Review Focus

- A "10" index (two digits) at 24cqw must not clip or collide with the symbol on the narrowest card (46 px trick card, 58 px hand card).
- The portrait must not touch the index on any court card at 46, 58 and 72 px wide.
- The upturned card in bidding, the announcement cards (`zoom: .5`) and the tutorial cards must render the new index (they all go through `cardElement`).
- Legal, hint and winning highlights and the card back are unchanged.
- The accessibility tree names each card once (its `aria-label`), with no stray symbol text, and each trump button reads "Clubs", "Diamonds", etc.

---

### Task 1: The suits sprite

**Files:**
- Create: `crates/beelot-server/static/suits.svg`
- Create: `crates/beelot-server/frontend-tests/suits-sprite.test.cjs`

**Interfaces:**
- Produces: `/suits.svg#suit-hearts|suit-diamonds|suit-spades|suit-clubs`, each a `<symbol viewBox="0 0 24 24">` painted with `currentColor`.

- [ ] **Step 1: Write the failing test** `suits-sprite.test.cjs`, in the style of `cards-sprite.test.cjs` (read `static/suits.svg`, parse `<symbol>` elements with a regex). Tests, each one behaviour:
  - `has exactly the four suit symbols with the expected ids` (sorted ids equal `suit-clubs, suit-diamonds, suit-hearts, suit-spades`).
  - `every symbol uses a 24 by 24 viewBox`.
  - `every symbol paints with currentColor and no fixed colour` (body contains `currentColor`; no `#hex` or `rgb(`).
  - `has no text, images or external references` (no `<text`, `<image`, `<script`, `href="http`, `url(http`).
  - `the four shapes differ` (the bodies are pairwise distinct).

- [ ] **Step 2: Run** `node --test crates/beelot-server/frontend-tests/suits-sprite.test.cjs`. Expected: FAIL (ENOENT on `suits.svg`).

- [ ] **Step 3: Create `static/suits.svg`**: `<svg xmlns="http://www.w3.org/2000/svg" style="display:none">` containing four symbols. Paths (from the approved mock-up), each `fill="currentColor"`:
  - hearts: `M12 21.5 C4 14.5 1.8 10 1.8 7 C1.8 4.2 4 2.4 6.5 2.4 C8.8 2.4 10.8 3.8 12 6.2 C13.2 3.8 15.2 2.4 17.5 2.4 C20 2.4 22.2 4.2 22.2 7 C22.2 10 20 14.5 12 21.5Z`
  - diamonds: `M12 1.2 L21.6 12 L12 22.8 L2.4 12Z`
  - spades: `M12 1.5 C12 1.5 2.6 8.6 2.6 14.2 C2.6 17.3 5 19.2 7.8 19.2 C9.4 19.2 10.8 18.5 11.5 17.4 C11.3 20 10.4 21.5 8.2 22.6 H15.8 C13.6 21.5 12.7 20 12.5 17.4 C13.2 18.5 14.6 19.2 16.2 19.2 C19 19.2 21.4 17.3 21.4 14.2 C21.4 8.6 12 1.5 12 1.5Z`
  - clubs: `<g fill="currentColor">` with circles `(12,6.6,r5)`, `(6,14.2,r5)`, `(18,14.2,r5)` and path `M12 11 C12 17 11.2 20.2 8.4 22.6 H15.6 C12.8 20.2 12 17 12 11Z`.

- [ ] **Step 4: Run the test.** Expected: 5/5 PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/beelot-server/static/suits.svg crates/beelot-server/frontend-tests/suits-sprite.test.cjs
git commit -m "feat: add the suit symbols sprite (US-095)"
```
(end the message with the `Co-Authored-By` line from the session's attribution reminder).

### Task 2: Cards with the new index

**Files:**
- Modify: `crates/beelot-server/static/app.js` (`cardElement`, ~line 1101; add `suitSymbol`)
- Modify: `crates/beelot-server/static/styles.css` (`.playing-card` ~741; index/face/pip/ace rules ~761–799; media queries at ~1213–1218, ~1378–1384, ~1393–1400, ~1528)

**Interfaces:**
- Produces: `suitSymbol(suit: string): SVGSVGElement` in `app.js`: `suit` is a server suit name such as `"HEARTS"`; returns `<svg class="suit suit-hearts" aria-hidden="true"><use href="/suits.svg#suit-hearts"/></svg>` (built with `createElementNS`, `href` set through `setAttribute`). US-098 and US-099 reuse it.
- Produces: `cardElement(card)` output: `.playing-card.<suit>` containing one `.card-index` (a `strong` rank and the `suitSymbol`), plus, for J/Q/K only, a `.card-face.court-face` with the existing `.court-art`.

- [ ] **Step 1: Capture the "before"**: in the browser (server running per README, a bot game started) screenshot the hand and trick at 1280 × 800 for comparison. Expected: pips, ace glyph and both corners visible.

- [ ] **Step 2: Implement `suitSymbol` and rewrite `cardElement`** in `app.js`: keep the class, `aria-label`, `data-rank`, `data-card` lines and the court branch; drop `corner()`, `pipLayouts`, the pip and ace branches and the bottom index. Number cards and aces get no `.card-face` element. Append `[index, face?]`. No use of `card.symbol`.

- [ ] **Step 3: Rewrite the card CSS** in `styles.css`:
  - `.playing-card`: add `container-type: inline-size;`.
  - `.card-index`: `left: 4px; top: 3px; line-height: 1;` (keep flex column, centred, `z-index: 2`); `.card-index strong { font-size: 24cqw; }`; `.card-index .suit { display: block; height: 40cqw; width: 40cqw; }`.
  - `.card-face.court-face { bottom: 0; height: 76%; inset: auto 2% 0 auto; width: 76%; }` (right 2%, bottom 0); keep `.court-art` rule.
  - Remove `.card-index-bottom`, `.pip-field`, `.card-pip`, `.ace-face`, the old `.card-face { inset }` base rule, `.court-card .card-index` halo (restore it only if Step 5 shows an overlap), and every media-query override of those (`.card-face`, `.pip-field`, `.card-pip`, `.ace-face`, `.card-index` and `.card-index strong` font and position rules, `.card-index-bottom`, `.trick-slot .ace-face|.card-pip|.card-index…`). Leave the colour rule `.playing-card.hearts, .playing-card.diamonds { color: var(--card-red) }` as it is.

- [ ] **Step 4: Check syntax**: `node --check crates/beelot-server/static/app.js`. Expected: no output.

- [ ] **Step 5: Verify in a real browser** (cache-bust `styles.css` with `fetch(..., {cache:'reload'})` then reload). Render all 32 faces through `cardElement()` in a scratch container that defines `--ch`/`--cw`, at 72, 58 and 46 px wide. Expected, from Review Focus: every suit identifiable, "10" not clipped or colliding, portraits clear of the index, symbols red/black, greyscale still distinguishes the four shapes. Adjust the `cqw` values only within ±15% of the spec if something collides, and record any change as a ruling.

- [ ] **Step 6: Verify in play**: start a bot game at 1280 × 800 and 390 × 844. Expected: hand, trick, upturned card, announcement cards and tutorial cards use the new index; legal/hint/winning highlights and card backs unchanged; no clipping or horizontal scroll; no console errors.

- [ ] **Step 7: Run all local Node tests** `node --test crates/beelot-server/frontend-tests/*.test.cjs`. Expected: all PASS.

- [ ] **Step 8: Commit** `feat: show large suit symbols in the card index (US-095)` with `app.js` and `styles.css`.

### Task 3: Trump buttons, texts and final checks

**Files:**
- Modify: `crates/beelot-server/static/index.html` (lines ~205–208 trump buttons; ~213 contract-suit radios; ~341 private-contract-suit radios)
- Modify: `crates/beelot-server/static/styles.css` (button/radio symbol sizing)
- Modify: `crates/beelot-server/static/i18n.js` (~101–104), `crates/beelot-server/static/i18n-nl.js` (~89–92)

**Interfaces:**
- Consumes: `/suits.svg` symbols (Task 1).
- Produces: dictionary keys `"Clubs"`, `"Diamonds"`, `"Hearts"`, `"Spades"` (FR: Trèfle, Carreau, Cœur, Pique; NL: Klaveren, Ruiten, Harten, Schoppen), replacing the keys `"Clubs ♣"`, `"Diamonds ♦"`, `"Hearts ♥"`, `"Spades ♠"` in both files.

- [ ] **Step 1: Confirm no other user of the old keys**: `grep -rn '"Clubs ♣"\|Clubs ♣' crates/beelot-server/static` shows only `index.html` and the two language files (and the 8 radio labels). Expected: nothing else.

- [ ] **Step 2: Edit the markup**: in each of the four `data-suit` buttons and the eight suit radio labels (`contract-suit` and `private-contract-suit`, Clubs/Diamonds/Hearts/Spades only; All trumps and No trumps unchanged) replace the glyph with `<svg class="suit" aria-hidden="true"><use href="/suits.svg#suit-<suit>"/></svg>` placed before the plain name, e.g. `<button type="button" data-suit="CLUBS"><svg …/> Clubs</button>`.

- [ ] **Step 3: Replace the four dictionary keys** in `i18n.js` and `i18n-nl.js` as in Interfaces. `translateStaticMarkup()` already translates text nodes by trimmed text, so the name next to the svg is translated.

- [ ] **Step 4: Style**: `.trump-options .suit, .suit-choice .suit { height: 1.1em; vertical-align: -.2em; width: 1.1em; }` (buttons and radios are inline; the red radios keep their `.red-suit` colours through `currentColor`). In Contrée the radio label for a hearts/diamonds suit stays red as before.

- [ ] **Step 5: Verify in the browser** in English, French (`beelot.language`) and Dutch: the second-round trump buttons (solo classic), the Contrée contract radios (solo) and the private-table radios each show symbol + translated name; disabled state (the upturned card's suit) still dims; selected radios keep their colour. Read the accessibility snapshot: buttons read "Clubs", "Diamonds"… once, cards are named once by `aria-label`. Expected: matches Review Focus.

- [ ] **Step 6: Run the project checks**: `node --check` on `app.js`, `i18n.js`, `i18n-nl.js`; `node --test crates/beelot-server/frontend-tests/*.test.cjs`; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --workspace` (Docker running). Expected: all pass; record counts.

- [ ] **Step 7: Commit** `feat: show the suit symbols on the trump buttons (US-095)`.

## Self-review

- **Spec coverage:** sprite (T1); one top-left index, no pips/ace/bottom index, cqw sizes, portrait placement, `suitSymbol` (T2); colours unchanged, no Settings (global constraints, nothing to build); bidding buttons, texts (T3); testing list (T1 test, T2 steps 5–7, T3 steps 5–6). Pass-and-play uses the same markup as the solo bidding, so the T3 markup covers it; confirm it in T3 Step 5.
- **Consistency:** `suitSymbol` name, `.suit` class, sprite ids and `cqw` values are the same in every task.
