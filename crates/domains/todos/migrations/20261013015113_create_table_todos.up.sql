create table todos (
  todo_id      uuid           not null default uuidv7(),
  task         string         not null,
  completed_at timestamptz(6)
);

select util.add_updated_at('todos');