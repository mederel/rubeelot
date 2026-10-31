# Choose the match target (US-062) — design

## Goal

Before a game, the solo player, the pass-and-play players or a private table's
owner chooses how many points win the match: 500, 1,000, 1,500 or 2,000. The
match ends at that target, the score shows it, and the match history records
it. Public matchmaking stays at 1,000.

## Decisions

- **The target is a validated `int`** (approach A). The allowed values are
  `MatchScore.TARGETS = [500, 1000, 1500, 2000]`. Any other value is refused.
- **1,000 by default**, everywhere: a request without a target, the arena, the
  existing `new MatchScore()` callers, and matches recorded before US-062.
- **The choice lasts for the game**: the next round and a rematch keep it, as
  the announcements option does (US-060).
- **Statistics show the target per match** in the recent matches table. The
  totals (played, won, win rate) keep counting every match together.

## Rules

- The first team to reach the target wins. When both teams reach it in the
  same round, the higher score wins; an exact tie is settled by another round.
  This is today's rule with 1,000 replaced by the target.

## Engine

- `MatchScore(int target)` throws `IllegalArgumentException` for a value not in
  `TARGETS`. `new MatchScore()` is `new MatchScore(1000)`. `target()` returns
  it. The class doc comment states the rule in terms of the target.
- `BotGame` gains `int target`.
- `PrivateTable` gains `target` (1,000 by default) and
  `setTarget(UUID playerId, int target)`, with the same guards as
  `setAnnouncements`:
  - public table: "Public tables always play to 1,000 points.";
  - not the owner: "Only the table owner can change the match target.";
  - game started: "The match target cannot change after the game starts.";
  - each a `PrivateTableConflictException` (409).

## Services and API

- `BotGameService.create` and `createPassAndPlay` take the target and build
  the `MatchScore` with it; `rematch` keeps `game.target()`.
- `PrivateTableService.setTarget(tableId, token, target)`; the table's
  `MatchScore` (start and rematch) uses `table.target()`.
- `POST /api/bot-games` (solo and pass-and-play) takes an optional
  `target` (`Integer`, 1,000 when absent).
- `POST /api/private-tables/{id}/target` takes `{playerToken, target}` and
  returns the table.
- Responses: the game response, the private table response and both match
  status responses (`BotGameService.MatchStatus`,
  `PrivateTableService.MatchStatus`) carry `target`.
- A target outside `TARGETS` in a request gives 400 with "The match target
  must be 500, 1,000, 1,500 or 2,000 points."

## History, export and statistics

- `V10__match_target.sql`:
  `ALTER TABLE match_record ADD COLUMN target INTEGER NOT NULL DEFAULT 1000;`
  Matches recorded before read as played to 1,000.
- `FinishedMatch` and `MatchRecord` carry `target`; both recorders pass it.
- The data export adds `target` to each match and stays `beelot-export-2`
  (fields are only added).
- `PlayerStatistics.RecentMatch` gains `target`; the statistics page's recent
  matches table gains a "Target" column.

## Client

- A "Match target" button group (US-028 style, like the announcements option)
  with 500, 1,000 (checked), 1,500 and 2,000:
  - in the solo and pass-and-play setup, sent with the create request;
  - in the private-table lobby: the owner changes it before the start through
    the new endpoint; other players see the choice read-only; a public table
    doesn't show it.
- The score line reads "Match score — North–South {0} · East–West {1} · to
  {2}" (solo, pass-and-play and private tables).
- The rules sentence becomes: "The first team to reach the match target
  (1,000 points unless the players choose 500, 1,500 or 2,000) wins the match.
  When both teams reach it in the same round, the higher score wins; an exact
  tie is settled by another round."
- The home tagline "race to 1,000 points" and the Classic variant line
  ("played to 1,000 points") stay: 1,000 is still the default.
- Every new string in English, French and Dutch.

## Error handling

- Invalid target in a request: 400 with the message above.
- Private table guards: 409 with the messages above; the client shows them in
  the lobby message area, as for announcements.

## Testing

- `MatchScore`: each target ends the match when reached; a tie at 500 plays on;
  an invalid target throws; the default is 1,000.
- `PrivateTable`: the owner sets the target before the start; the guards.
- `BotGameService` / `PrivateTableService`: the next round and a rematch keep
  the target; the match ends at 500.
- Controllers: default 1,000; `target` in the responses; 400 on 750.
- History: a recorded match keeps its target; V10 reads old rows as 1,000; the
  export and the statistics' recent matches carry it.
- Client resource test: the button groups, the score line and the
  translations.
- Headless browser: choose 500 in the solo setup and see "to 500" in the score
  line.

## Out of scope

- A "by target" statistics breakdown.
- Targets for public matchmaking or the arena.
- Changing the target during a match.
