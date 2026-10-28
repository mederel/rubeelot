# ADR-001: Use Rust for the game server

- Status: Accepted
- Date: 2026-10-28
- Decision makers: Product and engineering
- Supersedes: `beelot`'s ADR-001 (Java 21 and Spring Boot)
- Amends: `beelot`'s ADR-002 (MariaDB): `sqlx` replaces JPA/Hibernate and Flyway

## Context

Beelot's backend was a Java 21 / Spring Boot 4 modular monolith
([`mederel/beelot`](https://github.com/mederel/beelot), ADR-001 there). It
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

MariaDB stays (`beelot`'s ADR-002), reached through **`sqlx`** with its MySQL
driver and embedded migrations in place of JPA/Hibernate and Flyway. The schema
starts again from one fresh migration.

Bots must reproduce Java's decisions exactly: the same seed and the same view
give the same bid and the same card. Fixtures recorded from the Java
implementation (`fixtures/`) prove it.

## Considered options

### Keep Java and Spring Boot

Rejected at the product owner's request.

### A big-bang rewrite on a long-lived branch of `beelot`

Rejected: one huge PR, and features landing in Java meanwhile would be ported
twice. The rewrite went in three phases instead (rules and bots, server,
cutover), each its own set of commits.

### Port the server first, calling the Java rules over FFI

Rejected: the glue would be thrown away entirely.

### WebSocket for table updates

Rejected: nothing needs to travel from the client over the socket, and SSE is
simpler to build, test and proxy.

### OpenAPI

Rejected: there is one client, shipped from the same repository. Contract
tests (`crates/beelot-server/tests/contract/`) pin the snapshots instead.

## Consequences

- One binary, with no JVM, serves the API and the frontend.
- The rules crate has no framework dependency and is tested on its own,
  including against the Java fixtures.
- Tables are not kept across restarts, as before, and the server still runs as
  a single instance.
- Configuration keeps its environment variables; `BEELOT_DATABASE_URL` is now a
  `mysql://` URL.
- Rules or bot changes must now be made here; `beelot` is no longer the
  product.

## Revisit when

Reassess if the product needs several server instances, tables that survive a
restart, or native mobile clients.
