---
name: adding-an-endpoint
description: Adding a new HTTP endpoint to any API app in the clean-rust workspace (todos-api or any future app). Use when implementing a new route or handler — covers the domain-first workflow, DTO naming, schema validation wiring, OpenAPI descriptions, error mapping, migrations, and the per-handler test layout.
---

# Adding an HTTP endpoint in clean-rust

Read `AGENTS.md` at the repo root first, then the `writing-rust` skill (naming, layering, error patterns, verification loop). This skill is the endpoint-specific playbook for **any** API app — substitute your `<domain>` and `<app>` for `todos`/`todos-api` throughout.

Work **inside-out**: domain → infrastructure → delivery → tests. Each step is a natural task boundary; the dependency direction apps → domains → libs must hold at every step.

## 1. Domain — the use case and its port

- Define/extend the use case in `crates/domains/<domain>/src/use_cases` and, if the use case needs persistence, its repository port using the file-and-folder pattern (`use_cases/repositories.rs` + `use_cases/repositories/<thing>_repository.rs`). The port speaks in domain models and returns `Result<_, TodoError>`-style domain errors — never sqlx/axum/HTTP types.
- Add/extend model types in `crates/domains/<domain>/src/models`, named `*Model` (`TodoModel`, `NewTodoModel`, `TodoUpdateModel`). Put invariants on the model (e.g. `has_valid_task`) — one source of truth for validation; the HTTP layer must not re-encode rules or disagree with them.
- Extend the domain error (`use_cases/<thing>_error.rs`, thiserror) with any new variant. New error variants need an HTTP mapping later — plan for it.
- Document every public item as you go (workspace `missing_docs` lint is fatal under CI); business rules and their tests live here.

## 2. Infrastructure — implement the port

- Implement the port in `crates/domains/<domain>/src/infrastructure` with sqlx + sea-query. Repository errors go in a dedicated `<thing>_error.rs` file (classify and wrap the sqlx error; interpolate `{source}` in `#[error(...)]` messages so diagnostics survive `Display`).
- If the schema changes, add a migration under `crates/domains/<domain>/migrations`: `just migrate <domain> new` to create, `just migrate <domain> run` to apply (revert with `just migrate <domain> revert [-- --target-version <n>]`).
- Dependency direction check: infrastructure may depend on the domain's use_cases (to see the port), never the reverse.

## 3. Delivery — the app layer

In `crates/apps/<app>/src`, working through four files:

### controller.rs — handler + DTOs

- Name the DTOs after the domain models: `NewTodoModel` → `NewTodoHttp`, `TodoModel` → `TodoHttp`, `TodoUpdateModel` → `TodoUpdateHttp` (no other morphemes — `TodoPutHttp` would be wrong; the verb lives in the route). Query structs end in `Query` (`ListTodosQuery`); add a path struct when the route has path params (`TodoPath`).
- Colocate each HTTP struct immediately above the handler that uses it, not grouped at the top of the file.
- Doc-comment the handler and **every** DTO type and field: `missing_docs` enforces this, and — via schemars — the doc comments become the OpenAPI schema and parameter descriptions. Field docs should state units, defaults, bounds, and nullability semantics.
- Validate request bodies through the app's schema validators: use `ValidatedJson<T>` / `ValidatedQuery<T>` / `ValidatedPath<T>` from `web` (they enforce JSON-schema rules derived from the DTO via `HasSchema`, configured in `AppState`). Rejections surface as the documented 422 `ValidationErrorBody`.
- Handlers return `Result<impl IntoResponse, AppError>` and carry `# Errors` doc sections.

### app_state.rs — register the validator

- `AppState` pairs the interactors with a `Validator` per request DTO (`HasSchema`). Register the new DTO there. (`# Panics` doc applies — `AppState::new` derives schemas eagerly.)

### app_error.rs — map the new errors

- Add `From<YourDomainError>`-level mappings so the new domain error variants produce the right status codes. **Domain → HTTP mapping lives only in the app layer** (`app_error.rs`), never in `libs/web`. The app's `ApiError` is deliberately not a thiserror type — follow that.
- Add inline `#[cfg(test)]` unit tests for each new mapping (e.g. `not_found_maps_to_404`) in this file.

### api.rs — route + OpenAPI metadata

- Mount the route with `ApiRouter` and a transform closure. Set `op.id(...)` (camelCase `operationId`, e.g. `listTodos`), `.summary(...)`, and **`.description(...)` — aide cannot read handler doc comments**, so the operation description is a deliberate duplicate of the handler's doc-comment first paragraph; keep the two in sync (the file carries a keep-in-sync note).
- Chain the response types (`.response::<code, Json<T>>`) and the shared `standard_get_errors`/`standard_post_errors`/`standard_put_errors` transforms for the documented error responses.
- The OpenAPI document is generated at runtime by aide and served at `GET /openapi.json` — there is no committed `openapi.json` file. Success-response descriptions come from the response DTO's doc comment; if a response shows `description: ""`, the DTO is missing its doc comment.

## 4. Tests

- **Integration tests**: one file per handler — `tests/<handler_snake_case>.rs` (e.g. `add_todo.rs`). Cross-cutting tests (OpenAPI document checks, infrastructure-failure scenarios like the 503) go in `tests/api.rs`. Every test file has a crate-level `//!` doc (`missing_docs` applies to test binaries) and starts with `mod common;`.
- **Seed with fixtures, not chained endpoints**: create a `tests/fixtures/<operation_id_snake_case>.sql` file (e.g. `updateTodo` → `update_todo.sql`) with plain INSERTs and load it via `#[sqlx::test(migrator = "<domain>::MIGRATOR", fixtures("<name>"))]`. Assert deltas against the fixture's known values. `#[sqlx::test]` gives each test an isolated database — no truncate helpers needed.
- **Use the shared helpers**: generic request helpers (`get`, `post_json`, `put_json`, `post_body`, `next_cursor`, `ResponseParts`) live in `web::test_support` (apps opt in via `web = { workspace = true, features = ["test-support"] }`) — import them **directly in each test file, only what that file uses**. App-specific shared helpers live in `tests/common.rs` (flat file, included via `mod common;`); keep it to helpers *every* test binary uses — endpoint-specific helpers (fixture id derivation, response decoders like `read_page`) go in the single test file that uses them. Never duplicate the generic helpers locally, and never add `#![allow(dead_code, unused_imports)]` to test modules — structure the imports instead.
- Cover at least: the happy path, validation rejections (missing/invalid body, 422), not-found paths (404), malformed parameters, and — for new error mappings — each status the app_error tests assert. Match the existing assertion stack: `claims::assert_ok!`, `serde_json_assert::assert_json_include!`, content-type checks alongside status checks.
- If the endpoint adds schemas/operations worth pinning, extend the OpenAPI tests in `tests/api.rs` (`documents_descriptions_across_the_spec` is the pattern).

## 5. Verify the whole path

Run from the repo root (see `writing-rust` for the full loop): `just cargo-lint`, then `just cargo-test <app>` and `just cargo-test <domain>` with compose services up (`just infrastructure up -d --wait`), and keep `just cargo-fmt` clean. Formatting must be spotless and warnings fatal — that is CI.
