---
name: writing-rust
description: Writing or modifying Rust code in the clean-rust workspace. Use when creating or editing .rs files, Cargo.toml members, migrations, or tests — covers Clean Architecture layering, error handling patterns, module layout, and the lint/test loop.
---

# Writing Rust in clean-rust

Read `AGENTS.md` at the repo root first — it holds the hard rules (no commits without permission, `issue-N/short-description` branches only, no secrets anywhere, scratch files in `.agents/temp/`, never `/tmp`).

Adding a new HTTP endpoint to an API app? Follow the `adding-an-endpoint` skill — it expands the inside-out workflow below into the endpoint-specific playbook (DTO naming, OpenAPI wiring, validation, error mapping, per-handler test layout).

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

## Naming and structure — suffixes carry the layer

- **Domain models** (in `crates/domains/<domain>/src/models`) end in `Model`: `TodoModel`, `NewTodoModel`, `TodoUpdateModel`.
- HTTP-side naming and placement (DTOs mirroring `Model` names as `Http`, `Query`/path structs, colocating structs with their handler) is endpoint-specific — see the `adding-an-endpoint` skill.

## Comments — what belongs in code

- **Rustdoc is expected and lint-enforced**: the workspace sets `missing_docs = "warn"` (`[workspace.lints.rust]` in the root `Cargo.toml`, inherited by every crate) and CI fails on warnings, so every public item — including fields, enum variants, trait methods, and binary/test crate roots — needs a doc comment. Never add `#[allow(missing_docs)]`; document the item instead. Keep `# Errors` sections on fallible functions and `# Panics` on panicking ones. Intra-doc links on items visible at a crate root must be crate-qualified (`` [`storage::StorageError`] `` style, or `` [`crate::…`] ``) — docs on re-exported items re-render at crate-root scope, where short links silently break; verify with `cargo doc --no-deps -p <crate>`.
- **No "decision" comments**: never narrate choices, history, or rationale in code ("agreed rule:", "deliberately untouched", "mirrors the existing pattern", "we chose X because Y"). If a decision needs recording, it goes in the plan/notes, not the source.
- **No third-party API documentation comments**: never paraphrase what a framework, crate, or database feature does (e.g. what `AssertSqlSafe` means, what a Postgres function returns, how sea-query renders an expression). The code and upstream docs cover it.
- **Explanatory comments are opt-in**: anything beyond rustdoc must be explicitly planned and approved ahead of time (list the comment in the plan task's `Work:` section). Treat any unplanned explanatory comment as a review finding.

## Patterns

- **Imports**: four groups separated by blank lines, in this order — std library, third-party crates, internal workspace crates (`todos`, `database`, `telemetry`, `storage`, `utils`, `web`), then the current crate (`crate::` / `super::`).
- **Error types**: one dedicated file per crate/layer named `<thing>_error.rs`, using `thiserror`. When wrapping a source error, interpolate `{source}` in the `#[error(...)]` message so diagnostics survive `Display`. If a struct-variant split (like `DbError` → `StorageError` classification) is intentional, document it at the top of the file. (The app-layer HTTP error envelope convention is endpoint-specific — see `adding-an-endpoint`.)
- **Modules**: multi-file modules use the file-and-folder pattern — `mod.rs`-style parent file (`repositories.rs`) alongside its directory (`repositories/todos_repository.rs`). New sibling modules should match.
- **Validation**: single source of truth. If a domain model validates (e.g. min length), don't re-encode the rule in the delivery layer (or make them agree); error messages must match the actual rule (`EmptyTask` shouldn't be raised when the real rule is "more than one character").
- **Dependencies**: workspace deps come from the root `Cargo.toml` `[workspace.dependencies]` — reference with `workspace = true`. Before proposing a new third-party dependency, check that no existing workspace dependency already covers the need. Looking up the latest version of a crate (crates.io/docs.rs) is fine, but never add or upgrade a dependency without explicit human permission — propose the crate, version, and reason first, then wait. Never declare a dep a crate doesn't use (unused `uuid`, `thiserror`, `axum` were all flagged). Don't hand-edit `Cargo.lock`; CI builds `--locked`.
- **Magic numbers** in shared plumbing (timeouts, limits): accept them as parameters or name them as constants — no bare literals with no configuration path.

## Before you finish

Run from the repo root (tooling needs direnv active — see `AGENTS.md`):

1. `just cargo-fmt` — zero fmt drift (CI enforces `cargo fmt --all --check`; fix drift with `just cargo-fmt-fix`).
2. `just cargo-lint` — clippy pedantic with `-D warnings` (also enforces the workspace `missing_docs` lint), zero warnings. Mid-task, when the workspace is intentionally red (e.g. a trait method awaiting its impl), scope it: `just cargo-lint <crate>`.
3. `just cargo-check <crate> *args` — plain type-check of one crate for the fast loop (e.g. the designed intermediate states between port and impl tasks).
4. `just cargo-test <crate>` for every crate you touched. Integration tests need Postgres up: `just infrastructure up -d --wait`.
5. If you added/changed migrations: `just migrate <domain> run` (revert with `just migrate <domain> revert [-- --target-version <n>]`).

Use the `just` recipes above instead of bare `cargo` commands — they wire in env files and keep one place for flags.

Never commit anything. `.agents/temp/` is your scratch/output area — put notes, plans, and generated artifacts there, never in `coverage/`, `target/`, or the repo root.
