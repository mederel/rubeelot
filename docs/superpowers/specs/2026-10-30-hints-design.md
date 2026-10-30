# Ask for a hint (US-065) — design

## Goal

A learning player can ask which card or bid is best and why, while playing a
local game. From the backlog (`mederel/beelot`, `docs/product-backlog.md`):

- In solo games, a "Hint" button highlights the card or bid that the
  Challenging bot would choose, using only the player's view.
- The hint gives a short reason ("Win the trick cheaply", "Give points to your
  partner", "Lead trumps").
- Hints are not offered in online games.
- New texts are translated into French and Dutch.

## Decisions

- **Local tables only.** Hints are offered at solo and pass-and-play tables
  (`TableKind::Local`), never at private or public tables. Both local kinds are
  played on one device with no rating at stake.
- **Always the Challenging bot.** The hint uses `SamplingStrategy` whatever the
  table's difficulty, as the story names it. It sees exactly what a bot in the
  player's seat would see: the seat's `PlayView` or `AuctionView`.
- **Hints are free.** No limit beyond the existing per-client rate limit, and
  nothing recorded: the match history and statistics do not change.
- **The reason is derived after the choice.** The sampling search returns only
  a card. A pure classifier looks at the situation and the chosen card or bid
  and names one reason from a fixed list. Each reason has a condition that makes
  its text true; when none holds, a neutral fallback is used.
- **A read-only endpoint.** A hint is a question, not a move: it does not
  change the table, its version or what other devices see.

## Reasons

Reasons travel as stable codes; the client translates them.

### Card reasons

Checked in this order; the first that holds is the reason.

When leading (the current trick is empty):

| Code | Text | Condition |
| --- | --- | --- |
| `LEAD_TRUMPS` | Lead trumps | A suit contract, and the card is a trump. |
| `CASH_MASTER` | Cash a master card | No card left out of the hand can beat it in its suit (by the card memory). |
| `LEAD_LOW` | Lead a low card | The card has the lowest value among the hand's cards of its suit. |

When following:

| Code | Text | Condition |
| --- | --- | --- |
| `GIVE_POINTS` | Give points to your partner | The partner wins the trick so far and is sure to win it (no later player can beat them, by the card memory), the card does not overtake them, and it is worth points. |
| `KEEP_PARTNERS_TRICK` | Keep your partner's trick | The partner wins the trick so far and the card does not overtake them. |
| `WIN_CHEAPLY` | Win the trick cheaply | The card beats the trick's current winner, and no legal card that also beats it has a lower value. |
| `THROW_CHEAPEST` | Throw your cheapest card | The card does not beat the current winner, the partner is not winning, and no legal card has a lower value. |

Fallback, in both cases:

| Code | Text | Condition |
| --- | --- | --- |
| `BEST_FOR_ROUND` | The best card for the rest of the round | None of the above: the search found a card the simple rules would not pick (for instance a winning card that is not the cheapest one). |

"Value" is the card's point value under the round's contract
(`scoring` card points), ties broken by rank strength.

### Bid reasons

| Code | Text | Condition |
| --- | --- | --- |
| `TAKE` | Take: strong hand in {suit} | Classic Belote: the decision takes a trump (`AuctionDecision::Trump`). |
| `SUPPORT_PARTNER` | Support your partner's bid | Contrée: a bid in the trump of the bid the partner holds. |
| `BID_HAND_VALUE` | Bid what your hand is worth | Contrée: any other bid. |
| `COINCHE` | Coinche: you expect to defeat them | The decision is a coinche. |
| `PASS_WEAK` | Pass: your hand is too weak | The decision is a pass. |

## Architecture

### Game crate (`beelot-game`)

A new module `bot/hints.rs`:

```rust
pub struct CardHint { pub card: Card, pub reason: CardReason }
pub struct AuctionHint { pub decision: AuctionDecision, pub reason: AuctionReason }

pub fn card_hint(view: &PlayView) -> CardHint;
pub fn auction_hint(view: &AuctionView) -> AuctionHint;

pub(crate) fn card_reason(view: &PlayView, card: Card) -> CardReason;
pub(crate) fn auction_reason(view: &AuctionView, decision: &AuctionDecision) -> AuctionReason;
```

`card_hint` and `auction_hint` ask the Challenging strategy
(`strategies::for_difficulty(Difficulty::Challenging)`) for its choice, then
classify it. The reason enums serialize as the codes above
(`SCREAMING_SNAKE_CASE`). The classifiers reuse `CardMemory` and the trick
helpers the rule-based bot already has for "master card", "sure to win" and
"beats the current winner"; helpers move to a shared place only if
`rule_based.rs` keeps them private.

The sampling search seeds its draws from the view, so asking twice in the
same position gives the same hint, unless the 200 ms budget cuts the search
short.

### Server (`beelot-server`)

- **Route:** `GET /api/tables/{id}/hint`, with the player's bearer token, like
  `GET /api/tables/{id}`. The handler runs in `blocking`, as `act` does, since
  the search takes up to 200 ms.
- **`Table::hint(actor) -> Result<Hint, AppError>`**, where `Hint` is the
  answer below. It does not touch `version`, so no event is sent.
  - Online table: `404` (`AppError::NotFound`): the route does not exist for
    them.
  - Not the actor's turn, a completed trick under review, the round result
    shown, or no auction or board in progress: `409` with code
    `no-hint`.
- **Answer:**

  ```json
  { "type": "card", "card": { "rank": "A", "suit": "HEARTS", "symbol": "♥" }, "reason": "CASH_MASTER" }
  { "type": "bid", "action": { "type": "contract", "value": 90, "suit": "SPADES" }, "reason": "SUPPORT_PARTNER" }
  ```

  The card is the snapshot's `CardView`. The bid is the action the client
  would send to `/actions` to make it (`pass`, `trump`, `contract` or
  `coinche`), so the client can match it to its bid controls directly.
- **Snapshot:** a `hints: bool` field tells the client whether to show the
  button (true at local tables).
- **Rate limit:** the existing per-client limit on `/api` covers the route; no
  new limit.

### Client (`static/`)

- A "Hint" button by the hand and by the bid controls, shown when the
  snapshot has `hints` and it is the viewer's turn to play or bid.
- Tapping it calls the route, then:
  - for a card: adds a `hint-card` highlight to that card in the hand;
  - for a bid: highlights the matching bid button and preselects the amount
    and suit in the Contrée controls;
  - and shows the translated reason in a line under the hand or the bid
    controls.
- The highlight and the line are cleared when the player acts or the
  snapshot shows another turn. Pass-and-play's hand-off screen hides them with
  the cards.
- A failed request shows nothing new; the button is enabled again.
- New texts (the button, the thirteen reasons) in English, French
  (`i18n.js`) and Dutch (`i18n-nl.js`).

## Testing

- **Classifier unit tests** (`bot/hints.rs`): one hand-built view per reason,
  including the fallback, and each bid reason.
- **Hint tests:** `card_hint` returns a legal card and `auction_hint` a legal
  decision on sampled positions.
- **Server tests:** a hint at a solo table during the player's turn (card and
  bid), `404` at an online table, `409` out of turn and during a trick review,
  and the table's version unchanged after a hint.
- **Browser check:** a Playwright run of a solo round shows the button, a
  highlighted card and a translated reason.

## Out of scope

- Hints at online tables, or a setting to turn them off.
- Counting or recording hints (US-067 covers reviewing mistakes).
- Explaining the search's numbers (expected points per card).
