create or replace function util._set_updated_at() returns trigger as $$
  begin
    new.updated_at = clock_timestamp();
    return new;
  end;
$$ language plpgsql;

alter table todos
  alter column updated_at set default clock_timestamp();
