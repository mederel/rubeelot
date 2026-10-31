# Belote — Initial Product Backlog

## Product goal

Deliver an approachable and reliable application for four-player French Belote,
allowing players to learn, play against AI, and play with friends online.

## Release scope

### MVP

The first playable release supports classic Belote, a full four-player table,
AI opponents, private online games, scoring, and a rules reference. The first
implementation target is a game to 1,000 points.

### Later releases

Coinchee/Contree, public matchmaking, player progression, local pass-and-play,
and social features follow after the core game is stable.

## Epics and user stories

### E01 — Start and configure a game

**US-001 — Choose a game mode**

As a player, I want to choose whether to play a game against AI or create a
private online table, so that I can start the type of game I want.

Acceptance criteria:

- The home screen presents at least “Play against AI”, “Private online game”,
  “Tutorial”, and “Rules”.
- Selecting a mode leads to the corresponding configuration or game screen.
- The default rules are classic Belote to 1,000 points.

**US-002 — Configure an AI game**

As a solo player, I want to choose the AI difficulty before starting, so that
the challenge matches my experience.

Acceptance criteria:

- At least two difficulty levels are available.
- The selected difficulty is visible during the game.
- Starting the game immediately fills the other three seats with AI players.

**US-003 — Create and join a private online table**

As a player, I want to create a private table and share an invitation code,
so that I can play with people I know.

Acceptance criteria:

- The table owner receives a joinable code or link.
- A player can join using that code before the game begins.
- Each occupied seat displays the participant’s name and ready state.
- The owner can start only when four players are present and ready.

Functional tasks:

- Define the table lifecycle: created, waiting, ready, in progress, completed,
  and abandoned.
- Define how teams and seats are assigned in private games.
- Specify invitation expiry and the behaviour for invalid or full tables.

### E02 — Play a legal game of classic Belote

**US-004 — View my hand and the current table state**

As a player, I want to clearly see my cards, turn, trump suit, contract, and
score, so that I can make an informed play.

'Tout-Atout' (All Trumps) and 'Sans-Atout' (No Trumps) are out of scope of the MVP, and will be contributed in a later version of the product. 

Acceptance criteria:

- The player’s eight cards are visible only to that player.
- The active player is unambiguous.
- Trump, declaring team, current trick, completed-trick count, and both team
  scores are visible throughout a round.
- Opponents’ card faces are never exposed.

**US-005 — Deal and choose trump**

As a player, I want the application to conduct the deal and bidding sequence,
so that trump is selected according to the rules.

Acceptance criteria:

- The game deals cards and presents the upturned card as required by the
  selected classic-Belote rule set.
- Players are prompted in turn to pass or accept/select trump.
- If all players pass, the system follows the agreed redeal rule and explains
  what happened.
- Once selected, trump and the declaring team are shown to all players.

**US-006 — Play a card**

As a player, I want to select a card from my hand and play it on my turn, so
that I can participate in each trick.

Acceptance criteria:

- A card can be played only by the active player.
- The application enforces follow-suit, trumping, and overtrumping rules.
- Legal moves are available without requiring rule knowledge; illegal cards
  cannot be submitted.
- The played card is visible in the center of the table and removed from the
  player’s hand.
- High-light in the last trick which card is the winning card that will bring the 'Dix de der' to the winning party

**US-007 — Resolve a trick**

As a player, I want each trick to be resolved automatically and visibly, so
that I understand who takes the next lead.

Acceptance criteria:

- After four cards are played, the system identifies the winner using the
  lead suit and trump values.
- The winning team receives the trick’s card points.
- The winner leads the next trick.
- The completed trick is briefly reviewable before the next lead.

**US-008 — Declare Belote/Rebelote**

As a player holding the king and queen of trump, I want to declare
Belote/Rebelote while playing them, so that my team receives the bonus.

Acceptance criteria:

- The application recognizes eligible king/queen-of-trump plays.
- It records the declaration only once and awards the correct bonus.
- The declaration is shown to all players and included in the score breakdown.

Functional tasks:

- Confirm and document the precise classic-Belote rules used, including
  second-round trump choices, partner-winning exception, last-trick bonus,
  Belote/Rebelote, and all-pass redeal behaviour.
- Create a rules engine separated from UI and networking.
- Define deterministic game events for deal, bid, card play, trick win, and
  round end.
- Prepare rule-engine test cases for every legal-move constraint and card rank.

### E03 — Score rounds and finish a match

**US-009 — Receive an explained round result**

As a player, I want to see how each round was scored, so that the result is
trusted and easy to understand.

Acceptance criteria:

- The round result shows card points, last-trick bonus, Belote/Rebelote bonus,
  contract outcome, and total awarded to each team.
- The system identifies a failed contract and allocates points according to the
  selected rule set.
- Players can continue to the next deal after viewing the result.

**US-010 — Finish a match**

As a player, I want the game to identify the winning team at the score target,
so that the match has a clear conclusion.

Acceptance criteria:

- Cumulative team scores persist across deals.
- A match ends when a team reaches or exceeds 1,000 points after a round.
- The result screen identifies winners and shows a rematch option.

Functional tasks:

- Specify scoring examples for made, failed, tied, and capot rounds.
- Define rematch behaviour, including seat retention and score reset.

### E04 — Learn the game

**US-011 — Consult the rules during play**

As a new player, I want a concise rules reference available from the table, so
that I can resolve questions without leaving the game.

Acceptance criteria:

- The reference covers deal, trump selection, card ranks, obligations, and
  scoring for the active variant.
- Opening it does not reveal hidden cards or alter the current turn.
- It is available from the home screen and from an active game.

**US-012 — Play a guided tutorial**

As a new player, I want an interactive tutorial, so that I can learn the core
flow before joining other players.

Acceptance criteria:

- The tutorial explains trump, card strength, following suit, and taking a
  trick through guided choices.
- It gives feedback for incorrect choices and permits replay.

Functional tasks:

- Write tutorial scenarios and instructional copy.
- Define a compact, localized French-first rules glossary.

### E05 — Reliable online play

**US-013 — Recover from a temporary disconnection**

As an online player, I want to reconnect to my ongoing game, so that a brief
network interruption does not end the match.

Acceptance criteria:

- The player can rejoin their original seat while the game is active.
- The restored table reflects the authoritative current state.
- Other players are notified of the disconnected and reconnected status.
- A configurable timeout defines the outcome when reconnection fails.

**US-014 — Be protected by turn handling**

As a player, I want clear turn timing and inactivity handling, so that online
games keep moving.

Acceptance criteria:

- A table may enable a turn timer before the game starts.
- Players receive a visible warning before their turn expires.
- The timeout outcome is defined and communicated before play begins.

Functional tasks:

- Choose a server-authoritative model for game state and rule validation.
- Define reconnection tokens, session expiry, and anti-duplicate-session rules.
- Define online-game timeout and abandonment policy.

### E06 — Accessibility and presentation

**US-015 — Use the application comfortably**

As a player, I want readable cards and controllable presentation settings, so
that I can play comfortably on my device.

Acceptance criteria:

- Card suits are distinguishable without colour alone.
- Text and touch targets remain usable on supported screen sizes.
- Players can control sound and animation settings.
- The UI exposes accessible labels for essential game actions and state.

Functional tasks:

- Define supported devices, browsers/platforms, and minimum screen sizes.
- Produce UI states for waiting, active turn, inactive turn, result, error,
  reconnecting, and empty table.

## Post-MVP backlog

- **US-016:** As an experienced player, I want to select Coinchee/Contree, so
  that I can play my preferred variant.
- **US-017:** As a player, I want public matchmaking, so that I can find a game
  without arranging one privately.

  Acceptance criteria:

  - The home screen offers a "Find a game" mode leading to `/online/public`.
  - Finding a game seats the player at the open public table of the chosen
    variant with the most seated players, or opens a new one when none exists.
  - Players are only matched with players who chose the same variant.
  - The game starts automatically when the fourth player is seated; no ready
    step or owner action is needed.
  - While waiting, the player sees the seated players and a countdown until
    bots take the empty seats (60 seconds after the table opened); when it
    reaches zero, bots fill the table and the game starts.
  - A waiting player can leave the table; their seat is freed for others.
  - Public tables show no invitation code and cannot be joined by code.
  - New texts are translated into French and Dutch.

  Design: `docs/superpowers/specs/2026-10-02-public-matchmaking-design.md`.
- **US-018:** As a player, I want match history and statistics, so that I can
  follow my progress.

  Split into US-057 (sign in with an OAuth provider), US-058 (record finished
  matches) and US-059 (see my match history and statistics), under
  **Accounts and history** below. Design:
  `docs/superpowers/specs/2026-10-06-accounts-and-match-history-design.md`.
- **US-019:** As players sharing one device, we want pass-and-play, so that we
  can play locally.

  Acceptance criteria:

  - The home screen offers a "Pass and play" mode leading to `/play/local`.
  - Two to four players enter their names in the seats they choose (North,
    East, South, West); bots take the empty seats. The players choose the
    variant and the bots' difficulty. Names are required, at most 30
    characters, and differ from each other.
  - The game runs on the server, like a game against bots: bots play their
    turns, and the device always shows the hand of the human whose turn it is.
  - Before a different human's turn, a full-screen hand-off screen hides every
    card and asks to pass the device to that player, who taps "I'm ready" to
    see their hand. A completed trick stays with the player who just played.
  - Pass-and-play matches are not recorded in the match history, since one
    device and one sign-in cannot tell the players apart.
  - New texts are translated into French and Dutch.
- **US-020:** As a player, I want optional in-table reactions or chat, so that
  online games feel social.

  Acceptance criteria:

  - At private and public tables, while a match is played, a seated player can
    send one of six preset reactions: "Well played!", "Nice hand!", "Oops!",
    "Wow!", "Let me think…" and "Good game!". There is no free text, so
    nothing needs moderating or storing.
  - The other players see the reaction for a few seconds next to the sender's
    seat, in their own language.
  - The server accepts at most three reactions per player in ten seconds.
  - A setting turns reactions off: the player neither sees nor sends them.
  - Reactions are not offered in solo or pass-and-play games.
  - New texts are translated into French and Dutch.

## Additional user stories delivered after US-016

These stories were implemented after Coinchee/Contree (US-016) without being
planned in the backlog. They are recorded here retroactively.

### E02 — Play a legal game of classic Belote

**US-021 — Follow the bidding at the card table**

As a player, I want the bidding to take place at the card table with clear
animations, so that I can follow each player's decision.

Acceptance criteria:

- The card table, seats, and my hand remain visible during bidding.
- The player currently deciding is highlighted while they think.
- Each call appears as a bubble at the caller's seat, coloured by call kind
  and showing the suit symbol, with a ripple on the calling seat.
- A table message narrates each decision, and the auction is paced slowly
  enough to be read.

**US-022 — Play at a clear and attractive card table**

As a player, I want a bright table with readable cards, a sorted hand, and a
clear trick layout, so that I can understand the game at a glance.

Acceptance criteria:

- My hand is sorted by suit and rank.
- Trick cards are placed around a diamond, each in front of the player who
  played it.
- The winning card of each trick is highlighted.
- Cards have an improved, readable design.
- The application has a favicon.

**US-023 — See cards being dealt and played**

As a player, I want dealing and card play to be animated, so that I can see
where each card comes from.

Acceptance criteria:

- Cards are animated from the deck to each player during the deal.
- Each played card is animated from the player's seat to the trick.
- Cards played by bots immediately after a trick is collected are animated
  too.

### E01 — Start and configure a game

**US-024 — Fill a private table with bots**

As a private table owner, I want to start the game before four players have
joined, so that we can play even when some friends are missing.

Acceptance criteria:

- The lobby offers a “Start now with bots” action to the table owner.
- Empty seats are taken by bots that bid and play automatically.
- The lobby explains that the owner can either wait for four ready players or
  start now with bots.

### E03 — Score rounds and finish a match

**US-025 — Play several rounds and rematch at a private table**

As a player at a private table, I want to deal the next round and start a
rematch, so that we can play a full match with friends.

Acceptance criteria:

- The private table keeps a cumulative match score.
- The round result panel offers a button to deal the next round.
- When the match is over, a rematch can be started.

### E06 — Accessibility and presentation

**US-026 — Play in my language**

As a player, I want to use the application in my language, so that I can
understand every screen and message.

Acceptance criteria:

- French, English, and Dutch are available; French is the default.
- Static screens, dynamic UI text, suits, ranks, card names, and
  server-generated messages are translated.
- The language can be changed in Settings and the choice is remembered.

**US-027 — Hear the game**

As a player, I want sound effects for game events, so that the table feels
alive and I notice what happens.

Acceptance criteria:

- Dealing, card play, bids, trick and round results, and tutorial answers have
  sound effects.
- Sounds are synthesised in the browser, without audio files.
- Sound effects honour the Settings option to turn them off.

**US-028 — Choose options with buttons**

As a player, I want choices presented as buttons rather than dropdown menus,
so that I can pick them quickly, especially on touch screens.

Acceptance criteria:

- Bid amounts, trump suits, game variant, turn timer, and language are chosen
  with button groups.
- Illegal bid amounts and suits are disabled rather than rejected after
  submission.

**US-029 — Play comfortably on a phone**

As a mobile player, I want game screens to fit my screen, so that I can play
without scrolling.

Acceptance criteria:

- Game setup, bidding, and card play fill the phone viewport without
  scrolling.
- The header and HUD are compact, the table is flexible, cards are smaller,
  and the contract picker fits on one row.

### E05 — Reliable online play

**US-030 — Keep the service available under abuse**

As an operator, I want the server to bound its resource use, so that a single
client cannot degrade or exhaust the service.

Acceptance criteria:

- API requests are rate limited per client, with a stricter limit on game and
  table creation or joining; excess requests receive `429` with
  `Retry-After`.
- API request bodies above a size limit receive `413`.
- The number of AI games and private tables is capped; beyond it, creation
  returns `503`.
- Idle games and tables are evicted after a configurable delay.
- Tomcat thread, connection, timeout, and header-size limits are set, and
  player names are limited to 30 characters.
- Defaults are configurable and documented in the README.

**US-031 — Ship the server as a native executable**

As an operator, I want to compile the application to a GraalVM native image,
so that it starts quickly and uses less memory.

Acceptance criteria:

- The build can produce a native executable with GraalVM 25, with the
  toolchain resolved automatically.
- The native build command is documented in the README.

## Improvement user stories

These stories come from the improvement list gathered while playing the
application. They are not implemented yet.

### Terminology

**US-032 — Call computer players “bots” everywhere**

As a player, I want computer-controlled players to be consistently called
“bots”, so that the vocabulary is the same throughout the application.

Acceptance criteria:

- All UI text, in every language, uses “bot” instead of “AI”.
- Classes, endpoints, and other code identifiers are renamed from “AI” to
  “bot”.

### Bot play

**US-033 — Bots do not waste an ace on an opponent's trump**

As a player, I want bots to avoid playing an ace on a trick already trumped by
the opposing team, so that my bot partner does not give away points.

Acceptance criteria:

- When an opponent has trumped the current trick and the bot cannot win it,
  the bot does not play an ace if another legal card is available.

**US-034 — Bots do not trump the first trick when defending**

As a player, I want bots outside the contract-winning team not to play trump
on the first trick, so that they keep their trumps to defend.

Acceptance criteria:

- On the first trick, a bot whose team did not win the bidding does not play
  trump unless it has no other legal card.

**US-035 — Bots support their partner's bid**

As a player, I want bots to follow common bidding conventions with their
partner, so that the team reaches sensible contracts.

Acceptance criteria:

- A bot raises its partner's bid by 20 in the same suit when it holds:
  - the jack of the bid suit; or
  - two aces in other suits and a few cards of the bid suit.
- A bot raises its partner's bid by 10 in the same suit when it holds:
  - one ace in another suit and the nine of the bid suit; or
  - two aces in other suits and no card of the bid suit.

**US-036 — Bots do not throw valuable cards on lost tricks**

As a player, I want bots to avoid playing high-value cards on a trick that
the other team will obviously win, so that points are not given away.

Acceptance criteria:

- When the trick is clearly won by the opposing team, a bot plays its
  lowest-value legal card.

**US-037 — Bots cash their aces in time**

As a player, I want bots not to hold on to their aces too long, so that the
aces are not trumped the next time their suit is played.

Acceptance criteria:

- A bot considers the risk of an ace being trumped when that suit is next
  played, and plays the ace earlier when the risk is high.

**US-038 — Plan better bot play**

As a player, I want a plan to make bots stronger, so that games against bots
are more challenging and bot partners more reliable.

Acceptance criteria:

- A written plan identifies the main weaknesses of the current bot, possible
  algorithms, and a way to measure the bot's win rate.
- The plan is split into implementable user stories.

The plan is in `docs/bot-play-plan.md`. The stories below implement it.

**US-047 — Measure bot strength in a bot arena**

As a developer, I want to play bots against each other on repeatable deals, so
that every bot improvement is proven by a measured win rate.

Acceptance criteria:

- Deals can be generated from a seed, and the same seed gives the same deals.
- A headless arena plays two bot strategies against each other, in classic
  Belote and in Contrée. It uses the duplicate format: every deal is played
  twice, with the teams swapping seats.
- The arena reports:
  - the average point difference per deal, with a 95% confidence interval;
  - the win rate in matches to 1,000 points;
  - the contract success rate and the coinche success rate;
  - the redeal rate;
  - the decision time per card.
- The arena runs through a dedicated Gradle task and is not part of
  `./gradlew test`. A short arena run stays in the normal test suite.
- A random legal-card bot is available as a baseline.

**US-048 — Separate bot decisions from the rules engine**

As a developer, I want bot strategies to see only what their player can see,
so that bots cannot cheat and strategies can be swapped.

Acceptance criteria:

- Bidding and card-play decisions go through a bot strategy interface. A
  strategy receives only its player's view:
  - its own hand and legal cards;
  - the auction;
  - the contract;
  - the tricks played so far, with who played each card.
- `GameBoard` and `BotPlayers` no longer contain strategy rules.
- The existing bot behaviour (US-033 to US-037) is unchanged: for the same
  seed, the arena gives the same results before and after the change.

**US-049 — Make the difficulty levels play differently**

As a solo player, I want the difficulty I choose to change how well the bots
play, so that the challenge matches my experience.

Acceptance criteria:

- Each difficulty level uses its own bot strategy. Until the Challenging
  strategy exists (US-056), both levels use the current strategy.
- Bots at private tables use the Challenging strategy.

**US-050 — Bots take contracts in classic Belote**

As a player, I want bots to take a contract when their hand is strong enough,
so that I am not forced to take every contract and deals are not redealt
needlessly.

Acceptance criteria:

- A bot scores its hand for a trump suit using:
  - the jack and nine of that suit;
  - the number of trumps;
  - its aces in other suits;
  - the belote (king and queen of trumps).
- In the first round, a bot accepts the upturned suit when its score reaches a
  threshold. In the second round, it chooses its best suit other than the
  upturned one when that suit's score reaches the threshold.
- In the arena, the new bidding beats the current bidding in point difference.
  The redeal rate and contract success rate are reported.

**US-051 — Bots bid in Contrée according to their hand**

As a player, I want bots to bid what their hand is worth in Contrée, so that
their contracts are realistic and they compete with the opponents.

Acceptance criteria:

- A bot estimates the points its hand can make in each trump suit. It opens
  only when the estimate reaches 80, at the level of the estimate.
- A bot overcalls an opponent's contract when its estimate is higher than the
  current contract.
- Partner support (US-035) still applies.
- In the arena, the new bidding beats the current bidding in point difference
  in Contrée.

**US-052 — Solve a round with all cards visible**

As a developer, I want a solver that finds the best play when all hands are
known, so that stronger bots can search ahead.

Acceptance criteria:

- Given four known hands, the trump suit, and the tricks already played, the
  solver returns the best result each team can force and the card that
  achieves it. The result includes the last-trick bonus (dix de der) and the
  belote.
- Tests on hand-built endgames show the solver's results are exact.
- A benchmark gives the time to solve a full round and a round from the
  fourth trick, on the JVM and in the native image.

**US-053 — Bots play to win and score tricks**

As a player, I want bots to win tricks cheaply and give points to their
partner, so that they play like sensible card players.

Acceptance criteria:

- Bots remember the cards played. They know which cards are masters, how many
  trumps remain, and which suits each player has shown to be void in.
- When the trick is not yet won by its partner, a bot wins it with its
  cheapest winning card, where that is possible and worth it.
- When its partner is sure to win the trick, a bot adds its highest-value
  card that it does not need later.
- When leading, a bot prefers its master cards. It avoids leading a suit in
  which it holds the ten without the ace.
- In the arena, the new card play beats the current card play in point
  difference.

**US-054 — Declaring bots manage their trumps**

As a player, I want a bot that holds the contract to take control with its
trumps, so that it makes its contract more often.

Acceptance criteria:

- A bot on the declaring team leads trumps while the opponents may still hold
  trumps and its team holds the master trump.
- A bot keeps its jack and nine of trumps to regain the lead, rather than
  using them on low-value tricks.
- In the arena, the contract success rate rises compared with the previous
  strategy, and the point difference is positive.

**US-055 — Bots coinche contracts they expect to defeat**

As a player, I want bots to coinche when they are confident of defeating the
contract, so that overbidding is punished.

Acceptance criteria:

- A defending bot coinches when its estimated defensive points make the
  contract very likely to fail.
- In the arena, coinches by bots succeed at least two times out of three, and
  coinching improves the point difference.

**US-056 — Challenging bots search before playing a card**

As an experienced player, I want Challenging bots to look ahead before
choosing a card, so that the game is a real challenge.

Acceptance criteria:

- To choose a card, a Challenging bot:
  1. samples many deals of the unseen cards, consistent with its own hand,
     the cards played, and the voids shown;
  2. solves each sample with the solver (US-052);
  3. plays the card with the best average result.
- A bot uses only information its player can see.
- A bot chooses a card within a configurable time budget, 200 ms by default,
  on the JVM and in the native image.
- With a fixed seed, a bot's decisions are repeatable.
- In the arena, the Challenging strategy beats the Relaxed strategy in point
  difference in both variants. The lower bound of the 95% confidence interval
  is above zero.

### Table presentation

**US-039 — Fit the table on a laptop screen**

As a player on a laptop, I want the table and the UI to fit my screen, so that
I can play without scrolling or zooming out.

Acceptance criteria:

- On typical laptop screen sizes, the whole table, the player's hand, and the
  game controls are visible without scrolling.

**US-040 — Animate bidding at private tables**

As a player at a private table, I want the same bidding animations as in bot
games, so that I can follow each player's decision.

Acceptance criteria:

- The bidding animations of US-021 are also shown at private tables.

**US-041 — Collect tricks automatically**

As a player, I want completed tricks to be collected automatically after a
short delay, so that I can see the cards without having to press a button.

Acceptance criteria:

- The “pick up the trick” button is removed.
- A completed trick stays visible long enough for players to see the cards,
  then is collected with an animation.

**US-042 — Play the last trick automatically**

As a player, I want the last trick to be played automatically, so that I do
not have to play a card that is my only choice.

Acceptance criteria:

- When each player has one card left, the application plays them
  automatically, one after the other, without waiting for the players.

### Private tables

**US-043 — Choose seats when starting with bots**

As a private table owner who starts a game without all players, I want the
players to choose their positions at the table, so that we can pick our
partners.

Acceptance criteria:

- Before a game starts with bots, the human players can choose their seats.
- Bots take the remaining seats.

### Scoring

**US-044 — Score a capot**

As a player, I want a team that takes every trick to receive the capot score,
so that the scoring follows the rules.

Acceptance criteria:

- A team that takes all the tricks of a round scores 250 instead of 162.
- In Coinchee/Contree, the team also scores its bid.
- Belote/Rebelote adds 20 points to the declaring team.

**US-045 — Accumulate scores across rounds**

As a player, I want team scores to accumulate between rounds in bot games and
at private tables, so that we play a full match.

Acceptance criteria:

- Team scores are added up round after round, both in bot games and at
  private tables.

**US-046 — Win the match at 1,000 points**

As a player, I want the first team to reach 1,000 points to win the match, so
that the match has a clear end.

Acceptance criteria:

- The match ends when a team reaches 1,000 points, both in bot games and at
  private tables, and the winning team is announced.

### Accounts and history

**US-057 — Sign in with an OAuth provider**

As a player, I want to sign in with an existing Google or GitHub account, so
that the game can remember my matches without a new password.

Acceptance criteria:

- The home screen offers "Sign in with Google" and "Sign in with GitHub" for
  each configured provider, and nothing when none is configured.
- The first sign-in creates an account named after the provider's profile
  (name, else login). Later sign-ins with the same provider account find it.
- When signed in, the home screen shows the player's name and a "Sign out"
  button, and the name is suggested when creating or joining a table.
- Guests can still play every mode without signing in.
- Accounts are stored in MariaDB (PostgreSQL until ADR-002), created by a
  Flyway migration.
- Provider credentials and the database come from environment variables,
  documented in the README with the Docker Compose setup.
- New texts are translated into French and Dutch.

**US-058 — Record finished matches**

As a signed-in player, I want my finished matches recorded, so that I can look
back at them. Criteria in the design document.

**US-059 — See my match history and statistics**

As a signed-in player, I want to see my results, so that I can follow my
progress. Criteria in the design document.

## Roadmap after US-059

Every story up to US-059 is delivered. The stories below are grouped into
six phases, in the recommended order. Each phase can ship on its own.

| Phase | Goal | Stories |
| --- | --- | --- |
| 1 — Complete the rules | Play Belote as it is played at a real table | US-060 to US-064 |
| 2 — Learn by playing | Help players improve, reusing the solver | US-065 to US-067 |
| 3 — Compete and connect | Give signed-in players reasons to come back | US-068 to US-072 |
| 4 — Reach more players | Install the game and play it without a mouse | US-073 to US-074 |
| 5 — Accounts and privacy | Keep my history under my own name, and control my data | US-075 to US-084 (US-084 supersedes US-075, US-077 and US-079, ADR-005; US-076, US-078 and US-080 dropped or merged, ADR-004) |
| 6 — A playful, cartoon table | Make the table graphical and entertaining | US-086 to US-099 (build US-094 first, with US-086) |

Why this order:

- **Rules before features.** Players who know the game will miss the
  announcements (tierce, cinquante, cent, carré). They change the scoring, so
  they must come before ratings (US-068); otherwise ratings would be based on
  incomplete rules.
- **Coaching reuses the solver.** The double-dummy solver (US-052) and the
  sampling strategy (US-056) already compute the best play. A hint and a
  round review mostly need UI.
- **Competition needs players.** Ratings, leaderboards, and friends only make
  sense once public matchmaking has steady traffic.
- **Privacy before publishing names.** Phase 5 does not depend on the other
  phases. The privacy stories (US-081 to US-083) should ship before the
  leaderboard (US-069) shows players' names to everyone.

### Phase 1 — Complete the rules

**US-060 — Declare announcements**

As a player, I want to declare tierce, cinquante, cent, and carrés, so that
the game follows the full Belote rules.

Acceptance criteria:

- During the first trick, a player can declare sequences (tierce: 20,
  cinquante: 50, cent: 100) and carrés (jacks: 200, nines: 150, aces, tens,
  kings, queens: 100).
- Only the team with the best announcement scores its announcements. The
  best is the highest value, then the longest sequence, then the highest
  card, then the trump sequence; the first declarer wins remaining ties.
- Announcements are shown to every player at the end of the first trick, and
  counted in the contract outcome and in the round breakdown (US-009).
- A table option turns announcements on or off before the game starts. They
  are on by default.
- The rules reference (US-011) explains them.
- New texts are translated into French and Dutch.
- Delivered: design in
  `docs/superpowers/specs/2026-10-21-announcements-design.md`. Players opt in
  on their first card; scoring is FFB-style, so Belote and announcements count
  towards the contract (in Classic, the taker must beat the defence's total).

Functional tasks:

- ~~Confirm the rule source for announcement values, ties, and whether
  announcements count towards making the contract.~~ FFB-style scoring (see
  the design).

**US-061 — Bots declare and use announcements**

As a player, I want bots to declare their announcements and take them into
account, so that they play by the same rules as me.

Acceptance criteria:

- Bots always declare their announcements; bidding estimates include them.
- Bots use the cards revealed by the opponents' announcements, in card
  memory and in the Challenging bot's samples (US-056).
- The solver (US-052) scores announcements and judges a Classic contract by
  the US-060 rule: the taker's total, Belote included, must beat the
  defence's (today `SamplingStrategy` still checks card points ≥ 82 without
  Belote).
- In the arena, bidding with announcements beats bidding without them in
  point difference.
- Delivered: design in
  `docs/superpowers/specs/2026-10-22-bot-announcements-design.md`. In Contrée,
  counting announcements gains +9.95 ± 1.62 points per deal over
  `no-announcement-bidding`; in Classic the difference is within noise. The
  arena plays with announcements by default (`-Pannouncements=off`).

**US-062 — Choose the match target**

As a table owner or solo player, I want to choose how many points win the
match, so that a match fits the time we have.

Acceptance criteria:

- Before the game, the player chooses 500, 1,000 (default), 1,500, or 2,000
  points, using a button group (US-028).
- The target is shown during the game and used to end the match (US-046).
- Public matchmaking (US-017) stays at 1,000 points.
- The match history records the target of each match.
- New texts are translated into French and Dutch.
- Delivered: design in
  `docs/superpowers/specs/2026-10-22-match-target-design.md`. The target is
  chosen in the solo and pass-and-play setup and by a private table's owner
  in the lobby; the score line reads "· to 1,500"; Flyway V10 adds
  `match_record.target` (older matches read as 1,000) and the export and the
  statistics' recent matches carry it.

**US-063 — Play all trumps and no trumps in Contrée**

As an experienced player, I want to bid all trumps (tout-atout) and no trumps
(sans-atout) in Contrée, so that I can play my strongest hands.

Acceptance criteria:

- In Contrée, a player can bid tout-atout or sans-atout in addition to the
  four suits. A bid must still be higher in value than the previous one
  (FFB §4.1).
- Tout-atout: every suit uses the trump order and values. Sans-atout: every
  suit uses the plain order and values.
- Legal-move rules, trick resolution, belote, capot, and scoring are adapted
  to both contracts, and each has rule-engine tests.
- A table option turns them on or off before the game starts. It is on by
  default, and public matchmaking (US-017) always plays with it.
- The rules reference explains them.
- New texts are translated into French and Dutch.
- Delivered: design in
  `docs/superpowers/specs/2026-10-23-all-trumps-design.md`. Scales follow
  FFB §11: sans-atout A 19, 10 10, K 4, Q 3, J 2; tout-atout J 13, 9 9, A 6,
  10 5, K 3, Q 2; both total 162 with the dix de der. A bid must be higher in
  value than the previous one, so there is no same-value overcall. The
  option is chosen in the solo and pass-and-play setup and by a private
  table's owner in the lobby; Flyway V11 adds `match_record.all_trumps`.
  Bots play both contracts but do not bid them yet (US-064).

Functional tasks:

- ~~Confirm the point scales of tout-atout and sans-atout used by the rule
  source.~~ FFB §11 scales (see the design).

**US-064 — Bots bid and play all trumps and no trumps**

As a player, I want bots to bid and play tout-atout and sans-atout, so that
they use the whole Contrée auction.

Acceptance criteria:

- Bots estimate tout-atout and sans-atout hands and bid them when the
  estimate is the best.
- The solver and the Challenging bot handle both contracts.
- In the arena, Contrée bots with the new contracts beat bots without them in
  point difference.
- Delivered: design in
  `docs/superpowers/specs/2026-10-24-bot-all-trumps-design.md`. The tout-atout
  and sans-atout estimates are fitted on bot play by least squares
  (`./gradlew fitEstimates`), the coinche estimate against them on 162 less
  the taker's total; bots bid them with a margin of 25 and leave a partner's
  TA/SA contract alone. Under tout-atout the trump tactics no longer apply, so
  bots cash masters and use their jacks. The arena plays with the option on
  (`-PallTrumps`); `current` beats `no-all-trumps-bidding` by +2.40 ± 0.99
  and +2.72 ± 1.02 points per deal on two seeds.

### Phase 2 — Learn by playing

**US-065 — Ask for a hint**

As a learning player, I want to ask which card or bid is best, so that I can
improve while I play.

Acceptance criteria:

- In solo games, a "Hint" button highlights the card or bid that the
  Challenging bot would choose, using only the player's view.
- The hint gives a short reason ("Win the trick cheaply", "Give points to
  your partner", "Lead trumps").
- Hints are not offered in online games.
- New texts are translated into French and Dutch.

**US-066 — Review a finished round**

As a player, I want to replay the tricks of the last round, so that I can
understand what happened.

Acceptance criteria:

- From the round result, the player can step through the tricks one by one,
  seeing who played each card and who won the trick.
- In solo games, all four hands are shown during the review. In online
  games, only the cards that were played are shown.
- New texts are translated into French and Dutch.

**US-067 — See my costly mistakes**

As a learning player, I want the review to point out the cards that cost my
team points, so that I learn from my mistakes.

Acceptance criteria:

- In a solo-game review, the solver compares each of my cards with the best
  card for the same situation.
- Cards that lost my team ten or more points are marked, with the better
  card and the number of points lost.
- The analysis runs on the server within a time budget, without blocking
  the next round.
- New texts are translated into French and Dutch.

### Phase 3 — Compete and connect

**US-068 — Earn a rating in public games**

As a competitive signed-in player, I want a rating that changes with my
public-game results, so that I can measure my level.

Acceptance criteria:

- Signed-in players have a rating per variant. It changes after each
  finished public match (US-017) based on the result and on the ratings of
  both teams.
- Matches with bots, private tables, and abandoned matches do not change the
  rating.
- The rating is shown on the statistics page (US-059) with its history.
- New texts are translated into French and Dutch.

**US-069 — See the leaderboard**

As a competitive player, I want a leaderboard of the best-rated players, so
that I can compare myself with others.

Acceptance criteria:

- A leaderboard page lists the top 50 players per variant with their rating
  and number of matches. Players need at least ten rated matches to appear.
- A signed-in player sees their own rank even when it is outside the top 50.
- A player can hide themselves from the leaderboard in Settings.
- New texts are translated into French and Dutch.

**US-070 — Invite my friends**

As a signed-in player, I want to keep a list of friends and invite them to my
table, so that I don't have to send a code every time.

Acceptance criteria:

- A player can add a friend by sharing a personal friend link; both players
  must accept.
- A table owner can invite online friends from the lobby; the friend gets
  a notification in the app and can join in one click.
- A player can remove a friend at any time.
- New texts are translated into French and Dutch.

**US-071 — Watch a game**

As a player, I want to watch a game at a friend's table, so that I can learn
or wait for a seat.

Acceptance criteria:

- A table owner can allow spectators before the game starts.
- Spectators see the played cards, the bids, and the scores, but never a
  player's hand.
- Spectators cannot send reactions; seated players see how many spectators
  are watching.
- New texts are translated into French and Dutch.

**US-072 — Choose my display name**

As a signed-in player, I want to change the name shown to other players, so
that I am not forced to use my provider's profile name.

Acceptance criteria:

- A signed-in player can change their display name in Settings; it follows
  the existing name rules (at most 30 characters).
- The new name is used at tables, in match history, and on the leaderboard.
- New texts are translated into French and Dutch.

### Phase 4 — Reach more players

**US-073 — Install the game on my phone**

As a mobile player, I want to install Beelot from the browser, so that I can
start it like an app.

Acceptance criteria:

- The application is an installable Progressive Web App, with a manifest and
  icons.
- After the first visit, the shell, the rules reference, and the tutorial
  load without a network connection; online modes explain that a connection
  is needed.
- New texts are translated into French and Dutch.

**US-074 — Play with the keyboard and a screen reader**

As a player who doesn't use a mouse, I want to play every action with the
keyboard and hear the game through a screen reader, so that the game is
accessible to me.

Acceptance criteria:

- Cards, bids, and every game control can be reached and used with the
  keyboard, with a visible focus.
- Game events (bids, cards played, trick winner, round result) are announced
  through a live region.
- An accessibility audit of the main screens reports no serious issue.
- New texts are translated into French and Dutch.

### Phase 5 — Email accounts and privacy

Any player can create an account, and every player controls their data. The
stories follow the GDPR (RGPD).

Accounts are managed by Spring. There are no passwords and no email addresses:
a player signs in with a passkey only (US-084, ADR-005, which supersedes
ADR-004). The email links, Google and GitHub of US-075, US-077 and US-079 were
delivered first, then removed by US-084; US-081 to US-083 are delivered and
kept.

Shared rules for every story in this phase:

- Every page and API is served over HTTPS only, with HSTS. The session cookie
  is `Secure`, `HttpOnly` and `SameSite=Lax`.
- Session ids, passkey challenges and credential ids never appear in logs,
  error messages, URLs sent to third parties, or analytics.
- The security baseline is NIST SP 800-63B at assurance level 1. Beelot holds
  no payment data, so PCI DSS does not apply (ADR-004).
- New texts are translated into French and Dutch.

**US-075 — Create an account with my email address**

Superseded by US-084 (ADR-005): delivered, then removed with every email
address. The text below is the delivered story.

As a player without a Google or GitHub account, I want to create an account
with my email address, so that my matches are recorded.

Acceptance criteria:

- The home screen offers "Sign in", which asks for an email address and also
  offers Google and GitHub. There is no separate registration form: the first
  link opened creates the account.
- Spring emails a sign-in link. The link works once and expires after 15
  minutes. Only a hash of the token is stored.
- The link opens a page where the player confirms with a button, so that mail
  scanners that open links cannot use up the token.
- Opening the link proves the address: no separate confirmation is needed
  (this replaces US-076). Links never opened create no account; expired tokens
  are purged.
- The answer to a request is always "check your inbox", whether or not the
  address has an account.
- On the first sign-in, Spring creates the `account` row with a random
  pseudonym such as "Swift Otter 42", in English, French or Dutch, at most 30
  characters, and stores the email address in a table of its own (data
  minimisation). US-072 lets players change their name.
- Account creation is based on the performance of a contract, so no
  pre-ticked box and no marketing consent are involved. The link to the
  privacy policy on the sign-in form is added with that page (US-083).
- Google and GitHub sign in as direct OAuth providers. Beelot asks for the
  user id only (the `openid` scope for Google, no scope for GitHub) and keeps
  no name or email address from them.
- Link requests are rate limited per email address and per IP address, so
  that the form cannot flood an inbox; the player sees a message when the
  limit is reached.
- Delivered: design in
  `docs/superpowers/specs/2026-10-10-email-link-sign-in-design.md`.

**US-076 — Confirm my email address**

Merged into US-075 (ADR-004), and delivered with it: opening the sign-in link
confirms the address.

**US-077 — Keep my sessions safe**

Superseded by US-084 (ADR-005), which keeps its session rules: a new session id
on sign-in, 7 days idle, 30 days at most, the last sign-in shown.

As a signed-in player, I want my sessions to end when I stop playing and to
see when I last signed in, so that nobody else uses my account unnoticed.

Acceptance criteria:

- A successful sign-in creates a new session id (no session fixation).
- A session ends after 7 days without any request, and lasts at most 30 days.
- Signing out ends the session.
- The page shows when the last sign-in happened, so the player can spot
  unexpected access.
- Delivered: sessions get a new id on sign-in (Spring Security's default), the
  servlet timeout ends idle sessions after 7 days, and `SessionLifetimeFilter`
  ends any session 30 days after its sign-in. The home screen shows the latest
  sign-in other than the current one: a later one made elsewhere, or else the
  one before (`account.previous_sign_in_at`, also in the data export).

**US-078 — Reset a forgotten password**

Dropped (ADR-004): there are no passwords.

**US-079 — Change my email address**

Superseded by US-084 (ADR-005): Beelot no longer holds an email address.

As a player who signs in with an email link, I want to change my email
address, so that I can still sign in when I change mailbox.

Acceptance criteria:

- An "Account" section in Settings lets email-link players enter a new
  address.
- The new address is used only after a link sent to it is opened, with the
  rules of US-075. Until then, the old address keeps working.
- The old address is notified of the change.
- An address that already belongs to another account is refused without
  revealing it: the new address receives a notice instead of a link.
- Google and GitHub accounts do not see this option.
- Delivered: design in
  `docs/superpowers/specs/2026-10-15-change-email-design.md`. Confirming the
  change ends every session of the account and signs in the browser that
  confirmed it.

**US-080 — Protect my account with a second factor**

Dropped for now (ADR-004): an email link is as strong as the player's
mailbox, and Google and GitHub offer their own second factors. Passkeys come
back as a sign-in method of their own (US-084).

**US-081 — Delete my account**

As a signed-in player, I want to delete my account, so that the game no longer
keeps my personal data (GDPR article 17).

Acceptance criteria:

- Settings offers "Delete my account". It asks the player to sign in again
  with their passkey when their last sign-in is more than 10 minutes old, and
  to confirm.
- Spring deletes the player's `account` row, passkey, security events and
  sessions.
- Finished matches stay in other players' history, but the deleted player's
  seats show "Deleted player" and no longer link to an account.
- Backups containing the account are overwritten within the backup retention
  period stated in the privacy policy.
- Delivered: design in
  `docs/superpowers/specs/2026-10-13-gdpr-privacy-design.md`.

**US-082 — Download my data**

As a signed-in player, I want to download the data the game keeps about me, so
that I can see it and take it elsewhere (GDPR articles 15 and 20).

Acceptance criteria:

- Settings offers "Download my data". It returns a JSON file with the account
  details (display name, creation date, the passkey's creation and last-use
  dates, never its public key or credential id), the sign-in history and every
  recorded match of the player.
- The export is available only to the signed-in owner. It is limited to one
  per hour.
- Delivered: design in
  `docs/superpowers/specs/2026-10-13-gdpr-privacy-design.md`.

**US-083 — Know how my data is handled**

As a player, I want to read how my personal data is used and kept, so that I
can trust the game with it.

Acceptance criteria:

- A privacy policy page, linked from the home screen and the sign-in form,
  states:
  - the data controller and a contact address;
  - the data stored, the purposes and the legal bases;
  - the processors: hosting only;
  - the retention periods, the player's rights, and the right to complain to
    the CNIL.
- A data processing agreement is signed with the hosting provider (GDPR
  article 28).
- Retention periods are enforced by a scheduled job in Spring:
  - passkeys of sign-ups never finished after 1 day;
  - accounts without a sign-in for 3 years, with their passkey, without a
    warning;
  - Spring's security events after 12 months.
- Spring records security events without credential ids or challenges:
  sign-up, sign-in, failed sign-in, sign-out, session expiry, rate-limit
  refusal, export and deletion. Each event stores the account when known, the time, the IP
  address and the outcome.
- Beelot sets only the session cookie and the CSRF cookie. They are strictly
  necessary, so no cookie banner is needed; the policy lists them.
- The data breach procedure (who decides, notifying the CNIL within 72 hours,
  informing players) is documented in `docs/`.
- Delivered: design in
  `docs/superpowers/specs/2026-10-13-gdpr-privacy-design.md`; breach procedure
  in `docs/privacy/data-breach-procedure.md`.

**US-084 — Sign in with a passkey**

As a player, I want to sign in with a passkey, so that I don't need a mailbox,
a password, or a Google or GitHub account to play under my own name.

Acceptance criteria:

- Passkeys are the only way to sign in. Email links, Google, GitHub and the
  SMTP sender are removed, and every existing account is deleted by the rules
  of US-081 (Flyway V8): there is no migration path.
- The sign-in form offers "Sign in with a passkey" and "Create an account".
  Signing in needs no user name: the browser offers the Beelot passkey.
- Before the passkey is created, the form warns that it is the only way to
  sign in and that losing it loses the account, with no recovery.
- An account has exactly one passkey, created at sign-up. It cannot be added,
  replaced or removed later. There is no recovery of any kind.
- Registration requires a discoverable credential and user verification. The
  account gets a random pseudonym (US-075), which is the passkey's display
  name.
- Spring Security provides the WebAuthn flow (`webAuthn()`), with its JDBC
  repositories on MariaDB. Beelot refuses a sign-in whose signature counter
  does not go up.
- Beelot stores the passkey's public key, signature counter and creation and
  last-use dates. No biometric data and no private key leave the player's
  device.
- The sessions keep the rules of US-077.
- Sign-up and sign-in ceremonies are limited to 30 an hour per IP address, and
  recorded as security events (US-083). A failed sign-in never says why.
- The data export (US-082) shows the passkey's dates only. Deleting the
  account (US-081) deletes the passkey; the retention job (US-083) deletes
  accounts after 3 years without a sign-in, and the passkeys of sign-ups never
  finished after a day.
- The pages are translated into English, French and Dutch, and the flow is
  checked in the native image (US-031).
- Design in `docs/superpowers/specs/2026-10-19-passkeys-only-design.md`
  (ADR-005).

### Phase 6 — A playful, cartoon table

The look is playful and cartoonish: thick outlines, saturated colours, rounded
shapes, bouncy motion. Every story honours the effects level of US-094.

**US-086 — Give the table a cartoon look**

As a player, I want a bold, cartoon-style table and cards, so that the game
looks fun the moment it opens.

Acceptance criteria:

- The table has a halftone-textured felt, a chunky painted rim, hard offset
  shadows and rounded, ink-outlined shapes (comic-book style).
- Cards have twelve unique illustrated court cards (jack, queen, king; comic
  royals with one personality per suit) with outlines and clean pips, and a
  designed card back.
- Suits stay distinguishable without colour alone.
- The colours, outlines, radii and shadows are CSS custom properties, so that
  US-087 can swap them.
- The table stays fully visible on a laptop screen (US-039) and on a phone.

**US-087 — Choose a table theme**

As a player, I want to pick a table and card-back style in Settings, so that I
can make the game mine.

Acceptance criteria:

- Settings offers at least three themes (for example Classic green, Midnight
  and Sunset), each with its own card back.
- The choice is remembered in the browser and applies to the bot, private and
  pass-and-play tables.
- New texts are translated into French and Dutch.

**US-088 — Meet my opponents**

As a player, I want every seat to have an illustrated avatar and a
personality, so that the bots feel like characters.

Acceptance criteria:

- Each seat shows a cartoon avatar. The three bots look different, and each
  player's avatar is stable for the whole match.
- Avatars react to the game: thinking, pleased, annoyed, celebrating.
- Bots say short in-character lines at the table (for example after a big
  trick or a coinche); they never reveal their hand.
- New texts are translated into French and Dutch, and use "bot", not "AI".

**US-089 — Make card play feel physical**

As a player, I want cards that fan out, lift, tilt and land with weight, so
that playing them is satisfying.

Acceptance criteria:

- My hand fans out in an arc. A card lifts and tilts when hovered or touched.
- Tapping an illegal card makes it shake and shows why; legal cards are
  visibly distinct.
- A played card flies to the table, squashes on landing and rests at a slight
  random angle. The winning card of a trick pops.
- The animations build on the trick collection of US-041.

**US-090 — Celebrate great moments**

As a player, I want big plays to be celebrated, so that wins feel rewarding.

Acceptance criteria:

- A cartoon banner and a burst of confetti or stars mark a won round, a capot,
  belote and rebelote, and the match victory.
- A broken contract and a successful coinche have their own distinct effects.
- Each effect has a matching synthesised sound (US-027).
- Effects never hide the cards or delay the next action by more than two
  seconds.

**US-091 — Watch the score move**

As a player, I want scores that count up and a progress bar, so that I follow
the match at a glance.

Acceptance criteria:

- Round and match scores count from the old value to the new one.
- A progress bar shows each team's distance to the match target (US-062).

**US-092 — Enjoy a livelier home screen**

As a player, I want an animated home screen, so that the first impression is
fun.

Acceptance criteria:

- An animated shuffle or deal replaces the static suit icons of the hero.
- Mode cards bounce on hover and press.
- A short transition leads from the home screen to the table.

**US-093 — End the match with a show**

As a player, I want an end-of-match screen with a recap, so that finishing a
game feels like an event.

Acceptance criteria:

- The winning team is shown on a cartoon podium, with the final scores.
- A recap shows the number of rounds, the biggest round and the announcements
  of the match.
- "Play again" and "Home" buttons are offered.

**US-094 — Control the effects**

As a player, I want to turn the effects down, so that the game stays
comfortable and fast.

Acceptance criteria:

- Settings offers an effects level: Full, Reduced or Off, separate from the
  sound setting.
- The default follows `prefers-reduced-motion`.
- At Reduced, movement is replaced by short fades; at Off, state changes are
  instant.
- Every effect of US-086 to US-093 honours the level.
- The animations stay smooth on a mid-range phone.
- New texts are translated into French and Dutch.
- Later stories add their effects through the patterns in
  `docs/superpowers/specs/2026-10-31-effects-level-design.md`.

**US-095 — Read the suits at a glance**

As a player who struggles to tell the suits apart, I want large, distinct suit
symbols on the cards, so that I can identify every suit without effort.

Acceptance criteria:

- Each suit has a large, bold symbol on the card, in the corner indices as well
  as the centre. The symbols are much bigger than the rank letters beside them.
- The four suits differ by shape, not only by colour, and the symbols are
  distinct from one another at a small size: for example spades and clubs are
  not mistaken, and hearts and diamonds are not mistaken.
- Red and black keep a high contrast against the card face (WCAG AA at least).
- Settings offers a "Large suit symbols" option, and an option for four
  colours, one per suit, for players who need more help. Both are remembered in
  the browser and apply to the hand, the trick, the contract at the bidder's seat (US-099) and the
  bidding buttons.
- The suit names stay available to screen readers (US-074).
- New texts are translated into French and Dutch.

**US-096 — Keep the match score in view**

As a player, I want the score of the whole match to stay visible during the
game, so that I always know how close each team is to winning.

Acceptance criteria:

- A score bar stays on screen for the whole game: while dealing, bidding,
  playing tricks and between rounds. It shows the match score of each team and
  the match target (US-062), and names my team.
- It is shown in bot games, at private tables and in pass-and-play, and it
  never scrolls out of view or hides the cards, on a laptop screen (US-039) and
  on a phone.
- It also shows the points of each team so far in the round. The contract is
  not repeated there: it is shown at the bidder's seat (US-099).
- Both values update as soon as a round ends, with the count-up of US-091.
- It is exposed to screen readers as a live region that announces changes
  without interrupting play (US-074).
- New texts are translated into French and Dutch.

**US-097 — Show the dealer with a symbol**

As a player, I want the dealer marked by a symbol rather than a letter, so that
I recognise the dealer whatever language the game is in.

Acceptance criteria:

- The dealer marker is a pictogram, for example a dealer button or a hand
  holding a deck, with no letter and no text in it.
- It looks the same in English, French and Dutch, in bot games, at private
  tables and in pass-and-play.
- It is large enough to recognise at a glance and does not rely on colour
  alone, and it fits the cartoon style of US-086.
- It moves to the next seat when the deal passes, with a short animation
  (US-094 applies).
- Hovering it, or focusing it, still shows the translated text "Dealer", and
  screen readers still announce "Dealer" (US-074).

**US-098 — Show suit symbols next to suit names**

As a player, I want every suit named in the text to appear with its symbol, so
that I recognise the suit at a glance and without reading the word.

Acceptance criteria:

- Wherever the text names a suit, the suit symbol is shown next to the name,
  with the symbol first (for example "♠ Spades"): the trump line, the contract
  and the current contract, the bid and trump calls, the hints (US-065), the
  table messages and the tutorial.
- Where the symbol is already shown with a name close by (a call bubble, a
  bidding button), the name may be dropped when the symbol is enough.
- The symbol uses the style of US-095, including its large-symbol and
  four-colour options.
- The symbols come from one shared function next to `suitName`, so that every
  sentence uses the same display, in English, French and Dutch.
- Screen readers read the suit name once, not the name and the symbol
  (US-074).
- Existing translations are not duplicated: the suit names stay in the
  language files.

**US-099 — Show the contract at the bidder's seat**

As a player, I want the ongoing bet printed next to the avatar of the player
who made it, so that I see at once who is playing which contract, without
looking elsewhere on the screen.

Acceptance criteria:

- The current contract (value and trump suit in Contrée, the trump suit in
  classic Belote; with the suit symbol of US-098) appears next to the avatar of
  the player who holds it, as a badge that bounces when it is set or raised.
- During bidding the badge follows the highest bid: it moves to the new bidder
  when a bid is raised. Players who passed show nothing.
- Once the bidding ends the badge stays at the taker's seat for the whole
  round, with a mark when the contract is coinched or surcoinched.
- The contract is no longer printed anywhere else on the screen: the contract
  summary line, the current contract label and the trump line are removed, in
  bot games, at private tables and in pass-and-play. The score bar (US-096)
  does not repeat it.
- The badge is exposed to screen readers as part of the seat, naming the
  player, the value and the suit (US-074).
- Nothing is shown before the first bid.
- New texts are translated into French and Dutch.

### Decisions required for the roadmap

- ~~Select the rule source for announcements (US-060)~~ (FFB-style, delivered)
  and ~~the tout-atout and sans-atout scales (US-063)~~ (FFB §11, delivered).
- Choose the rating system for US-068 (Elo by team average, or Glicko-2).
- ~~Accept ADR-004 (email links in Spring, Google and GitHub directly).~~
  Accepted; ADR-004 supersedes ADR-003.
- ~~Choose the SMTP provider for production.~~ Not needed since US-084.
- Name the data controller and the privacy contact for the privacy policy:
  set through `BEELOT_PRIVACY_CONTROLLER` and `BEELOT_PRIVACY_CONTACT` where
  Beelot runs (see the README).

## Decisions required before implementation

- Confirm the initial platform: web, mobile, or both.
- Confirm whether online play is in scope for the first release or follows the
  AI-only playable version.
- Select the authoritative classic-Belote rules source and regional options.
- ~~Decide whether guests may play online and what account model is required.~~
  Guests play every mode; accounts sign in with a passkey (US-084, ADR-005).
- Define the privacy, moderation, and age requirements before adding chat or
  public matchmaking. (US-020 offers preset reactions only, with no free text,
  so it needs none of them.)
