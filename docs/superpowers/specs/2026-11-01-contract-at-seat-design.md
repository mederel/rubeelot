# Show the contract at the bidder's seat (US-099) — design

## Goal

The ongoing bet is printed next to the avatar of the player who holds it, as a
badge, so a player sees at once who plays which contract. From the backlog
(`docs/product-backlog.md`):

- The badge shows the value and the trump suit in Contrée, the trump suit alone
  in classic Belote, with the suit symbol of US-095 and US-098. It bounces when
  it is set or raised.
- During bidding it follows the highest bid and moves to the new bidder when a
  bid is raised. Players who passed show nothing, and nothing shows before the
  first bid.
- Once bidding ends it stays at the taker's seat for the whole round, with a
  mark when the contract is coinched.
- The contract is no longer printed anywhere else: the contract summary line,
  the current contract label and the trump line go, in bot games, at private
  tables and in pass-and-play.
- Screen readers get it as part of the seat: player, value, suit (US-074).
- New texts are translated into French and Dutch.

Frontend only (`crates/beelot-server/static/`): no server change, no new
dependency, no new Settings option. The snapshots already carry what is needed.

## Decisions

- **Existing call bubbles stay.** The speech bubble of each call (`.player-call`,
  "80 ♣", "Pass", "Coinche!") remains the transient reaction to a call; the badge
  is the lasting state. Decided with the user.
- **Surcoinche is not in the game.** No code, snapshot or rule knows it, so the
  mark covers the coinche only (a "×2" mark). The mark is a flag on the contract,
  so a later rule can add a second state without changing the layout.
- **The contract is derived, not stored.** A new pure module computes it from
  the calls shown so far, or from the board once play has started. The server
  sends nothing new. Rejected: a per-seat `contract` field from the server
  (changes the API contract files for no new information).
- **The badge follows the reveal, not the snapshot.** Bots' calls are revealed one
  at a time with a thinking pause (`announceCall`). The badge must not show a bid
  before its bubble does, so it is updated from the same step that shows the
  bubble, and from the calls shown so far when the seats are re-rendered.
- **One place draws the badge.** `showContractBadge(prefix, contract)` puts the
  badge on the holder's seat and removes it from the others. Both
  `renderTableSeats` and the reveal step call it, so the two cannot disagree.

## The pure module: `static/contract-badge.js`

Loaded before `app.js`, ends with the same `module.exports` guard as
`suit-text.js`. A contract is `null` or
`{ player: string, value: number | null, suit: string, coinched: boolean }`
(`value` is `null` in classic Belote; `suit` is kept as the server gives it —
"Hearts", "HEARTS", "All trumps" — and displayed through `suitName()`).

- `contractAfterCall(contract, player, call)`: the contract once `call` by
  `player` is shown. `call` is the server's English text (`call_label` in
  `tables/messages.rs`): `Pass`, `90 Hearts`, `Hearts` for a chosen trump,
  `Coinche!`.
  - `"<value> <suit>"` (a leading integer, then the rest): a new contract for
    `player`, `coinched: false`. Contrée's rules make every bid a raise, so no
    comparison is needed.
  - `"Pass"`: unchanged.
  - `"Coinche!"` (any `^coinche`, as `callKind` matches it): the same contract with `coinched: true`;
    unchanged when there is no contract.
  - anything else (a bare suit, classic Belote's trump call): a contract for
    `player` with `value: null`.
- `contractFromAuction(calls)`: folds `contractAfterCall` over an ordered list of
  `{ playerName, call }` (the snapshot's `bidding.auction`, or its first *n*
  entries while revealing).
- `contractFromBoard(board)`: once play has started, the contract of the board:
  `{ player: board.seats[board.declaringPlayerIndex].name, value:
  board.contractValue (Contrée) or null, suit: board.trump, coinched:
  board.coinched }`.

## What changes

### `app.js`

- `renderTableSeats(prefix, seats, currentPlayerIndex, contract = null)` calls
  `showContractBadge(prefix, contract)` after it rebuilds the seats (it replaces
  every child, so the badge is re-added each time).
- `showContractBadge(prefix, contract)`: for the station whose `playerName` is
  `contract.player`, appends `<span class="contract-badge">` after the dealer chip
  and before the call bubble; every other station loses its badge. Its content is
  `setSuitText(badge, "<value> <suit name>", { symbolOnly: true })`, then a
  `×2` mark when coinched. For tout atout / sans atout (no symbol exists) the
  translated name stays as text. The badge is `aria-hidden`.
- **Bounce only on change.** The station keeps `dataset.contractKey` (player,
  value, suit, coinched). The badge gets the bounce class only when that key
  differs from the one it had; a re-render with the same contract draws the badge
  without animation. When bidding ends and the board replaces the auction, the
  key is unchanged, so nothing bounces again. Coinche changes the key, so it
  bounces once. The bounce follows the effects level (US-094) the same way the
  other badge pops do (none at Reduced and Off).
- **Callers pass the contract:**
  - bot bidding (`loadBidding`, `playOpeningDeal`, `submitBid`): the contract of
    the calls shown so far, including the human's own call as soon as its bubble
    shows;
  - bot game (`loadBotGame`): `contractFromBoard(board)`;
  - private table bidding: `contractFromAuction(bidding.auction.slice(0,
    revealedCount))`, plus the viewer's own call once `privateOwnCallShown`;
  - private table board (`renderPrivateBoard`): `contractFromBoard(board)`;
  - pass-and-play uses the bot-game paths; the badge is public information, so
    the hand-off cover needs no change.
- `announceCall` updates the badge right after `showPlayerCall` shows a call,
  from a per-prefix running contract that `contractAfterCall` advances. The
  running contract is reset when a new auction starts.
- The station's `aria-label` gets the contract appended, e.g. "Luc, East–West, 5
  cards, 110 Hearts, coinched", so each seat reads its contract once (the badge
  itself is `aria-hidden`). The calls are already announced as they happen by the
  `role="status"` table message ("Luc bids 110 Hearts."), so no new live region.

### Removed (`index.html`, `app.js`)

| Removed | Where |
|---|---|
| `#contract-summary` line | bot game board |
| `#trump-suit` ("Trump …") | bot game board status |
| `#current-contract` | bidding table |
| `#private-trump` ("Trump …") | private table board status |
| `#private-current-contract` | private table auction |
| the contract in `#private-game-heading` | private table board: the heading shows the variant name instead (same text as the bot game's `#game-variant-title`) |

Their writers in `app.js` and now-unused translation keys (for example
`Contract: {0} {1}{2}`) go with them. Messages that merely mention a suit
("Camille takes with Hearts.") stay: they are narration, not a contract line. The
trump suit remains visible to players during play through the badge, which in
classic Belote is the trump symbol at the taker's seat.

### `styles.css`

`.contract-badge` placed beside the avatar on the side that does not overlap the
dealer chip, the name or the call bubble, for the four seat placements; red tint
for hearts and diamonds as in the call bubbles; a `×2` mark style; the bounce
keyframes under the existing effects-level rule. It must not hide cards or overflow
on a phone seat.

### `i18n.js`, `i18n-nl.js`

Only the texts the station label adds ("{0}, contract {1}" and "coinched", unless
existing keys already cover them), French and Dutch.

## Testing

- **Node tests** `frontend-tests/contract-badge.test.cjs`: `contractAfterCall`
  (first bid, raise by another player moves the holder, passes after a bid keep
  it, pass first gives `null`, coinche marks and keeps the holder, coinche with no
  contract is ignored, classic trump call, tout atout bid "100 All trumps");
  `contractFromAuction` on the contract example `contree_bidding.json` (holder
  Luc, 110 Hearts) and on a prefix of it; `contractFromBoard` for Contrée,
  classic and a coinched board.
- **Browser** (Playwright, French, then English and Dutch once each; 1280 × 800
  and 390 × 844): bot Contrée auction (badge appears with the bubble, moves on a
  raise, stays after passes), classic round 1 accept and round 2 suit choice,
  coinche mark, badge stays through play and removed lines are gone, a private
  table, pass-and-play across the hand-off cover, reload mid-auction, effects
  Reduced and Off (no bounce), accessibility tree (each seat reads its contract
  once), no console errors.
- Project checks from `AGENTS.md`: `cargo fmt --check`, `cargo clippy
  --all-targets -- -D warnings`, `cargo test --workspace` (Docker running).

## Out of scope

- Surcoinche, until the rule exists.
- The score bar (US-096) and the dealer symbol (US-097); this story only makes
  sure the score bar will not need to repeat the contract.
- A symbol for tout atout and sans atout (none in the US-095 sprite).
- Any server or API change.

## Known limits

- The badge reads the calls the client has shown, so on a very slow reveal it
  trails the server by design.
