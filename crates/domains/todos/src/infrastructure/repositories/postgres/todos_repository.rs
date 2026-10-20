use std::pin::Pin;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use sea_query::extension::postgres::PgExpr;
use sea_query::{Expr, ExprTrait, Order, PostgresQueryBuilder, Query};
use sea_query_sqlx::SqlxBinder;
use sqlx::{self, AssertSqlSafe, PgPool};
use tracing::instrument;
use tracing_futures::Instrument;
use uuid::Uuid;

use database::db_error::DbContext;
use storage::storage_error::StorageError;
use utils::pagination::{Cursor, Sort, SortOrder};

use crate::models::todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey};
use crate::use_cases::repositories::TodosRepository as Interface;

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
        sqlx::query_as::<_, TodoRow>(
            r#"
            insert into todos (
                task,
                completed_at
            )
            values ($1, $2)
            returning
                todo_id,
                task,
                completed_at,
                updated_at
            "#,
        )
        .bind(&todo.task)
        .bind(todo.completed_at)
        .fetch_one(&self.service)
        .await
        .map(TodoModel::from)
        .db_context("inserting todo")
        .map_err(StorageError::from)
    }

    fn stream_by_filter(
        &self,
        filter: &TodoFilter,
        sort: &Sort<TodoSortKey>,
        cursor: &Cursor<TodoCursor>,
    ) -> Pin<Box<dyn Stream<Item = Result<TodoModel, StorageError>> + Send + '_>> {
        let direction = match sort.order {
            SortOrder::Asc => Order::Asc,
            SortOrder::Desc => Order::Desc,
        };
        let sort_columns: &[&str] = match sort.key {
            TodoSortKey::TodoId => &["todo_id"],
            TodoSortKey::UpdatedAt => &["updated_at", "todo_id"],
        };
        let (text, values) = Query::select()
            .columns(["todo_id", "task", "completed_at", "updated_at"])
            .from("todos")
            .apply_if(filter.todo_id.clone(), |q, v| {
                q.and_where(Expr::col("todo_id").eq_any(v));
            })
            .apply_if(filter.is_completed, |q, v| {
                q.and_where(if v {
                    Expr::col("completed_at").is_not_null()
                } else {
                    Expr::col("completed_at").is_null()
                });
            })
            .apply_if(cursor.after, |q, after| {
                let updated = || Expr::tuple([Expr::col("updated_at"), Expr::col("todo_id")]);
                let position =
                    || Expr::tuple([Expr::value(after.updated_at), Expr::value(after.todo_id)]);
                q.and_where(match (sort.key, sort.order) {
                    (TodoSortKey::TodoId, SortOrder::Asc) => Expr::col("todo_id").gt(after.todo_id),
                    (TodoSortKey::TodoId, SortOrder::Desc) => {
                        Expr::col("todo_id").lt(after.todo_id)
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
