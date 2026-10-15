[private]
default:
  @just --list --unsorted --list-heading '' --list-prefix ''

watch:
  watchexec -r -w src/ -- cargo run

test-watch:
  watchexec -r -w src/ -- cargo test

cover:
  cargo tarpaulin -o Html

psql:
  docker compose exec -it postgres psql -U postgres

migrate direction:
  #!/usr/bin/env bash
  case {{direction}} in
    up)   sqlx migrate run ;;
    down) sqlx migrate revert ;;
    zero) sqlx migrate revert --target-version 0 ;;
    *) echo "error: migrations unknown direction" >&2 && exit 1 ;;
  esac