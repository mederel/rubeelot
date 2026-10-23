# Rubeelot

The Rust backend of [Beelot](https://github.com/mederel/beelot): French Belote
and Contrée in the browser, against bots, around one device, or online with
friends.

This repository is being built to replace Beelot's Java / Spring Boot backend.
Until the cutover, [`mederel/beelot`](https://github.com/mederel/beelot) remains
the product. The design is in
[`docs/superpowers/specs/2026-10-23-rust-backend-design.md`](docs/superpowers/specs/2026-10-23-rust-backend-design.md).

## Develop

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

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
