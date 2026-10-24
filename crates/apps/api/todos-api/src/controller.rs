use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use todos::models::todo::{NewTodoModel, TodoCursor, TodoFilter, TodoModel, TodoSortKey};
use utils::pagination::{Cursor, Page, Sort, SortOrder};
use web::{
    cursor_token::CursorError,
    extractors::{ValidatedJson, ValidatedPath, ValidatedQuery},
};

use crate::{app_error::AppError, app_state::AppState};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewTodoHttp {
    #[validate(length(min = 1))]
    task: String,
}

impl From<NewTodoHttp> for NewTodoModel {
    fn from(new: NewTodoHttp) -> Self {
        NewTodoModel {
            task: new.task,
            completed_at: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoHttp {
    pub todo_id: Uuid,
    pub task: String,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<TodoModel> for TodoHttp {
    fn from(model: TodoModel) -> Self {
        TodoHttp {
            todo_id: model.todo_id,
            task: model.task,
            updated_at: model.updated_at,
            completed_at: model.completed_at,
        }
    }
}

pub async fn add_todo(
    State(app_state): State<AppState>,
    ValidatedJson(new_todo): ValidatedJson<NewTodoHttp>,
) -> Result<(StatusCode, Json<TodoHttp>), AppError> {
    let model = app_state.interactors.todos.add(&new_todo.into()).await?;
    Ok((StatusCode::CREATED, Json(model.into())))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SortKeyHttp {
    TodoId,
    UpdatedAt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OrderHttp {
    Asc,
    Desc,
}

impl From<SortKeyHttp> for TodoSortKey {
    fn from(key: SortKeyHttp) -> Self {
        match key {
            SortKeyHttp::TodoId => Self::TodoId,
            SortKeyHttp::UpdatedAt => Self::UpdatedAt,
        }
    }
}

impl From<TodoSortKey> for SortKeyHttp {
    fn from(key: TodoSortKey) -> Self {
        match key {
            TodoSortKey::TodoId => Self::TodoId,
            TodoSortKey::UpdatedAt => Self::UpdatedAt,
        }
    }
}

impl From<OrderHttp> for SortOrder {
    fn from(order: OrderHttp) -> Self {
        match order {
            OrderHttp::Asc => Self::Asc,
            OrderHttp::Desc => Self::Desc,
        }
    }
}

impl From<SortOrder> for OrderHttp {
    fn from(order: SortOrder) -> Self {
        match order {
            SortOrder::Asc => Self::Asc,
            SortOrder::Desc => Self::Desc,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct CursorToken {
    sort: SortKeyHttp,
    order: OrderHttp,
    todo_id: Uuid,
    updated_at_micros: i64,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTodosQuery {
    #[validate(length(max = 50))]
    todo_id: Option<Vec<Uuid>>,
    is_completed: Option<bool>,
    sort: Option<SortKeyHttp>,
    order: Option<OrderHttp>,
    #[validate(range(min = 1, max = 200))]
    limit: Option<u64>,
    cursor: Option<String>,
}

pub struct ListParams {
    pub filter: TodoFilter,
    pub sort: Sort<TodoSortKey>,
    pub cursor: Cursor<TodoCursor>,
}

impl TryFrom<ListTodosQuery> for ListParams {
    type Error = CursorError;

    fn try_from(query: ListTodosQuery) -> Result<Self, CursorError> {
        let token = query
            .cursor
            .as_deref()
            .map(web::cursor_token::decode::<CursorToken>)
            .transpose()?;
        let key = query
            .sort
            .or(token.as_ref().map(|t| t.sort))
            .unwrap_or_else(|| TodoSortKey::default().into());
        let order = query
            .order
            .or(token.as_ref().map(|t| t.order))
            .unwrap_or_else(|| SortOrder::default().into());
        let after = match token {
            Some(t) if t.sort != key || t.order != order => return Err(CursorError::SortMismatch),
            Some(t) => Some(TodoCursor {
                todo_id: t.todo_id,
                updated_at: DateTime::from_timestamp_micros(t.updated_at_micros)
                    .ok_or(CursorError::Malformed)?,
            }),
            None => None,
        };

        Ok(Self {
            filter: TodoFilter {
                todo_id: query.todo_id,
                is_completed: query.is_completed,
            },
            sort: Sort {
                key: key.into(),
                order: order.into(),
            },
            cursor: Cursor {
                after,
                limit: query.limit,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoPageHttp {
    pub items: Vec<TodoHttp>,
    pub next: Option<String>,
}

impl TodoPageHttp {
    #[must_use]
    pub fn new(page: Page<TodoModel, TodoCursor>, sort: Sort<TodoSortKey>) -> Self {
        let next = page.next.map(|cursor| {
            web::cursor_token::encode(&CursorToken {
                sort: sort.key.into(),
                order: sort.order.into(),
                todo_id: cursor.todo_id,
                updated_at_micros: cursor.updated_at.timestamp_micros(),
            })
        });
        Self {
            items: page.items.into_iter().map(TodoHttp::from).collect(),
            next,
        }
    }
}

pub async fn list_todos(
    State(app_state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<ListTodosQuery>,
) -> Result<Json<TodoPageHttp>, AppError> {
    let ListParams {
        filter,
        sort,
        cursor,
    } = ListParams::try_from(query)?;
    let page = app_state
        .interactors
        .todos
        .list_by_filter(&filter, sort, &cursor)
        .await?;
    Ok(Json(TodoPageHttp::new(page, sort)))
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct TodoPath {
    todo_id: Uuid,
}

#[axum::debug_handler]
pub async fn get_todo(
    State(app_state): State<AppState>,
    ValidatedPath(path): ValidatedPath<TodoPath>,
) -> Result<Json<TodoHttp>, AppError> {
    let todo = app_state.interactors.todos.get_by_id(path.todo_id).await?;
    Ok(Json(todo.into()))
}
