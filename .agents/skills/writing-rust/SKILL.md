---
name: writing-rust
description: Writing or modifying Rust code in the clean-rust workspace. Use when creating or editing .rs files, Cargo.toml members, migrations, or tests — covers Clean Architecture layering, error handling patterns, module layout, and the lint/test loop.
---

# Writing Rust in clean-rust

Read `AGENTS.md` at the repo root first — it holds the hard rules (no commits without permission, `issue-N/short-description` branches only, no secrets anywhere, scratch files in `.agents/temp/`, never `/tmp`).

## Placement — decide the layer before writing code

Dependency direction is strictly apps → domains → libs. Put new code where its dependencies point outward:

- **HTTP/framework/CLI concerns** (axum handlers, extractors, config parsing, `main`) → `crates/apps/<app>`
- **Business rules, ports, use cases** → `crates/domains/<domain>/src/use_cases`
- **Domain models and invariants** (validation like `has_valid_task`) → `crates/domains/<domain>/src/models`
- **Database implementations** (sqlx, sea-query) → `crates/domains/<domain>/src/infrastructure`
- **Generic reusable plumbing** (web middleware, error envelopes, telemetry, pagination) → `crates/libs/*`

Leaks to avoid (found in past reviews):

- No sqlx/postgres types in use_cases — use case code sees only its repository trait.
- No domain-specific types in shared libs — e.g. `CursorError::SortMismatch` existing only for todos in `libs/web` was flagged; add a config knob or keep the concern in the app instead.
- Domain → HTTP status mapping belongs in the app's `app_error.rs`, never in `libs/web` (`From<StorageError>` impls hard-coding 503/409/500 there were flagged).
- No UUID-version or ID-scheme checks in use cases (`get_version_num() != 7` conflated "bad ID" with "not found" — don't repeat it).

## Patterns

- **Imports**: four groups separated by blank lines, in this order — std library, third-party crates, internal workspace crates (`todos`, `database`, `telemetry`, `storage`, `utils`, `web`), then the current crate (`crate::` / `super::`).
- **Error types**: one dedicated file per crate/layer named `<thing>_error.rs`, using `thiserror`. When wrapping a source error, interpolate `{source}` in the `#[error(...)]` message so diagnostics survive `Display`. If a struct-variant split (like `DbError` → `StorageError` classification) is intentional, document it at the top of the file.
- **HTTP error envelope**: the app-layer `ApiError` is deliberately NOT a `thiserror` type — it maps domain errors via `From` impls at the boundary. Follow that; don't invent a second style.
- **Modules**: multi-file modules use the file-and-folder pattern — `mod.rs`-style parent file (`repositories.rs`) alongside its directory (`repositories/todos_repository.rs`). New sibling modules should match.
- **Validation**: single source of truth. If a domain model validates (e.g. min length), don't re-encode the rule in the HTTP layer (or make them agree); error messages must match the actual rule (`EmptyTask` shouldn't be raised when the real rule is "more than one character").
- **Dependencies**: workspace deps come from the root `Cargo.toml` `[workspace.dependencies]` — reference with `workspace = true`. Before proposing a new third-party dependency, check that no existing workspace dependency already covers the need. Looking up the latest version of a crate (crates.io/docs.rs) is fine, but never add or upgrade a dependency without explicit human permission — propose the crate, version, and reason first, then wait. Never declare a dep a crate doesn't use (unused `uuid`, `thiserror`, `axum` were all flagged). Don't hand-edit `Cargo.lock`; CI builds `--locked`.
- **Magic numbers** in shared plumbing (timeouts, limits): accept them as parameters or name them as constants — no bare literals with no configuration path.

## Before you finish

Run from the repo root (tooling needs direnv active — see `AGENTS.md`):

1. `just cargo-lint` — clippy pedantic with `-D warnings`; zero warnings, zero fmt drift (`cargo fmt --all --check` is CI-enforced).
2. `just cargo-test <crate>` for every crate you touched. Integration tests need Postgres up: `just infrastructure up -d --wait`.
3. If you added/changed migrations: `just migrate <domain> run` (revert with `just migrate <domain> revert [-- --target-version <n>]`).

Never commit anything. `.agents/temp/` is your scratch/output area — put notes, plans, and generated artifacts there, never in `coverage/`, `target/`, or the repo root.
