# AGENTS.md

## Project

`clean-rust` is a Rust workspace (edition 2024) demonstrating Clean Architecture. Three layers with dependency direction apps → domains → libs:

- `crates/apps/` — deployables: `todos-api` (axum HTTP API), `email-worker`, `ops-cli`
- `crates/domains/` — business logic (e.g. `todos`): use_cases define repository ports, infrastructure implements them (sqlx + sea-query), migrations live in `crates/domains/<domain>/migrations`
- `crates/libs/` — shared libraries: `web`, `database`, `storage`, `telemetry`, `utils`

Stack: axum, sqlx (Postgres), sea-query, opentelemetry, tracing, thiserror. Clippy `pedantic` lints are enabled workspace-wide and CI fails on warnings.

## Hard rules

- Never commit without explicit human permission.
- Never commit to the `main` branch. Commit only to branches named `issue-N/short-description`, where `N` is a GitHub issue number supplied by the human.
- No sensitive secrets may ever be added to the repo — not even in `.agents/temp/`, env files, or scratch files.
- Never add or upgrade a dependency without explicit human permission; looking up the latest version is fine.
- Always use `.agents/temp/` as a scratch/output area; never write to the system `/tmp`.

## Commands

Tooling is managed via `direnv` (`.envrc` sets `CARGO_INSTALL_ROOT=tools/cargo` and adds `tools/cargo/bin` to `PATH`); run `just init` to install sqlx-cli, watchexec, and cargo-llvm-cov there.

| Task | Command |
|---|---|
| Lint | `just cargo-lint` (clippy, `-D warnings`) |
| Test one crate | `just cargo-test <crate>` |
| Test all | `just cargo-test-all` |
| Coverage | `just cargo-cover` (HTML + lcov in `coverage/`) |
| Run an app with hot reload | `just cargo-run <app> [args]` — valid `<app>` names are the crate names found in `crates/apps/**/Cargo.toml` (currently `todos-api`, `email-worker`, `ops-cli`) |
| TDD loop | `just cargo-tdd <crate>` |
| Migrations | `just migrate <domain> <action>` — valid `<action>` values are `run` and `revert` (`revert --target-version 0` reverts all) |
| Postgres shell | `just psql <database>` |
| Compose up/down | `just infrastructure up -d` / `down` |

`just` recipes source env vars from `tools/envs/<name>.env` when the file exists (e.g. `todos-api.env`, `test.env`); crates without env files run plain — don't add empty env files as ceremony. CI (`.github/workflows/ci.yaml`) runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` against docker-compose services — so keep formatting clean and pass `--locked` semantics (don't hand-edit `Cargo.lock`).

## Infrastructure

`infrastructure/docker-compose.yaml` provides local dev services:

- **postgres** (18, tmpfs storage) on `127.0.0.1:5432` — dev credentials only, not secrets
- **pgweb** UI on `127.0.0.1:8081`
- **opentelemetry-collector** on `4317`/`4318` (gRPC/HTTP)

Tests require the compose services running: `just infrastructure up -d --wait`.

## Conventions

- Error types live in dedicated files named `<thing>_error.rs` (e.g. `db_error.rs`, `storage_error.rs`, `api_error.rs`).
- Multi-file modules use the file-and-folder pattern (`use_cases/repositories.rs` + `use_cases/repositories/todos_repository.rs`).
- Domain → HTTP error mapping belongs in the app layer (`app_error.rs`), not in shared libs.
- `.agents/temp/` is gitignored scratch space for agent outputs.
