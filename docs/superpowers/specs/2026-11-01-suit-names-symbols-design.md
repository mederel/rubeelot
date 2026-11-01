# Show suit symbols next to suit names (US-098) — design

## Goal

Wherever the text names a suit, the suit symbol of US-095 is shown with it, so
a player recognises the suit at a glance. From the backlog
(`docs/product-backlog.md`):

- The symbol comes first: "♠ Spades". This applies to the trump line, the
  contract and the current contract, the bid and trump calls, the hints
  (US-065), the table messages and the tutorial.
- Where a symbol already sits next to a name (a call bubble, a bidding button),
  the name may be dropped when the symbol is enough.
- The symbol is the SVG symbol of US-095 (`suitSymbol()`), not a font glyph.
- One shared function, next to `suitName`, so that every sentence uses the same
  display in English, French and Dutch.
- Screen readers read the suit name once, not the name and the symbol (US-074).
- The suit names stay in the language files; nothing is duplicated.

Frontend only (`crates/beelot-server/static/`): no server change, no new
dependency, no new Settings option, no new translatable text. It builds on
`suitSymbol()` and `suits.svg` of US-095.

## Decisions

- **Strings stay plain.** `suitName()`, `t()`, the `{0}` templates and the
  server-message patterns (`translateServer`) keep producing ordinary text such
  as "Camille bids 80 Clubs." or "Cœur est atout". No marker characters are put
  in strings. Rejected: fragments built at each call site, and invisible
  markers inside strings (more call-site churn, and markers leak into
  `aria-label`s and comparisons).
- **One renderer finds the suit words at display time.** `setSuitText(element,
  text, { symbolOnly = false } = {})` in `app.js` sets the element's content from
  `text`: each suit word becomes the symbol (`suitSymbol()`), then a space, then
  the word as text; everything else stays text. With `symbolOnly`, the word is
  replaced by the symbol and a visually hidden span holding the name. The suit
  words are the localised names for the active language (from
  `locale.suits`) and the English names, matched case-insensitively and only as
  whole words (not preceded or followed by a letter, using `\p{L}`).
- **A pure splitter, so it can be tested.** `splitSuitText(text, words)` in a new
  `static/suit-text.js` returns an array of parts, `string | { suit: "hearts",
  word: "Cœur" }`, where `words` maps each lower-case word to a suit key. The
  file ends with the same `module.exports` guard as `theme.js` and is loaded
  before `app.js`. `suitWordMap()` in `i18n.js`, next to `suitName`, builds the
  map for the active language plus English (replacing the `suitSymbols` and
  `suitTail` tables that decorated call bubbles with font glyphs).
- **Accessible once.** The symbol is `aria-hidden`; the word stays text, so a
  screen reader reads "Hearts are trump" as written. In symbol-only bubbles the
  hidden span carries the name (`.visually-hidden`, added if absent).
- **Call bubbles show the symbol only**, after the bid value: "80 ♣" and "♣" for
  a chosen trump (decided with the user: compact on phone seats). The bubble's
  text for screen readers remains "80 Clubs".
- **Everything else shows symbol and name**, including the seat's table messages,
  hints, the trump and contract lines and the tutorial.
- **Known limit.** A player named after a suit ("Hearts", "Pique") would see a
  symbol in sentences that include their name. Accepted: rare, harmless.

## What changes

### Places that now use `setSuitText` (`app.js`)

| Place | Today | Change |
|---|---|---|
| Table message (`#table-message` and the private equivalents) | `textContent = t(message)` | `setSuitText` |
| Hint reason lines (`#hint-reason`, `#bid-hint-reason`) | `textContent` with `suitName` | `setSuitText` |
| Current contract labels (`#current-contract`, private) | `textContent` | `setSuitText` |
| Trump / contract lines (`#trump-suit`, `#private-trump`, contract summary) | `textContent = suitName(...)` | `setSuitText` |
| Tutorial prompt and feedback | `textContent = t(...)` | `setSuitText` |
| Call bubbles (`callText`) | `t(call)` plus a font glyph | `setSuitText(..., { symbolOnly: true })` |

US-099 removes the contract and trump lines later; they are converted now so
the story stands alone, without extra polish.

### Removed

- The `suitSymbols`, `symbolBySuit` and `suitTail` tables in `i18n.js` and the
  `callText` glyph decoration; `data-red` on call bubbles (set from the glyph)
  is replaced by the suit class on the symbol (`suit-hearts` and
  `suit-diamonds` colours come from the bubble's `currentColor` rule, so the
  bubble keeps its red tint for those suits).
- The comparison `…?.textContent === callText(call)` is replaced by a comparison
  on a `data-call` attribute set to the plain call text.

### Styles (`styles.css`)

- `.suit-inline` (the symbol inside text): `height: 1.05em; width: 1.05em;
  vertical-align: -.15em; margin-right: .2em`. It inherits `currentColor`, so
  red or black suits follow the text colour of the context where that colour is
  defined (cards) and the surrounding text colour elsewhere.
- `.visually-hidden` if the stylesheet does not have it.

## Testing

Node tests (local, like the other frontend tests), `suit-text.test.cjs`, for
`splitSuitText`:

- splits "Hearts are trump" into the suit and the rest, in English, French
  ("Cœur est atout") and Dutch ("Harten is troef");
- matches whole words only ("pique" inside "piquant" and "club" inside "clubhouse"
  are left alone);
- is case-insensitive and keeps the original word;
- handles two suits in one sentence and a suit at the start and the end;
- returns the text unchanged when no suit word occurs.

Browser verification, in English, French and Dutch: each place in the table
above, including a real bot game through bidding (both rounds), a Contrée game
(bid and coinche messages), the hint, the tutorial, and a private table; the
accessibility tree reads each suit name once; call bubbles at 390 px; no console
errors. `node --check` on the changed scripts, the Node tests, and the existing
`cargo fmt --check`, `cargo clippy` and `cargo test --workspace` unchanged.

## Out of scope

- Moving or removing the contract and trump lines, and the bidder's-seat badge
  (US-099); the score bar (US-096).
- The home-page glyphs, the rules page text and the history and statistics
  pages, which are not in the backlog list.
- Changing the suit names or the sentences in the language files.
