create table todos (
  todo_id      uuid           not null default uuidv7(),
  task         string         not null,
  completed_at timestamptz(6),
  primary key (todo_id)
);

select util.add_updated_at('todos');

create index todos_updated_at_todo_id_idx on todos (updated_at, todo_id);