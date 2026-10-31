# ADR-001: Use Rust for the game server

- Status: Accepted
- Date: 2026-10-28
- Decision makers: Product and engineering
- Amends: ADR-002 (MariaDB): `sqlx` and embedded migrations replace the previous data-access layer

## Context

Beelot's first backend was a modular monolith. It
grew solo games, pass-and-play, private tables, public matchmaking, passkey
accounts, match history and statistics, GDPR export, deletion and retention,
and a bot arena. Its game state lived in several parallel maps, online tables
polled every 3 seconds, and the Challenging bot's double-dummy solver set the
pace of the whole server.

The product owner asked to rewrite the backend in Rust with the same features.
There was no production data to keep.

## Decision

Build the backend in **Rust** (edition 2024, toolchain pinned in
`rust-toolchain.toml`) as a Cargo workspace in this repository, started with a
fresh history:

- `beelot-game`: rules, scoring, announcements, bots and the solver, with no
  I/O and no async. It is the sole authority for dealing, bidding, legal moves,
  trick resolution and scoring.
- `beelot-arena`: the bot arena CLI.
- `beelot-server`: an axum and tokio server that serves the static frontend.

The server keeps one table model for solo, pass-and-play and online games,
held in memory. Online tables push snapshots over **Server-Sent Events** in
place of the poll. Passkeys go through `webauthn-rs`.

MariaDB stays (ADR-002), reached through **`sqlx`** with its MySQL
driver and embedded migrations. The schema starts again from one fresh
migration.

Bots decide from their seat's view alone: the same seed and the same view give
the same bid and the same card. Recorded deals (`fixtures/`) check the
sampling bot's choices.

## Considered options

### Keep the previous stack

Rejected at the product owner's request.

### A big-bang rewrite on a long-lived branch of the previous repository

Rejected: one huge PR, and features landing in the old backend meanwhile would
be ported twice. The rewrite went in three phases instead (rules and bots, server,
cutover), each its own set of commits.

### Port the server first, calling the old rules over FFI

Rejected: the glue would be thrown away entirely.

### WebSocket for table updates

Rejected: nothing needs to travel from the client over the socket, and SSE is
simpler to build, test and proxy.

### OpenAPI

Rejected: there is one client, shipped from the same repository. Contract
tests (`crates/beelot-server/tests/contract/`) pin the snapshots instead.

## Consequences

- One binary serves the API and the frontend.
- The rules crate has no framework dependency and is tested on its own,
  including against recorded deals.
- Tables are not kept across restarts, as before, and the server still runs as
  a single instance.
- Configuration keeps its environment variables; `BEELOT_DATABASE_URL` is now a
  `mysql://` URL.
- Rules and bot changes are made here.

## Revisit when

Reassess if the product needs several server instances, tables that survive a
restart, or native mobile clients.
