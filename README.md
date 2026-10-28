# Beelot

[![Build](https://github.com/mederel/rubeelot/actions/workflows/build.yml/badge.svg)](https://github.com/mederel/rubeelot/actions/workflows/build.yml)
![Rust 1.95](https://img.shields.io/badge/Rust-1.95-orange?logo=rust)
![axum](https://img.shields.io/badge/axum-tokio-blue)
![French, English, Dutch](https://img.shields.io/badge/languages-FR%20%C2%B7%20EN%20%C2%B7%20NL-purple)

**Deal in. Good times await.** French Belote and Contrée in the browser:
against sharp bots, around one device, or online with friends.

| | |
| --- | --- |
| **Two games, one table** | Classic Belote with upcard bidding, and Contrée with an 80–160 auction and coinche. |
| **Bots worth beating** | Relaxed bots play by the rules of thumb; Challenging bots sample hidden hands and solve them double-dummy before every card. |
| **Play your way** | Quick games against bots, pass-and-play on one device, private tables for your crew, or public matchmaking. |
| **Your story in numbers** | Sign in to keep every match: wins per variant and difficulty, contracts, coinches, capots and belotes. |
| **No passwords, ever** | Sign in with a passkey, or just play as a guest. No email address asked. |
| **Privacy built in** | Download or delete your data from Settings, and a clear privacy policy. |
| **Speaks your language** | French, English and Dutch, with sound effects and reduced-motion settings. |

This repository is Beelot's Rust backend and its frontend. It replaced the Java /
Spring Boot backend of [`mederel/beelot`](https://github.com/mederel/beelot);
[ADR-001](docs/adr/001-rust-platform.md) records the decision and
[the design](docs/superpowers/specs/2026-10-23-rust-backend-design.md) the
details.

## Quick start

The server needs MariaDB; `compose.yaml` starts one with the server's defaults.

```sh
docker compose up -d --wait
cargo run -p beelot-server
```

Then open <http://localhost:8080>. The server applies its database migrations at
startup, serves the frontend from `crates/beelot-server/static/`, and stops
cleanly on Ctrl-C.

## Configuration

The server reads these variables from the environment, or from a `.env` file in
the directory it runs from:

| Variable                         | Default                         | Meaning                                                    |
| -------------------------------- | ------------------------------- | ---------------------------------------------------------- |
| `BEELOT_BIND`                    | `0.0.0.0:8080`                  | the address the server listens on                          |
| `BEELOT_DATABASE_URL`            | `mysql://localhost:3306/beelot` | the MariaDB database                                       |
| `BEELOT_DATABASE_USERNAME`       | `beelot`                        | its user                                                   |
| `BEELOT_DATABASE_PASSWORD`       | `beelot`                        | its password                                               |
| `BEELOT_PUBLIC_URL`              | `http://localhost:8080`         | the address players use: the passkeys' relying party       |
| `BEELOT_TRUST_FORWARDED_HEADERS` | `false`                         | take the client address from `X-Forwarded-For` (behind a proxy) |
| `BEELOT_STATIC_DIR`              | the crate's `static/`           | the frontend's files                                       |
| `BEELOT_PRIVACY_CONTROLLER`      | unset                           | the data controller named in the privacy policy            |
| `BEELOT_PRIVACY_CONTACT`         | unset                           | the privacy contact                                        |
| `BEELOT_HOSTING_PROVIDER`        | unset                           | the hosting provider                                       |
| `BEELOT_BACKUP_RETENTION_DAYS`   | unset                           | how many days backups are kept                             |

The privacy policy shows "not configured" for each privacy value left unset, and
the server lists them in a warning at startup. Logs go to standard output; set
`RUST_LOG` (for example `RUST_LOG=beelot_server=debug`) to change their level.

## Develop

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

The server's database tests start a MariaDB container with Testcontainers, so
Docker must be running. Building the server needs OpenSSL's development files
(`libssl-dev` on Debian and Ubuntu), which `webauthn-rs` links against.

The snapshots the table API sends are pinned by the files in
`crates/beelot-server/tests/contract/`. After an intended change, rewrite them
with `UPDATE_CONTRACT=1 cargo test -p beelot-server --test api_contract` and
review the diff.

## Verify

Before a release, check in a browser, with the console open, that these work
without errors:

- a solo Classic game to the end of a round, and a solo Contrée game with a
  coinche;
- pass-and-play with two humans;
- a private table in two browsers: create it, join by code, choose seats, get
  ready, start, play a trick (both update without reloading) and send a
  reaction;
- quick-match in one browser: bots take the empty seats after 60 seconds;
- closing one browser at a private table: its seat shows a bot 2 minutes later;
- passkey sign-up, sign-out and sign-in, statistics after a finished match, the
  data export and the account deletion.

## Release build

```sh
cargo build --release
```

The server is `target/release/beelot-server`. It finds the frontend in the
crate's `static/` folder at the path it was built from; when the binary runs
elsewhere, copy `crates/beelot-server/static/` next to it and point
`BEELOT_STATIC_DIR` at the copy.

## Bot arena

The arena plays two bots against each other on seeded duplicate deals: every
deal is played twice with the same cards, once with each bot as North–South,
and the report gives the point difference per deal with its 95% confidence
interval, the matches won, contract and coinche success (and the tout-atout and
sans-atout contracts on their own), and decision times.

```sh
cargo run --release -p beelot-arena -- --first current --second random
```

| Option            | Values                                   | Default                       |
| ----------------- | ---------------------------------------- | ----------------------------- |
| `--first`         | an arena bot                             | `current`                     |
| `--second`        | an arena bot                             | `random`                      |
| `--deals`         | duplicate deal pairs, at least 2         | `10000`                       |
| `--seed`          | the seed the deals are dealt from        | `1`                           |
| `--variant`       | `classic`, `contree` or `both`           | `both`                        |
| `--announcements` | `on` or `off`                            | `on`                          |
| `--all-trumps`    | Contrée tout-atout and sans-atout        | `on`                          |
| `--dealing`       | `shuffled` or `table` (see below)        | `shuffled`                    |
| `--report`        | the report file                          | `target/bot-arena/report.txt` |

Games deal like at a real table: only a match's first deal is shuffled, and
each later deal is cut from the previous deal's gathered tricks. The arena
shuffles every deal pair by default, so that bot strength is measured on
independent hands; `--dealing table` deals each pair from the previous pair's
gathered cards instead.

The arena bots are `current`, `challenging`, `baseline`, `previous-play`,
`previous-trumps`, `previous-all-trumps-play`, `no-coinche`,
`no-announcement-bidding`, `no-all-trumps-bidding` and `random`. The command
prints each variant's report and writes them to the report file.

The bots' tout-atout and sans-atout estimates were fitted on bot play by least
squares. To fit them again:

```sh
cargo run --release -p beelot-arena --bin fit-estimates -- --stage offence --deals 20000 --seed 1
```

`--stage offence` fits the taker's estimate and `--stage defence` the
defence's, which the coinche uses. The command prints each feature's weight
with its standard error and the fit's R²; the rounded weights go into
`crates/beelot-game/src/bot/rule_based.rs`.

## Solver benchmark

The double-dummy solver behind the Challenging bot has its own benchmark:

```sh
cargo bench -p beelot-game --bench solver
```
