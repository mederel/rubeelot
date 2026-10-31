# Repository Guidelines

## Project status

This repository holds Beelot's Rust backend and its frontend
(`docs/adr/001-rust-platform.md`). The design is in
`docs/superpowers/specs/2026-10-23-rust-backend-design.md`. The Cargo workspace is in
`crates/`: `beelot-game` (rules and bots), `beelot-arena` (the bot arena CLI) and
`beelot-server` (the axum server, which serves the frontend in its `static/`
folder). `fixtures/` holds recorded deals (see `fixtures/SOURCE`).

Run the server with `docker compose up -d --wait && cargo run -p beelot-server`;
its configuration variables are listed in the README.

## Build, test and format

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

CI (`.github/workflows/build.yml`) runs the same three commands, the tests
under `cargo llvm-cov` to report their coverage on the pull request. The server's
database tests start MariaDB with Testcontainers: Docker must be running.

## Working conventions

- Keep changes focused on the requested task.
- Prefer clear, idiomatic code and small, reviewable commits.
- Do not modify generated files unless the task explicitly requires it.
- Preserve existing user changes; avoid destructive Git operations.

## Before submitting changes

- Run the relevant formatter, tests, and build once they are available.
- Report commands that were run and any checks that could not be run.
- Document new setup steps or developer workflows in the README when added.
