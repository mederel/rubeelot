# Give the table a cartoon look (US-086) — design

## Goal

The table and the cards look like a bold comic-book game the moment it opens.
From the backlog (`docs/product-backlog.md`):

- The table has a textured felt, a chunky wooden rim, soft shadows and
  rounded, outlined shapes.
- Cards have illustrated court cards (jack, queen, king) with outlines and
  clean pips, and a designed card back.
- Suits stay distinguishable without colour alone.
- The colours, outlines, radii and shadows are CSS custom properties, so that
  US-087 can swap them.
- The table stays fully visible on a laptop screen (US-039) and on a phone.

The frontend only (`crates/beelot-server/static/`): no server change and no
new dependency. It builds on the effects level of US-094 (PR #13).

## Decisions

- **Direction: comic book**, chosen from three sketches. Halftone-dot felt,
  black ink outlines, hard offset shadows, tilted seat stickers, angular
  accents. The backlog wording "soft shadows" and "wooden rim" gives way to
  this choice: shadows are hard and offset, and the rim is a chunky painted
  rim (purple, as sketched), not wood. The backlog story is updated to match.
- **Twelve unique court portraits**, as comic royals with one personality per
  suit: hearts romantic, spades warriors, clubs adventurers, diamonds
  merchants. Each suit has its own props; the jack, queen and king differ by
  crown, hair and pose. Every portrait stays readable at the hand size
  (about 54 × 78 px) and at the larger trick size.
- **One SVG sprite, `static/cards.svg`.** It holds 12 `<symbol>`s
  (`court-<rank>-<suit>`, for example `court-K-hearts`) and the card back
  (`card-back`). Cards reference them with `<use href="/cards.svg#...">`.
  Colours inside the sprite are `var(--...)` references with fallbacks, which
  pass through `<use>`, so US-087 can re-skin the art without editing it.
- **The current oval table stays.** Its shape and layout do not change.
- **Static look, no new animation.** Existing animations keep following the
  effects level of US-094; this story adds none.
- **The art is reviewed before it is wired in.** The first task draws all 12
  portraits and the card back as a gallery page in the browser; the person
  reviewing it approves or asks for changes before any of it reaches the game.

## Design tokens

Declared on `:root` in `styles.css`, used by every table and card rule. The
names are the contract with US-087, which overrides them per theme.

| Token | Role | Default |
| --- | --- | --- |
| `--ink` (existing, reused) | outlines and hard shadows | near-black |
| `--outline-width` | outline of cards, seats, controls on the table | `3px` |
| `--radius-card`, `--radius-chip` | corner radii of cards and seat stickers | `7px`, `999px` |
| `--shadow-hard` | offset shadow of cards and stickers | `4px 4px 0 var(--ink)` |
| `--felt`, `--felt-dot` | felt colour and halftone dot colour | teal, translucent black |
| `--rim`, `--rim-edge` | table rim and its outer outline | purple, `var(--ink)` |
| `--card-paper`, `--card-red`, `--card-black` | card face and suit colours | cream, red, ink |
| `--card-back-a`, `--card-back-b` | the two colours of the card back | purple, gold |

Existing tokens (`--felt`, `--felt-dark`, `--paper`, `--red`, `--black-card`)
are folded into this set rather than duplicated; any rule that used a raw
colour for the table or a card is moved onto a token.

## What changes

### Table

- `.card-table`: halftone felt (a radial-gradient dot pattern), a thick rim
  with an ink outer outline and a hard offset shadow, inside the existing
  oval. The `::before`/`::after` decorations are redrawn in the same tokens.
- Seat labels and the call badges become ink-outlined stickers with the hard
  shadow; the active seat keeps its gold marker, drawn with an outline rather
  than only a glow.
- The score bar, buttons and panels outside the table keep their current look
  in this story; they get the outline and radius tokens only where they sit on
  the table.

### Cards

- `.playing-card`: cream paper, `--outline-width` ink outline, `--radius-card`,
  `--shadow-hard`. The corner rank and suit symbol stay (they are what tells
  suits apart without colour); the pips are redrawn as bold, outlined shapes.
- Court cards: the portrait is an `<svg><use href="/cards.svg#court-K-hearts">`
  in the card face. `cardElement()` in `app.js` builds it in place of the
  Unicode `courtIcons`, which are removed.
- Card back: the sprite's `card-back` symbol, used by every hidden hand, the
  upturned stack and the deal packets.
- Legal, hint and winning-card highlights keep their meaning and are redrawn
  to sit on the new outline (gold ring outside the ink outline).

### Sprite

- Each portrait is a self-contained `viewBox` (`0 0 100 130`, the card face
  aspect), drawn with outlines of `var(--outline-width, 3px)`-equivalent
  stroke, flat fills from the card tokens plus a fixed set of skin and
  accent colours, and no text.
- Suits are told apart by the symbol in the corners and by the portrait's own
  props and palette, not only by red against black.

## Testing

The frontend has no automated tests, so the story is verified in a browser:

1. A scratch gallery page (not committed) shows all 12 portraits and the card
   back at hand size, trick size and 2× size. The reviewer approves it before
   integration.
2. A bot game, a private table and pass-and-play at 1280 × 800 and 390 × 844:
   the whole table, the hand, the trick and the opponents' hidden hands are
   visible with no clipping or scrolling.
3. A scratch page renders all 32 card faces through `cardElement()`; each
   suit is identifiable with `filter: grayscale(1)` applied.
4. Contrast of rank and symbol against the paper is at least 4.5:1.
5. At each effects level (US-094) the table renders the same static look.
6. `node --check` on the changed scripts, and the existing `cargo fmt --check`,
   `cargo clippy` and `cargo test --workspace` unchanged.

## Out of scope

- Switching themes and card backs (US-087); this story only makes them
  possible through the tokens and the sprite.
- Opponent avatars (US-088), card-play motion (US-089), celebrations (US-090),
  score animation (US-091), the home screen (US-092), the match end (US-093).
- Large, shape-distinct suit symbols and the four-colour option (US-095).
- The score bar and the contract badge (US-096, US-099).
