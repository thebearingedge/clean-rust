set quiet

[private]
default:
  @just --list --unsorted --list-heading '' --list-prefix ''

init:
  cargo install --locked sqlx-cli@0.9.0
  cargo install --locked watchexec-cli@2.8.0
  cargo install --locked cargo-llvm-cov@0.9.1

infrastructure *args:
  docker compose -f infrastructure/docker-compose.yaml {{args}}

migrate domain action='run' *args:
  set -a; [ -f tools/envs/{{domain}}.env ] && . tools/envs/{{domain}}.env; set +a; \
  sqlx migrate {{action}} --source crates/domains/{{domain}}/migrations {{args}}

psql database:
  docker compose -f infrastructure/docker-compose.yaml exec -it postgres psql -U postgres -d {{database}}

cargo-lint crate='':
  cargo clippy {{ if crate == '' { '--workspace' } else { '-p ' + crate } }} --all-targets -- -D warnings

cargo-check crate *args:
  cargo check -p {{crate}} {{args}}

cargo-fmt:
  cargo fmt --all --check

cargo-fmt-fix:
  cargo fmt --all

cargo-run app *args:
  set -a; [ -f tools/envs/{{app}}.env ] && . tools/envs/{{app}}.env; set +a; \
  watchexec -r -w crates -e rs,toml,sql -- cargo run -p {{app}} -- {{args}}

cargo-test crate *args:
  set -a; [ -f tools/envs/{{crate}}.env ] && . tools/envs/{{crate}}.env; set +a; \
  cargo test -p {{crate}} {{args}}

cargo-tdd crate *args:
  set -a; [ -f tools/envs/{{crate}}.env ] && . tools/envs/{{crate}}.env; set +a; \
  watchexec -r -w crates -e rs,toml,sql -- cargo test -p {{crate}} -- {{args}}

cargo-test-all:
  set -a; . tools/envs/test.env; set +a; \
  cargo test --workspace

cargo-cover:
  set -a; . tools/envs/test.env; set +a; \
  cargo llvm-cov --no-report --workspace
  cargo llvm-cov report --html --output-dir coverage/
  cargo llvm-cov report --lcov --output-path coverage/rust.lcov.info
