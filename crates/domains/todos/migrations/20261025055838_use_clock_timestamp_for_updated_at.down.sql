alter table todos
  alter column updated_at set default now();

create or replace function util._set_updated_at() returns trigger as $$
  begin
    new.updated_at = now();
    return new;
  end;
$$ language plpgsql;
