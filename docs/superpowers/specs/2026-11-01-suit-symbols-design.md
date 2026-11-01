# Read the suits at a glance (US-095) — design

## Goal

A player who struggles to tell the suits apart can identify every suit at a
glance. From the backlog (`docs/product-backlog.md`):

- Each suit has a large, bold symbol on the card, much bigger than the rank
  letter beside it.
- The four suits differ by shape, not only by colour, and stay distinct at a
  small size.
- Red and black keep a high contrast against the card face (WCAG AA at least).
- The symbols apply to the hand, the trick, the tutorial and the bidding
  buttons. There is no Settings option: one size, and the usual red and black
  (an earlier "Large suit symbols" option and a four-colour option were dropped
  after reviewing mock-ups).
- The suit names stay available to screen readers (US-074).
- New texts are translated into French and Dutch.

The frontend only (`crates/beelot-server/static/`): no server change and no new
dependency. It builds on the cards and tokens of US-086 and the themes of
US-087.

## Decisions

- **Plain solid shapes** (style B of the mock-ups): a round heart, a rhombus
  diamond, a pointed spade and a three-lobed club, no outline. They are drawn
  once, in a sprite, `static/suits.svg`, with ids `suit-hearts`,
  `suit-diamonds`, `suit-spades` and `suit-clubs`, each filled with
  `currentColor`.
- **One corner only.** The symbol appears in a single index, under the rank at
  the top left. The upside-down bottom-right index and the centre symbol are
  gone: number cards (7, 8, 9, 10) and aces have a plain cream centre, with no
  pips and no numeral. This replaces the backlog wording "in the corner indices
  as well as the centre", which is updated. Court cards keep their portraits.
  Rejected: a big centre numeral and a left-edge strip layout (they collided
  with the symbols at 72 px), and larger cards (they affect the whole table
  layout).
- **One size, relative to the card.** `.playing-card` becomes a size container
  (`container-type: inline-size`) and the index is sized in `cqw`, so every card
  size (hand, trick, small screens, tutorial) scales the same way: symbol about
  40cqw wide, rank about 24cqw. There is no "Large suit symbols" option: a
  larger size covered too much of the court portraits, and the one size is
  already much bigger than the old glyph.
- **Red and black, as today.** Hearts and diamonds use `--card-red`, spades and
  clubs use `--card-black` (set by the `.playing-card` colour rule, which the
  symbols inherit through `currentColor`). Both already pass 4.5:1 against
  `--card-paper` in every theme (verified in the browser). Rejected: a
  four-colour option (blue diamonds, green clubs).
- **Court portraits make room.** On court cards the portrait is about 76% of the
  card width, anchored to the bottom-right (`right: 2%; bottom: 0`, bottom edge
  still cropped), instead of the 88% inset of US-086, so it never touches the
  rank and symbol at the top left.
- **One shared function.** `suitSymbol(suit)` in `app.js` returns the
  `<svg class="suit suit-<suit>"><use href="/suits.svg#suit-<suit>"/></svg>`
  element for a suit name such as `"HEARTS"`. Cards and the bidding buttons use
  it, and US-098 and US-099 will reuse it.
- **No new setting.** Nothing is added to Settings, to `applySettings()` or to
  browser storage, and there is no new text to translate except the changed
  suit-name keys below.
- **Bidding buttons.** The trump buttons in the solo, private and pass-and-play
  bidding show the symbol, then the name, in place of the font glyph ("Clubs ♣"
  becomes the symbol and "Clubs"). The suit name stays as text.
- **Accessibility.** The symbol is `aria-hidden`: the card's existing
  `aria-label` and the button text already name the suit (US-074). The shape
  carries the suit for sighted players, so colour is never the only cue.

## What changes

### Cards (`cardElement` in `app.js`, `.playing-card` rules in `styles.css`)

- The top-left index is the rank above `suitSymbol(card.suit)`; the font glyph
  from `card.symbol` is no longer drawn, and the rotated bottom-right index is
  removed (with its CSS).
- The pip layouts, the ace's centre symbol and the `.pip-field`, `.card-pip`
  and `.ace-face` rules are removed. Court cards keep the portrait (US-086) and
  the paper halo behind the index.
- The suit colour stays the existing red/black rule on `.playing-card`; the
  symbol takes it through `currentColor`.
- `.court-art` is resized and moved as described above; the index keeps its
  paper halo only if the portrait still reaches it (check in the browser).

### Bidding buttons (`index.html`, `i18n.js`, `i18n-nl.js`)

- The static buttons (`data-suit` buttons in the solo, private and
  pass-and-play bidding) get the sprite symbol in front of the name. The i18n
  keys "Clubs ♣", "Diamonds ♦", "Hearts ♥" and "Spades ♠" are replaced by
  "Clubs", "Diamonds", "Hearts" and "Spades" (checked against existing keys
  first).

## Testing

A Node test `suits-sprite.test.cjs` (local, like the other frontend tests)
checks that `suits.svg` has exactly the four symbols with the expected ids, each
with `viewBox="0 0 24 24"`, filled with `currentColor`, with no text, images or
external references, and that the four shapes differ. The rest is verified in a
real browser:

1. All 32 faces rendered through `cardElement()` at hand, trick and tutorial
   sizes, in red and black and in greyscale: every suit identifiable, no
   collision between rank, symbol and portrait, court portraits still
   recognisable.
2. Contrast of red and of black against the card paper at least 4.5:1 in
   every theme (US-087).
3. The symbols look right in every theme (US-087) and effects level (US-094).
4. The trump buttons in the solo, private and pass-and-play bidding show the
   symbol and the name; screen-reader text is unchanged (accessibility tree).
5. A real bot game at 1280 × 800 and 390 × 844 with no clipping or horizontal
   scroll.
6. `node --check` on the changed scripts, the Node tests, and the existing
   `cargo fmt --check`, `cargo clippy` and `cargo test --workspace` unchanged.

## Out of scope

- Suit symbols next to suit names in text, the trump and contract lines and
  hints (US-098), and the contract at the bidder's seat (US-099).
- The decorative glyphs on the home page and the glyphs inside call-bubble text.
- Per-suit portraits or other changes to the court cards.
