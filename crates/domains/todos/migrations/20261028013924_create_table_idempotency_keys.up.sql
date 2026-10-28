create table idempotency_keys (
  idempotency_key string         not null check (idempotency_key ~ '^[!-~]+$'
                                                 and length(idempotency_key) between 1 and 64),
  fingerprint     string         not null,
  status          string         not null check (status in ('pending', 'completed')),
  entity_id       uuid           references todos (todo_id),
  created_at      timestamptz(6) not null default clock_timestamp(),
  primary key (idempotency_key),
  check (status = 'pending' or entity_id is not null)
);

select util.add_updated_at('idempotency_keys');
