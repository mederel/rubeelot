# Rubeelot

The Rust backend of [Beelot](https://github.com/mederel/beelot): French Belote
and Contrée in the browser, against bots, around one device, or online with
friends.

This repository is being built to replace Beelot's Java / Spring Boot backend.
Until the cutover, [`mederel/beelot`](https://github.com/mederel/beelot) remains
the product. The design is in
[`docs/superpowers/specs/2026-10-23-rust-backend-design.md`](docs/superpowers/specs/2026-10-23-rust-backend-design.md).

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

## Bot arena

The arena plays two bots against each other on seeded duplicate deals: every
deal is played twice with the same cards, once with each bot as North–South,
and the report gives the point difference per deal with its 95% confidence
interval, the matches won, contract and coinche success, and decision times.

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
| `--report`        | the report file                          | `target/bot-arena/report.txt` |

The arena bots are `current`, `challenging`, `baseline`, `previous-play`,
`previous-trumps`, `no-coinche`, `no-announcement-bidding` and `random`. The
command prints each variant's report and writes them to the report file.

The double-dummy solver behind the Challenging bot has its own benchmark:

```sh
cargo bench -p beelot-game --bench solver
```
