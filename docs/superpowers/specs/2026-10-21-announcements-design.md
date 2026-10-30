# Declare announcements (US-060) — design

## Goal

Players declare tierce, cinquante, cent and carrés on their first card, as at a
real table. Announcements change the score and whether the contract is made.
A table option turns them off. Bots always declare, but they don't yet use
announcements in bidding or play (US-061).

## Decisions

- **FFB-style scoring.** A team's total for a round is its card points, dix de
  der, Belote and announcements. Belote and announcements count towards the
  contract. This contract rule applies whether or not announcements are on.
- **Opt in on the first card.** A player declares by playing their first card
  of the round with the "Declare" option ticked. A player who doesn't declare
  then loses those points.
- **The server finds the announcements** in the player's hand. The client sends
  only a `declare` flag, never a list of cards.
- **Bots always declare** everything they hold.
- **On by default** for bot games, pass-and-play and private tables. Public
  matchmaking always plays with announcements. Inside the engine the switch is
  off by default, so the arena, the solver and existing callers don't change
  until US-061.

## Rules

### What counts

- **Carrés:** four jacks 200, four nines 150, four aces, tens, kings or queens
  100. Four eights or four sevens don't count.
- **Sequences:** three or more cards in a row in one suit, in the order
  7-8-9-10-J-Q-K-A, whatever the trump. Three cards make a tierce (20), four a
  cinquante (50), and five or more a cent (100).
- A card counts in one announcement at most. **Carrés come first**: sequences
  are formed from the cards that are not in a carré. A hand may hold several
  announcements; it declares all of them.
- Belote/Rebelote is not an announcement. It stays as it is today.

### The best announcement

Only the team holding the best announcement scores, and it scores **all** of
its announcements. The other team scores none of its announcements. One
announcement beats another by, in order:

1. the higher value;
2. the longer run: a sequence counts its length, a carré counts 4;
3. the higher top card, in the order A, K, Q, J, 10, 9, 8, 7;
4. being in the trump suit (a carré is never in the trump suit);
5. having been declared first, in the order the first trick was played.

Announcements of the same team never compete with each other.

### Scoring the round

Each team's total = card points + dix de der + its Belote + its announcements,
if its announcements score.

- **Classic:** the taker makes the contract if its total is **greater than**
  the defence's total. A tie fails the contract; there is no "litige".
- **Contrée:** the taker makes the contract if its total is **at least the
  bid**.
- **Made:** each team scores its total.
- **Failed:** the defence scores 162 + every announcement that scored, whichever
  team declared it, + its own Belote. The taker scores its own Belote only.
- **Capot:** a taker who takes every trick scores 250 (plus the bid in
  Contrée) + its own announcements + its Belote. A taker who is capoted fails
  the contract, and the defence scores 250 + every announcement that scored,
  whichever team declared it, + its own Belote, so a capot never pays less than
  an ordinary failure. The other team scores its Belote only.
- **Coinche** doubles both teams' scores, as today.

## Flows

### Declaring

1. Before a player plays their first card of the round, their board view lists
   the announcements their hand holds (type, cards, value).
2. If that list is not empty, the hand shows a "Declare" option, ticked, that
   names them: "Tierce to the King of ♥, Carré of Jacks".
3. The player plays a card. The request carries `declare: true` or `false`
   (`POST /api/bot-games/{id}/cards` and `POST /api/private-tables/{id}/cards`,
   field `declare`, default `false`).
4. With `declare: true`, the server records the announcements found in the
   hand **before** the card leaves it. The option is gone from the second card
   on.

`declare: true` is ignored, without an error, when the hand holds nothing,
after the first card, or when the table has announcements off.

### Revealing

- During the first trick, every player sees who has declared, by name only
  ("North declares").
- When the first trick ends, the trick-review pause shows an "Announcements"
  panel to every player: each declarer, what they declared with the cards face
  up, and which team scores. If no one declared, there is no panel.
- The scoring team's points go into the live score at that moment, as Rebelote
  does today.

### Table option

- **Bot games and pass-and-play:** an "Announcements: On / Off" button group
  (US-028) next to the variant choice, on by default; the create request gets
  `announcements` (default `true`).
- **Private tables:** the same button group in the lobby. Only the owner can
  change it, before the game starts, through `POST
  /api/private-tables/{tableId}/announcements` with `{playerToken,
  enabled}`, built like `turn-timer`. Others see the current choice.
- **Public matchmaking:** always on.

## Data

### Engine

- A pure `Announcements` class in `fr.beelot.game` finds a hand's
  announcements and picks the best of a round's declarations.
- `Announcement` record: kind (`TIERCE`, `CINQUANTE`, `CENT`, `CARRE`), cards,
  value, suit (null for a carré), top card.
- `GameBoard` keeps each seat's declared announcements and, after the first
  trick, the scoring team.
- `RoundResult` gains `northSouthAnnouncements`, `eastWestAnnouncements` and the
  list of declarations (seat, announcements, scored or not).
- The board view gains: `announcements` (on or off), `ownAnnouncements` (only
  before the player's first card), `declarers` (seats that declared, during
  trick 1), and `revealedAnnouncements` (from the end of trick 1).
- Bots play through the same path with `declare = true`.

### Migration V9

V9 follows the passkeys migration V8 (this branch starts from
`feat/passkeys-only`).

- `match_record.announcements BOOLEAN NOT NULL DEFAULT FALSE`.
- `match_round.north_south_announcements INTEGER NOT NULL DEFAULT 0` and
  `match_round.east_west_announcements INTEGER NOT NULL DEFAULT 0`.

Matches recorded before V9 read as played without announcements.

### Data export

The export (US-082) adds `announcements` to each match and
`northSouthAnnouncements` / `eastWestAnnouncements` to each round. The fields
are added, nothing is removed, so the format stays `beelot-export-2`.

## Client

- The "Declare" option above the hand on the first card, ticked by default.
- The "who declares" note during trick 1 and the "Announcements" panel at its
  end, in bot games, pass-and-play and private tables.
- The round breakdown (US-009) gets a line when announcements are on, such as
  "Announcements: North–South 70 (tierce + cinquante)".
- The On / Off option in the solo setup and in the private-table lobby.
- The rules reference (US-011) gets an "Announcements" section: the values, the
  best-announcement order and how they count towards the contract. Its
  scoring section says that Belote and announcements count towards the
  contract.
- Every new string in English, French and Dutch.

## Error handling

- `declare` never makes a card play fail; an illegal card fails as today.
- `POST /{tableId}/announcements` answers 409 when the caller is not the owner
  or the game has started, as `turn-timer` does.

## Testing

- `Announcements` unit tests: each carré and the excluded eights and sevens;
  tierce, cinquante and cent, and runs longer than five; carrés taking a card a
  sequence would use; several announcements in one hand; each tie-break step,
  down to the first declarer.
- `GameBoard` scoring tests: made, failed and capot in Classic and Contrée, with
  and without a coinche; announcements declared but not best; a player who
  doesn't declare; Belote counting towards the contract; with announcements
  off, card points and Belote only.
- Integration tests: `declare` on bot games and private tables; the private
  table option (owner only, refused after the start); matchmaking tables with
  announcements on; history columns; export fields.
- UI string tests for the new markup and translations; a headless browser check
  of declaring, the reveal panel and the breakdown line.

## Out of scope

- Bots using announcements in bidding, card memory, the solver or the arena
  (US-061).
- Tout-atout and sans-atout (US-063).
- Belote as an announcement, or "litige" carry-over on a tie.
