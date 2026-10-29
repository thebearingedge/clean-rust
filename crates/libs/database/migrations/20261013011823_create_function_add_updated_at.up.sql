create function util._set_updated_at() returns trigger as $$
  begin
    new.updated_at = clock_timestamp();
    return new;
  end;
$$ language plpgsql;

create function util.add_updated_at(table_name regclass) returns void as $$
  begin
    execute format('
      alter table %I
        add column
            updated_at timestamptz(6) not null default clock_timestamp()
    ', table_name);
    execute format('
      create trigger set_updated_at_update
        before update
        on %I
        for each row
        when (new *<> old and new.updated_at = old.updated_at)
        execute procedure util._set_updated_at()
    ', table_name);
  end;
$$ language plpgsql;