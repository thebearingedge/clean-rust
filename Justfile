[private]
default:
  @just --list --unsorted --list-heading '' --list-prefix ''

watch api:
  watchexec -r -w apps/api/{{api}} -- cargo run -p {{api}}

test-watch api:
  watchexec -r -w apps/api/{{api}} -- cargo test -p {{api}}

cover:
  cargo tarpaulin -o Html --workspace --output-dir coverage/

psql database:
  docker compose exec -it postgres psql -U postgres -d {{database}}
