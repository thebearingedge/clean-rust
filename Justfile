[private]
default:
  @just --list --unsorted --list-heading '' --list-prefix ''

cover:
  cargo tarpaulin -o Html --workspace --output-dir coverage/

psql database:
  docker compose exec -it postgres psql -U postgres -d {{database}}
