# Cartoon table (US-086) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the table and the cards a comic-book look, with 12 unique illustrated court cards, driven by CSS tokens that US-087 can swap.

**Architecture:** A token block on `:root` feeds the restyled table, seats, cards and a CSS-drawn card back. The twelve portraits live in one SVG sprite, `static/cards.svg`, referenced from each court card with `<use>`; the sprite's colours are `var()` references so tokens pass through. The art is reviewed as a gallery before it is wired into the game.

**Tech Stack:** Vanilla JS/CSS and one SVG file in `crates/beelot-server/static/`; Node (local only) for a structural test of the sprite; the browser tools for visual checks.

**Spec:** `docs/superpowers/specs/2026-10-31-cartoon-table-design.md`

## Global Constraints

- Direction: comic book. Ink outlines (`--outline-width: 3px`), hard offset shadows (`--shadow-hard: 4px 4px 0 var(--card-ink)`), halftone-dot felt, chunky purple rim with an ink outer outline, tilted seat stickers. No soft blurred shadows on table or cards.
- Twelve unique portraits, ids `court-<J|Q|K>-<clubs|diamonds|hearts|spades>`, one personality per suit: hearts romantic, spades warriors, clubs adventurers, diamonds merchants; jack, queen and king differ by crown, hair and pose.
- Every portrait is a `<symbol viewBox="0 0 100 130">`, contains no `<text>`, no external reference and no raster image, and stays readable at 54 × 78 px.
- Suits stay distinguishable without colour: corner rank and suit symbol are kept on every card.
- The existing oval table shape and layout do not change; the table stays fully visible at 1280 × 800 and 390 × 844 with no clipping or scrolling.
- Tokens (names are the contract with US-087): `--card-ink`, `--outline-width`, `--radius-card`, `--radius-chip`, `--shadow-hard`, `--felt`, `--felt-dot`, `--rim`, `--rim-edge`, `--card-paper`, `--card-red`, `--card-black`, `--card-back-a`, `--card-back-b`. Rank/symbol contrast against `--card-paper` is at least 4.5:1.
- No new animation; existing animations keep following the effects level of US-094 (`body[data-effects]`).
- No server change, no new dependency. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace` stay green. FR/NL texts are unaffected (no new visible text).

## Review Focus

- A card face drawn from the sprite when `/cards.svg` fails to load or is slow: the card must still show its corner rank and suit and keep its size (no layout jump, no blank card).
- Dark rim and ink outline colours at the 390 px width: the thicker outline plus rim must not push the oval table past the viewport (no horizontal scroll).
- Highlights (legal, hint, winning card) stay visible against the new outline and are not hidden by `overflow: hidden` on the card.
- Hidden hands and the deal packets (`.card-back` at 48 × 68 and 58 × 84 px) still read as card backs at every size the existing media queries set.
- `<use>` colour inheritance: portraits must pick up `--card-red`/`--card-black` per suit from the card, not a fixed colour.

---

### Task 1: The sprite and its structural test

**Files:**
- Create: `crates/beelot-server/static/cards.svg`
- Create: `crates/beelot-server/frontend-tests/cards-sprite.test.cjs`
- Create (scratch, not committed): `.superpowers/brainstorm/<session>/content/court-gallery.html`

**Interfaces:**
- Produces: sprite symbols `court-<rank>-<suit>` for rank in `J Q K`, suit in `clubs diamonds hearts spades`, served at `/cards.svg`. Colours used inside are `var(--card-red, #d62839)`, `var(--card-black, #1b1b2f)`, `var(--ink, #111)`, `var(--card-paper, #fff8e6)` plus a fixed accent palette (skin, gold, and per-suit accents) declared once as the `ACCENTS` list in the test.

- [ ] **Step 1: Write the failing test** `cards-sprite.test.cjs` (reads `../static/cards.svg` as text, no XML dependency; split on `<symbol`):
  - `has exactly twelve court symbols with the expected ids` (assert the sorted list of ids equals the 12 `court-<rank>-<suit>` ids and no other symbol exists).
  - `every symbol has viewBox "0 0 100 130"`.
  - `no symbol contains text, images or external references` (assert absence of `<text`, `<image`, `href="http`, `xlink:href="http`, `<script`).
  - `every colour is a token or an allowed accent` (collect every `fill`/`stroke` value; each must be `none`, `currentColor`, a `var(--...)`, or one of the hex codes in `ACCENTS`).
  - `the twelve portraits are all different` (the markup of no two symbols is identical).
- [ ] **Step 2: Run** `node --test crates/beelot-server/frontend-tests/cards-sprite.test.cjs` — Expected: FAIL (`ENOENT … cards.svg`).
- [ ] **Step 3: Draw `cards.svg`**: root `<svg xmlns="http://www.w3.org/2000/svg" style="display:none">` holding the 12 symbols. Each portrait: half-length figure on the card face, 3-unit ink outline (`stroke="var(--card-ink,#111)" stroke-width="3" stroke-linejoin="round"`), flat fills, a crown or hat, hair and one suit prop. Cast: hearts (romantic: heart-shaped crown, rose, flowing hair), spades (warriors: horned or plumed helmet, sword/shield), clubs (adventurers: feathered cap, compass/torch, clover charm), diamonds (merchants: gem crown, coin purse, gem prop). Jack = youngest, hat/cap; queen = long hair, tiara; king = beard, full crown. Suit colour of the cast follows `--card-red` (hearts, diamonds) and `--card-black` (spades, clubs) for the main garment.
- [ ] **Step 4: Run the test** — Expected: PASS (5 tests).
- [ ] **Step 5: Gallery review (spec-mandated gate).** Write a scratch `court-gallery.html` in the brainstorm `content/` folder (never committed) that inlines `cards.svg` and shows all 12 portraits on a cream card at 54 × 78, 72 × 104 and 144 × 208 px, each suit's colour tokens applied, plus a greyscale row. Confirm the brainstorm server is alive (restart with the same `--project-dir` if not), give the user the full URL, and **stop for their review**; revise the sprite until they approve. This is the one planned human gate; record the approval in the ledger.
- [ ] **Step 6: Commit** `feat: draw the twelve comic court portraits (US-086)` (sprite and test only).

### Task 2: Tokens, table and seat stickers

**Files:**
- Modify: `crates/beelot-server/static/styles.css` (`:root` block; `.card-table` and its `::before`/`::after` at `:361-400`; seat stations and call badges around `:440-510`)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: the token block named in Global Constraints, declared on `:root` with the comic defaults (`--card-ink: #111`, `--felt: #1f9d8a`, `--felt-dot: rgba(0,0,0,.12)`, `--rim: #5b2b82`, `--rim-edge: var(--card-ink)`, `--card-paper: #fffdf2`, `--card-red: #d62839`, `--card-black: #1b1b2f`, `--card-back-a: #5b2b82`, `--card-back-b: #ffd23f`, `--radius-card: 7px`, `--radius-chip: 999px`, `--outline-width: 3px`, `--shadow-hard: 4px 4px 0 var(--card-ink)`). The existing `--paper`, `--red`, `--black-card` stay for non-table UI.

- [ ] **Step 1: Add the token block** to `:root`; leave every existing token in place.
- [ ] **Step 2: Restyle `.card-table`** onto the tokens: halftone felt (`radial-gradient(var(--felt-dot) 1.4px, transparent 1.6px) 0 0 / 9px 9px` over `var(--felt)`), `border: 10px solid var(--rim)`, `box-shadow: 0 0 0 4px var(--rim-edge), 8px 10px 0 var(--card-ink)`; keep `aspect-ratio`, `border-radius`, `min-height`, `overflow` and `isolation`. Redraw `::before` (inner line) and `::after` (the BEELOT watermark) in token colours at low opacity.
- [ ] **Step 3: Restyle seat labels and call badges** as stickers: `border: var(--outline-width) solid var(--card-ink)`, `box-shadow: var(--shadow-hard)` (smaller offset on badges is allowed if it clips), `border-radius: var(--radius-chip)`, a slight tilt on the seat label via `rotate`; the active/deciding seat keeps its gold marker, now drawn with an outline ring (`box-shadow` ring of `--gold` outside the ink outline). Do not touch the Reduced/Off rules of US-094 (`body[data-effects=…]` blocks) except to confirm their selectors still match.
- [ ] **Step 4: Verify in the browser** (start the app: `docker compose up -d --wait && cargo run -p beelot-server`; hard-reload `styles.css`, the browser caches it): bot game at 1280 × 800 and 390 × 844 — whole oval, seats, call badges and the hand visible, no horizontal scroll (`document.documentElement.scrollWidth <= innerWidth`).
- [ ] **Step 5: Commit** `feat: give the table a comic look with design tokens (US-086)`.

### Task 3: Cards, court art and card back

**Files:**
- Modify: `crates/beelot-server/static/styles.css` (`.playing-card` `:711-800`, `.card-back` `:573-590`, `.deal-packet .card-back` `:1031`, small-screen overrides `:1203-1208`, `:1342`)
- Modify: `crates/beelot-server/static/app.js` (`cardElement` `:1101-1160`; remove the `courtIcons` object)

**Interfaces:**
- Consumes: sprite ids from Task 1; tokens from Task 2.
- Produces: `cardElement(card)` unchanged in signature and in the classes/dataset it already sets (`playing-card <suit>`, `data-rank`, `data-card`, `aria-label`); court faces now contain `<svg class="court-art" aria-hidden="true"><use href="/cards.svg#court-<rank>-<suit lowercase>"/></svg>` inside `.card-face.court-face.court-<suit>`; `.court-figure` and the Unicode icons are removed.

- [ ] **Step 1: Restyle `.playing-card`**: `background: var(--card-paper)`, `border: var(--outline-width) solid var(--card-ink)`, `border-radius: var(--radius-card)`, `box-shadow: var(--shadow-hard)`, colours `var(--card-red)` for hearts/diamonds and `var(--card-black)` otherwise. Keep size, `overflow`, `transition`. Redraw `.card-pip` and `.ace-face` bolder (`font-weight: 900`, slightly larger) so they read at 58 × 84 px.
- [ ] **Step 2: Wire the court art**: in `cardElement`, replace the `figure`/`courtIcons` branch with the `<svg class="court-art"><use …></svg>` (create it with `document.createElementNS("http://www.w3.org/2000/svg", …)`; set `href` through `setAttribute("href", …)`). `.court-art { width: 100%; height: 100%; display: block; }`; remove `.court-figure` rules and the `:1208` override. Keep the corner `card-index` elements so a card whose sprite fails to load still shows rank and suit.
- [ ] **Step 3: Highlights**: redraw `.legal-card`, `.hint-card`, the hover state and `.winning-card` so the gold ring sits outside the ink outline (`box-shadow: 0 0 0 var(--outline-width) var(--card-ink), 0 0 0 calc(var(--outline-width) * 2) var(--gold), var(--shadow-hard)`), still visible with `overflow: hidden` on the card.
- [ ] **Step 4: Card back in CSS from tokens** (`.card-back`): a `repeating-conic-gradient` starburst between `--card-back-a` and `--card-back-b`, `border: var(--outline-width) solid var(--card-ink)`, `--radius-card`, `--shadow-hard` (reduced offset on the 48 × 68 packet), and a bold "B" in an `::after` roundel. Ruling recorded here: the spec's sprite `card-back` symbol is dropped, because a `background-image` cannot target a `<symbol>` fragment and tokens do not pass into image backgrounds; the CSS back uses the same tokens and needs no JS change. Update the spec's Sprite section and the sprite bullet in "Decisions" accordingly in this task's commit.
- [ ] **Step 5: Verify**: `node --check crates/beelot-server/static/app.js`. In the browser, evaluate on a real game page: render all 32 faces through the global `cardElement` into a scratch container (ranks `7 8 9 10 J Q K A` × four suits), screenshot at 1× and with `filter: grayscale(1)`; confirm each suit is identifiable and each court portrait loads (no empty `<use>`: `getBoundingClientRect` of each `.court-art` has non-zero size and its sprite resolves). Block `/cards.svg` (override `fetch`/use a bad id) and confirm a court card still shows its rank and suit at full size. Compute the contrast of `--card-red` and `--card-black` against `--card-paper` and confirm at least 4.5:1.
- [ ] **Step 6: Commit** `feat: illustrated court cards and a comic card back (US-086)`.

### Task 4: Cross-table, effects and wrap-up checks

**Files:**
- Modify: `docs/product-backlog.md` (note US-086 delivered in its entry, if the file marks delivery), `README.md` only if it describes the table look.

- [ ] **Step 1: Browser sweep** at 1280 × 800 and 390 × 844: bot table through bidding, a trick and its collection; a private table; pass-and-play (`/play/local`); the rules page if it shows cards. Check opponents' hidden hands, the upturned card, deal packets, hint highlight and winning card. Repeat once at each effects level (set `localStorage["beelot.effects"]`, hard reload): identical static look, no leftover motion at Off.
- [ ] **Step 2: Run** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running), `node --test crates/beelot-server/frontend-tests/effects.test.cjs crates/beelot-server/frontend-tests/cards-sprite.test.cjs` — Expected: all green; capture and report the counts.
- [ ] **Step 3: Commit** `docs: mark US-086 delivered`; report what was run and what was not (CPU-throttled performance, Dutch/French checks are not needed because no text changed).
