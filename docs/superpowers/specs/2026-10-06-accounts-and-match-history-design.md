# Accounts, match history and statistics (US-018) — design

## Goal

As a player, I want match history and statistics, so that I can follow my
progress.

History needs to know who the player is across games and devices, so accounts
come first. US-018 is split into three stories, delivered in order:

1. **US-057 — Sign in with an OAuth provider**
2. **US-058 — Record finished matches**
3. **US-059 — See my match history and statistics**

## Decisions

- **Sign-in with OAuth only** (Google and GitHub). The app stores no passwords
  and sends no email. A provider is offered only when its client id and secret
  are configured.
- **Guests keep playing.** Every mode works as today without signing in. Only
  signed-in players get their matches recorded.
- **MariaDB** (ADR-002; PostgreSQL as first planned in ADR-001): Spring Data
  JPA with Flyway migrations. Developers run it with Docker Compose
  (`compose.yaml`); the tests start it with Testcontainers, so
  `./gradlew test` needs a running Docker daemon.
- **Server-side session.** Signing in creates an HTTP session (cookie
  `JSESSIONID`, `HttpOnly`, `SameSite=Lax`). Sessions live in memory: a
  restart signs everyone out, like it already ends every game.
- **CSRF protection** once sessions exist: Spring Security's cookie token
  (`XSRF-TOKEN`), which `app.js` sends back as the `X-XSRF-TOKEN` header on
  every non-GET request. Game endpoints stay open to guests.
- **Data minimisation.** An account stores the provider, the provider's user
  id, a display name and timestamps. No email and no avatar.
- **All finished matches count**: solo games against bots, private tables and
  public tables. A match is finished when a team reaches 1,000 points (US-046).
  Abandoned matches are not recorded.

## US-057 — Sign in with an OAuth provider

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
- Accounts are stored in MariaDB, created by a Flyway migration.
- Provider credentials and the database come from environment variables,
  documented in the README with the Docker Compose setup.
- New texts are translated into French and Dutch.

## US-058 — Record finished matches

As a signed-in player, I want my finished matches recorded, so that I can look
back at them.

Acceptance criteria:

- A seat remembers the account of the signed-in player who took it (solo
  game, table creation, joining by code, quick match).
- When a match with at least one signed-in player ends, the server stores:
  - when it ended, the mode (solo, private, public), the variant, and the
    difficulty for solo games;
  - each seat: its account if any, its name, its team, and whether a bot
    played it;
  - the final score of each team and the winning team;
  - each round: the declaring team, the contract and trump, whether it was
    made, coinched, a capot, which team had the belote, and the points each
    team scored.
- A match is stored once, even if it is ended from several requests.
- Matches without a signed-in player, and abandoned matches, are not stored.

## US-059 — See my match history and statistics

As a signed-in player, I want to see my results, so that I can follow my
progress.

Acceptance criteria:

- A "My statistics" page, reachable from the home screen when signed in,
  shows:
  - matches played, won and lost, and the win rate: overall, per variant, and
    for solo games per difficulty;
  - the 20 most recent matches: date, mode, variant, partner and opponents,
    final score and result;
  - contracts: taken by the player's team and made, coinches by and against
    the team and how many succeeded, capots and belotes;
  - the average points per round of the player's team.
- A guest who opens the page is invited to sign in.
- A player only ever sees their own statistics.
- New texts are translated into French and Dutch.

## Out of scope

Password sign-in, email, account deletion and data export (to be decided with
the privacy requirements), linking several providers to one account,
leaderboards, and history for guests.
