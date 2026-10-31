# Control the effects (US-094) — design

## Goal

A player can turn the motion and visual effects of the table down. From the
backlog (`docs/product-backlog.md`):

- Settings offers an effects level: Full, Reduced or Off, separate from the
  sound setting.
- The default follows `prefers-reduced-motion`.
- At Reduced, movement is replaced by short fades; at Off, state changes are
  instant.
- Every effect of US-086 to US-093 honours the level.
- The animations stay smooth on a mid-range phone.
- New texts are translated into French and Dutch.

US-094 is built first so that the cartoon effects of US-086 to US-093 are
written against the level from the start. The frontend only (`static/`): no
server change and no new dependency.

## Decisions

- **One source of truth: `data-effects` on `<body>`.** Its value is `full`,
  `reduced` or `off`. CSS selects on it; JavaScript reads it through a single
  `effectsLevel()` helper. The old `reduced-motion` body class goes away.
- **The stored choice is separate from the resolved level.** `localStorage`
  key `beelot.effects` holds `full`, `reduced` or `off` once the player has
  chosen. When the key is absent or invalid, the level follows
  `prefers-reduced-motion` (Reduced if set, otherwise Full) and keeps following
  it if the system setting changes while the page is open.
- **Migration of the old checkbox.** If `beelot.effects` is absent and
  `beelot.reduced-motion` is `"true"`, the level is Reduced. The old key is
  never written again and is removed when the player picks a level.
- **Settings control.** The "Reduce motion" checkbox becomes a three-option
  radio group in the same style as Language (`choice-group`), labelled
  "Effects", with the options Full, Reduced and Off. It sits next to, and is
  independent of, "Sound effects".
- **Pacing is not an effect.** The `setTimeout` delays that pace the game (bot
  turns, how long a finished trick stays on view, the call reveals) are
  unchanged at every level. Only decoration and movement change.

## What each level does

| Kind of effect | Full | Reduced | Off |
| --- | --- | --- | --- |
| Movement (card deal, card drop, trick collected, hand slide-in) | as today | replaced by a fade of at most 150 ms | none, state changes at once |
| Pop, shake, ripple and bounce (call badges, reactions) | as today | fade only | none |
| Looping decoration (floating hero suits, thinking dots, "deciding" glow, winning-card pulse, live dot) | as today | stopped; a static state shows the same information | stopped |
| Hover and focus transitions | as today | at most 150 ms | none |

Information carried by an animation is always kept in a static form: the
active player stays marked, a call badge simply appears, a trick that is
collected simply disappears.

### CSS

- Each existing keyframe family gets an explicit rule per level under
  `body[data-effects="reduced"]` and `body[data-effects="off"]`, instead of the
  current blanket `animation-duration: .01ms !important` override. Reduced maps
  each movement keyframe to a shared `fade-in` keyframe.
- Effects are authored against custom properties
  (`--effect-fast`, `--effect-medium`) so that US-086 to US-093 add their
  animations through the same switch: a new animation must use one of the
  documented patterns below, and a review of any new keyframe checks all three
  levels.
- Patterns for later stories: a one-shot movement uses a `translate`/`scale`
  keyframe and gets its Reduced and Off rules; a loop is wrapped in
  `body[data-effects="full"]`.

### JavaScript

`motionReduced()` (`app.js:1362`) becomes `effectsLevel() !== "full"`. Its
current callers all skip movement-only choreography (the card deal, the
second deal, the trick-collect), so they behave the same at Reduced and Off.
At Reduced the fade is done by CSS on the cards that appear; the choreography
that moves cards across the table is not run.

`applySettings()` sets `document.body.dataset.effects`, checks the matching
radio and listens for the `prefers-reduced-motion` media query so an unset
choice stays in step with the system.

## Texts

New texts in `i18n.js` (French) and `i18n-nl.js` (Dutch): "Effects", "Full",
"Reduced", "Off" and the description under the setting. Because "Full",
"Reduced" and "Off" are common words, they are checked against existing keys
before adding. "Reduce motion" and its description are removed.

## Testing

The frontend has no automated tests, so the story is verified in a real
browser, in English, French and Dutch:

1. Default level with and without `prefers-reduced-motion` (emulated).
2. Switching levels in Settings applies at once, persists after reload, and is
   independent of the sound setting.
3. A game at each level, from the bot table: the deal, a bid with a call
   badge, a played trick and its collection, a hint, a finished round. At
   Full everything moves as today; at Reduced only fades; at Off nothing moves.
4. Migration: with only `beelot.reduced-motion=true` stored, the level is
   Reduced and the radio shows it.
5. Private and pass-and-play tables follow the same level.
6. `node --check` on the changed scripts, and the existing
   `cargo fmt --check`, `cargo clippy` and `cargo test --workspace` unchanged.

Smoothness on a mid-range phone is checked by emulating CPU throttling in
the browser tooling; only `transform`, `translate`, `scale` and `opacity` are
animated.

## Out of scope

- The cartoon effects themselves (US-086 to US-093); they are specified
  separately and must follow the patterns above.
- Storing the choice on the server or in an account: like the language and
  sound settings, it stays in the browser.
- A separate setting for each effect.
