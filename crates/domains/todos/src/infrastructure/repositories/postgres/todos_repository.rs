use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use sea_query::extension::postgres::PgExpr;
use sea_query::{Expr, ExprTrait, Iden, Order, PostgresQueryBuilder, Query};
use sea_query_sqlx::SqlxBinder;
use sqlx::{self, AssertSqlSafe, PgPool};
use tracing::instrument;
use tracing_futures::Instrument;
use uuid::Uuid;

use database::db_error::DbContext;
use storage::storage_error::StorageError;
use utils::pagination::{Cursor, Sort, SortOrder};

use crate::models::todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey};
use crate::use_cases::repositories::{TodoStream, TodosRepository as Interface};

#[derive(Clone, Copy, Iden)]
enum Todos {
    Table,
    TodoId,
    Task,
    CompletedAt,
    UpdatedAt,
}

#[derive(sqlx::FromRow)]
struct TodoRow {
    todo_id: Uuid,
    task: String,
    completed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl From<TodoRow> for TodoModel {
    fn from(row: TodoRow) -> Self {
        Self {
            todo_id: row.todo_id,
            task: row.task,
            completed_at: row.completed_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TodosRepository {
    service: PgPool,
}

impl TodosRepository {
    #[must_use]
    pub fn new(service: PgPool) -> Self {
        Self { service }
    }
}

#[async_trait]
impl Interface for TodosRepository {
    #[instrument(skip_all)]
    async fn add(&self, todo: &NewTodoModel) -> Result<TodoModel, StorageError> {
        let (text, values) = Query::insert()
            .into_table(Todos::Table)
            .columns([Todos::Task, Todos::CompletedAt])
            .values_panic([Expr::val(&todo.task), Expr::val(todo.completed_at)])
            .returning(Query::returning().columns([
                Todos::TodoId,
                Todos::Task,
                Todos::CompletedAt,
                Todos::UpdatedAt,
            ]))
            .build_sqlx(PostgresQueryBuilder);

        sqlx::query_as_with::<_, TodoRow, _>(AssertSqlSafe(text), values)
            .fetch_one(&self.service)
            .await
            .map(TodoModel::from)
            .db_context("inserting todo")
            .map_err(StorageError::from)
    }

    fn stream_by_filter(
        &self,
        filter: &TodoFilter,
        sort: Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> TodoStream<'_> {
        let direction = match sort.order {
            SortOrder::Asc => Order::Asc,
            SortOrder::Desc => Order::Desc,
        };
        let sort_columns: &[Todos] = match sort.key {
            TodoSortKey::TodoId => &[Todos::TodoId],
            TodoSortKey::UpdatedAt => &[Todos::UpdatedAt, Todos::TodoId],
        };
        let (text, values) = Query::select()
            .columns([
                Todos::TodoId,
                Todos::Task,
                Todos::CompletedAt,
                Todos::UpdatedAt,
            ])
            .from(Todos::Table)
            .apply_if(filter.todo_id.clone(), |q, v| {
                q.and_where(Expr::col(Todos::TodoId).eq_any(v));
            })
            .apply_if(filter.is_completed, |q, v| {
                q.and_where(if v {
                    Expr::col(Todos::CompletedAt).is_not_null()
                } else {
                    Expr::col(Todos::CompletedAt).is_null()
                });
            })
            .apply_if(cursor.after, |q, after| {
                let updated =
                    || Expr::tuple([Expr::col(Todos::UpdatedAt), Expr::col(Todos::TodoId)]);
                let position =
                    || Expr::tuple([Expr::value(after.updated_at), Expr::value(after.todo_id)]);
                q.and_where(match (sort.key, sort.order) {
                    (TodoSortKey::TodoId, SortOrder::Asc) => {
                        Expr::col(Todos::TodoId).gt(after.todo_id)
                    }
                    (TodoSortKey::TodoId, SortOrder::Desc) => {
                        Expr::col(Todos::TodoId).lt(after.todo_id)
                    }
                    (TodoSortKey::UpdatedAt, SortOrder::Asc) => updated().gt(position()),
                    (TodoSortKey::UpdatedAt, SortOrder::Desc) => updated().lt(position()),
                });
            })
            .order_by_columns(sort_columns.iter().map(|c| (*c, direction.clone())))
            .apply_if(cursor.limit, |q, v| {
                q.limit(v);
            })
            .build_sqlx(PostgresQueryBuilder);

        sqlx::query_as_with::<_, TodoRow, _>(AssertSqlSafe(text), values)
            .fetch(&self.service)
            .map(|row| {
                row.map(TodoModel::from)
                    .db_context("streaming todo")
                    .map_err(StorageError::from)
            })
            .instrument(tracing::Span::current())
            .boxed()
    }
}
