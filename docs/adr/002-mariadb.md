# ADR-002: Use MariaDB for persistent data

- Status: Accepted
- Date: 2026-10-06
- Decision makers: Product and engineering
- Supersedes: the PostgreSQL choice in ADR-001

## Context

ADR-001 chose PostgreSQL for accounts and completed match records. The first
persistent data (accounts, US-057, and match history, US-058) was built on
PostgreSQL. The product owner then asked to run on MariaDB instead.

## Decision

Store persistent data in **MariaDB 11.4** (LTS), still through Spring Data
JPA/Hibernate with Flyway migrations:

- The MariaDB Connector/J driver and Flyway's MySQL/MariaDB support.
- Identifiers use MariaDB's native `UUID` type; times are `DATETIME(6)` in UTC
  (`hibernate.jdbc.time_zone=UTC`).
- Docker Compose runs MariaDB for development, and the tests start it with
  Testcontainers.

The migrations were rewritten for MariaDB rather than added on top: no
production database existed yet.

## Consequences

- Schema features stay within what both MariaDB and Hibernate's MariaDB
  dialect support; time zones are handled by the application, since
  `DATETIME` stores none.
- A local PostgreSQL volume from earlier development (`beelot_beelot-postgres`)
  is no longer used and can be removed.
