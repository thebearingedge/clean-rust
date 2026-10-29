create table idempotency_keys (
  idempotency_key string                 not null check (idempotency_key ~ '^[!-~]+$'
                                                         and length(idempotency_key) between 1 and 64),
  fingerprint     string                 not null,
  status          idempotency_key_status not null,
  entity_id       uuid,
  created_at      timestamptz(6)         not null default clock_timestamp(),
  primary key (idempotency_key),
  check (status = 'pending' or entity_id is not null)
);

select util.add_updated_at('idempotency_keys');

create index idempotency_keys_entity_id_idx on idempotency_keys (entity_id);
