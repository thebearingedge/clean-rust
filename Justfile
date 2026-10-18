set quiet

[private]
default:
  @just --list --unsorted --list-heading '' --list-prefix ''

init:
  cargo install --locked sqlx-cli@0.9.0
  cargo install --locked watchexec-cli@2.8.0
  cargo install --locked cargo-llvm-cov@0.9.1

migrate domain action='run' *args:
  set -a; . tools/envs/{{domain}}.env; set +a; \
  sqlx migrate {{action}} --source crates/domains/{{domain}}/migrations {{args}}

psql database:
  docker compose exec -it postgres psql -U postgres -d {{database}}

cargo-lint:
  cargo clippy --workspace --all-targets -- -D warnings

cargo-run app *args:
  set -a; . tools/envs/{{app}}.env; set +a; \
  watchexec -r -w crates -e rs,toml,sql -- cargo run -p {{app}} -- {{args}}

cargo-test app *args:
  set -a; . tools/envs/{{app}}.env; set +a; \
  cargo test -p {{app}} {{args}}

cargo-tdd app *args:
  set -a; . tools/envs/{{app}}.env; set +a; \
  watchexec -r -w crates -e rs,toml,sql -- cargo test -p {{app}} -- {{args}}

cargo-test-all:
  set -a; . tools/envs/test.env; set +a; \
  cargo test --workspace

cargo-cover:
  set -a; . tools/envs/test.env; set +a; \
  cargo llvm-cov --no-report --workspace
  cargo llvm-cov report --html --output-dir coverage/
  cargo llvm-cov report --lcov --output-path coverage/rust.lcov.info
