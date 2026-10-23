# Repository Guidelines

## Project status

This repository holds the Rust rewrite of the Beelot backend. The design is in
`docs/superpowers/specs/2026-10-23-rust-backend-design.md`. The Java backend it
replaces lives in https://github.com/mederel/beelot. The Cargo workspace is in
`crates/`; `fixtures/` holds data recorded from the Java implementation
(see `fixtures/SOURCE`).

## Build, test and format

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

CI (`.github/workflows/build.yml`) runs the same three commands.

## Working conventions

- Keep changes focused on the requested task.
- Prefer clear, idiomatic code and small, reviewable commits.
- Do not modify generated files unless the task explicitly requires it.
- Preserve existing user changes; avoid destructive Git operations.

## Before submitting changes

- Run the relevant formatter, tests, and build once they are available.
- Report commands that were run and any checks that could not be run.
- Document new setup steps or developer workflows in the README when added.
