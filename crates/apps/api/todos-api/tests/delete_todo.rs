//! HTTP integration tests for the `DELETE /v1/todos/{todo_id}`
//! (`delete_todo`) endpoint, exercised through `create_todos_api` against a
//! migrated test database.

mod common;

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;
    use uuid::Uuid;

    use web::test_support::{delete, get};

    use super::common::todos_api;

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("delete_todo"))]
    async fn responds_with_204_and_removes_the_todo(postgres: PgPool) {
        let parts = delete(
            todos_api(postgres.clone()),
            "/v1/todos/00000000-0000-7000-8000-000000000001",
        )
        .await;

        assert_eq!(parts.status, StatusCode::NO_CONTENT);
        assert_eq!(parts.content_type, "");
        assert_eq!(parts.body, None);

        let parts = get(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000001",
        )
        .await;
        assert_eq!(parts.status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("delete_todo"))]
    async fn removes_only_the_targeted_todo(postgres: PgPool) {
        let parts = delete(
            todos_api(postgres.clone()),
            "/v1/todos/00000000-0000-7000-8000-000000000001",
        )
        .await;
        assert_eq!(parts.status, StatusCode::NO_CONTENT);

        let parts = get(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000002",
        )
        .await;
        assert_eq!(parts.status, StatusCode::OK);
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("delete_todo"))]
    async fn returns_404_when_deleting_a_missing_todo(postgres: PgPool) {
        let parts = delete(
            todos_api(postgres),
            &format!("/v1/todos/{}", Uuid::now_v7()),
        )
        .await;

        assert_eq!(parts.status, StatusCode::NOT_FOUND);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body.expect("should have returned an error body"), expected: json!({
            "error": "todo not found"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn returns_422_for_a_malformed_todo_id(postgres: PgPool) {
        let parts = delete(todos_api(postgres), "/v1/todos/not-a-uuid").await;

        assert_eq!(parts.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "details": [{ "path": "{todo_id}" }],
            "error": "schema violation"
        }));
    }
}
