set quiet

[private]
default:
  @just --list --unsorted --list-heading '' --list-prefix ''

init:
  cargo install --locked sqlx-cli@0.9.0
  cargo install --locked watchexec-cli@2.8.0
  cargo install --locked cargo-llvm-cov@0.9.1

migrate domain *args='run':
  set -a; . tools/envs/{{domain}}.env; set +a; \
  sqlx migrate {{args}} --source crates/domains/{{domain}}/migrations

cargo-lint:
  cargo clippy --workspace --all-targets -- -D warnings

cargo-run app *args:
  set -a; . tools/envs/{{app}}.env; set +a; \
  watchexec -r -w crates -e rs,toml,sql -- cargo run -p {{app}} -- {{args}}

cargo-test app *args:
  set -a; . tools/envs/{{app}}.env; set +a; \
  cargo test -p {{app}} {{args}}

cargo-cover app:
  set -a; . tools/envs/{{app}}.env; set +a; \
  cargo llvm-cov --no-report -p {{app}}

cargo-report:
  cargo llvm-cov report --html --output-dir coverage/
  cargo llvm-cov report --lcov --output-path coverage/rust.lcov.info

psql database:
  docker compose exec -it postgres psql -U postgres -d {{database}}
