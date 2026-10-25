mod common;

#[cfg(test)]
mod tests {
    use super::common::{
        ResponseParts, create_todo_id, get, get_todo_ids, next_cursor, post_body, post_json,
        put_json,
    };

    use axum::{Router, body::Body, http::StatusCode};
    use chrono::{DateTime, Utc};
    use claims::assert_ok;
    use serde_json::json;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;
    use uuid::Uuid;

    use todos_api::{api::create_todos_api, app_state::AppState, controller::TodoPageHttp};

    fn todos_api(postgres: PgPool) -> Router {
        create_todos_api(AppState::new(postgres))
    }

    fn read_page(parts: ResponseParts) -> TodoPageHttp {
        serde_json::from_value(parts.body.expect("should have returned a page"))
            .expect("the page should deserialize")
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn serves_openapi_document(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/openapi.json").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "openapi": "3.1.0",
            "info": { "title": "Todos API" },
            "paths": {
                "/v1/todos": {
                    "get": { "operationId": "listTodos" },
                    "post": { "operationId": "addTodo" }
                },
                "/v1/todos/{todo_id}": {
                    "get": { "operationId": "getTodo" },
                    "put": { "operationId": "updateTodo" }
                }
            }
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn documents_the_update_todo_responses(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/openapi.json").await;

        assert_eq!(parts.status, StatusCode::OK);

        let put = &parts.body.expect("should have returned the document")["paths"]["/v1/todos/{todo_id}"]
            ["put"];

        assert_json_include!(actual: put.clone(), expected: json!({
            "operationId": "updateTodo",
            "summary": "Update a todo"
        }));

        let responses = put["responses"]
            .as_object()
            .expect("the operation should document its responses");

        assert!(
            responses.contains_key("200"),
            "updateTodo should document a 200 response"
        );
        assert!(
            responses.contains_key("404"),
            "updateTodo should document a 404 response"
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn responds_503_when_the_database_is_unavailable(postgres: PgPool) {
        let router = todos_api(postgres.clone());

        postgres.close().await;

        let parts = post_json(router, "/v1/todos", &json!({ "task": "write some rust!" })).await;

        assert_eq!(parts.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "service unavailable"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn requires_a_json_body(postgres: PgPool) {
        let parts = post_body(todos_api(postgres), "/v1/todos", None, Body::empty()).await;

        assert_eq!(parts.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "Expected request with `Content-Type: application/json`"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn requires_a_valid_json_body(postgres: PgPool) {
        let parts = post_body(
            todos_api(postgres),
            "/v1/todos",
            Some("application/json"),
            Body::from("{"),
        )
        .await;

        assert_eq!(parts.status, StatusCode::BAD_REQUEST);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "Failed to parse the request body as JSON: EOF while parsing an object at line 1 column 1"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn rejects_a_missing_task(postgres: PgPool) {
        let parts = post_json(todos_api(postgres), "/v1/todos", &json!({ "task": "" })).await;

        assert_eq!(parts.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "details": [
                {
                    "message": "\"\" is shorter than 1 character",
                    "path": "/task"
                }
            ],
            "error": "schema violation"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn accepts_a_single_character_task(postgres: PgPool) {
        let parts = post_json(todos_api(postgres), "/v1/todos", &json!({ "task": "a" })).await;

        assert_eq!(parts.status, StatusCode::CREATED);
        assert_json_include!(actual: parts.body.expect("should have returned the added todo"), expected: json!({
            "task": "a",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn rejects_a_whitespace_only_task(postgres: PgPool) {
        let parts = post_json(todos_api(postgres), "/v1/todos", &json!({ "task": "   " })).await;

        assert_eq!(parts.status, StatusCode::BAD_REQUEST);
        assert_json_include!(actual: parts.body.expect("should have returned an error body"), expected: json!({
            "error": "task must not be empty"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn responds_with_the_added_todo(postgres: PgPool) {
        let parts = post_json(
            todos_api(postgres),
            "/v1/todos",
            &json!({ "task": "write some rust!" }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::CREATED);
        assert_eq!(parts.content_type, "application/json");

        let data = parts
            .body
            .as_ref()
            .expect("should have returned the added todo");

        let todo_id = data["todoId"].as_str().expect("todoId should be a string");
        let todo_id = Uuid::parse_str(todo_id).expect("todoId should be a valid uuid");

        assert_eq!(todo_id.get_version_num(), 7);

        let updated_at = data["updatedAt"]
            .as_str()
            .expect("updatedAt should be a string");

        assert_ok!(DateTime::parse_from_rfc3339(updated_at));
        assert_json_include!(actual: parts.body, expected: json!({
            "task": "write some rust!",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("get_todo"))]
    async fn reads_a_created_todo(postgres: PgPool) {
        let parts = get(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000001",
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "task": "one",
            "updatedAt": "2026-01-05T00:00:00Z",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn returns_404_for_a_missing_todo(postgres: PgPool) {
        let parts = get(
            todos_api(postgres),
            &format!("/v1/todos/{}", Uuid::now_v7()),
        )
        .await;

        assert_eq!(parts.status, StatusCode::NOT_FOUND);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "todo not found"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("update_todo"))]
    async fn responds_with_the_updated_todo(postgres: PgPool) {
        let parts = put_json(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000001",
            &json!({ "task": "write more rust!", "isCompleted": true }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let data = parts
            .body
            .as_ref()
            .expect("should have returned the updated todo");

        let updated_at = data["updatedAt"]
            .as_str()
            .expect("updatedAt should be a string");
        let updated_at = assert_ok!(DateTime::parse_from_rfc3339(updated_at));
        let original_updated_at = assert_ok!(DateTime::parse_from_rfc3339("2026-01-05T00:00:00Z"));
        assert!(
            updated_at > original_updated_at,
            "updatedAt should strictly increase ({original_updated_at} -> {updated_at})"
        );

        let completed_at = data["completedAt"]
            .as_str()
            .expect("completedAt should be set for a completed todo");
        assert_ok!(DateTime::parse_from_rfc3339(completed_at));

        assert_json_include!(actual: parts.body, expected: json!({
            "task": "write more rust!"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("update_todo"))]
    async fn clears_completed_at_when_an_updated_todo_is_uncompleted(postgres: PgPool) {
        let parts = put_json(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000002",
            &json!({ "task": "write less rust!", "isCompleted": false }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "task": "write less rust!",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("update_todo"))]
    async fn refreshes_completed_at_when_an_updated_todo_stays_completed(postgres: PgPool) {
        let parts = put_json(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000002",
            &json!({ "task": "write more rust!", "isCompleted": true }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let data = parts
            .body
            .as_ref()
            .expect("should have returned the updated todo");

        let completed_at = data["completedAt"]
            .as_str()
            .expect("completedAt should stay set for a completed todo");
        let completed_at = assert_ok!(DateTime::parse_from_rfc3339(completed_at));
        let original_completed_at =
            assert_ok!(DateTime::parse_from_rfc3339("2026-01-06T00:00:00Z"));
        assert!(
            completed_at > original_completed_at,
            "completedAt should refresh ({original_completed_at} -> {completed_at})"
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("update_todo"))]
    async fn returns_404_when_updating_a_missing_todo(postgres: PgPool) {
        let parts = put_json(
            todos_api(postgres),
            &format!("/v1/todos/{}", Uuid::now_v7()),
            &json!({ "task": "write some rust!", "isCompleted": false }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::NOT_FOUND);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "todo not found"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("update_todo"))]
    async fn rejects_a_whitespace_only_task_when_updating(postgres: PgPool) {
        let parts = put_json(
            todos_api(postgres),
            "/v1/todos/00000000-0000-7000-8000-000000000001",
            &json!({ "task": "   ", "isCompleted": false }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::BAD_REQUEST);
        assert_json_include!(actual: parts.body.expect("should have returned an error body"), expected: json!({
            "error": "task must not be empty"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn returns_422_for_a_malformed_todo_id(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos/not-a-uuid").await;

        assert_eq!(parts.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "details": [{ "path": "{todo_id}" }],
            "error": "schema violation"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn returns_todos_ordered_by_id(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let ids = get_todo_ids(
            parts
                .body
                .as_ref()
                .expect("should have returned seeded todos"),
        );

        assert_eq!(
            ids,
            vec![
                create_todo_id(1),
                create_todo_id(2),
                create_todo_id(3),
                create_todo_id(4),
                create_todo_id(5),
            ]
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn returns_pages_of_todos(postgres: PgPool) {
        let router = todos_api(postgres);

        let parts = get(router.clone(), "/v1/todos?limit=2").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        let ids = get_todo_ids(
            parts
                .body
                .as_ref()
                .expect("should have returned the first page"),
        );
        assert_eq!(ids.len(), 2);
        let cursor = next_cursor(&parts).expect("the first page should have a next cursor");

        let parts = get(
            router.clone(),
            &format!("/v1/todos?limit=2&cursor={cursor}"),
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        let ids = get_todo_ids(
            parts
                .body
                .as_ref()
                .expect("should have returned the second page"),
        );
        assert_eq!(ids.len(), 2);
        let cursor = next_cursor(&parts).expect("the second page should have a next cursor");

        let parts = get(router, &format!("/v1/todos?limit=2&cursor={cursor}")).await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");
        let ids = get_todo_ids(
            parts
                .body
                .as_ref()
                .expect("should have returned the last page"),
        );
        assert_eq!(ids.len(), 1);
        assert_eq!(
            next_cursor(&parts),
            None,
            "the last page should not have a next cursor"
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn returns_completed_todos(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos?isCompleted=true").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let todo_page = read_page(parts);

        assert_eq!(todo_page.items.len(), 2);
        assert_eq!(todo_page.next, None);

        todo_page
            .items
            .iter()
            .for_each(|todo| assert_ne!(todo.completed_at, None));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn returns_incomplete_todos(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos?isCompleted=false").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let todo_page = read_page(parts);

        assert_eq!(todo_page.items.len(), 3);
        assert_eq!(todo_page.next, None);

        todo_page
            .items
            .iter()
            .for_each(|todo| assert_eq!(todo.completed_at, None));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn sorts_todos_by_updated_at(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos?sort=updatedAt").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let todo_page = read_page(parts);

        assert_eq!(todo_page.items.len(), 5);
        assert_eq!(todo_page.next, None);

        let updated_dates = todo_page
            .items
            .iter()
            .map(|todo| todo.updated_at)
            .collect::<Vec<DateTime<Utc>>>();

        assert!(updated_dates.is_sorted());
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn sorts_todos_by_todo_id_descending(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos?sort=todoId&order=desc").await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(parts.content_type, "application/json");

        let todo_page = read_page(parts);

        assert_eq!(todo_page.items.len(), 5);
        assert_eq!(todo_page.next, None);

        let todo_ids = todo_page
            .items
            .iter()
            .map(|todo| todo.todo_id)
            .collect::<Vec<Uuid>>();

        assert_eq!(
            todo_ids,
            vec![
                create_todo_id(5),
                create_todo_id(4),
                create_todo_id(3),
                create_todo_id(2),
                create_todo_id(1),
            ]
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn responds_400_for_a_malformed_cursor(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos?cursor=not-a-cursor").await;

        assert_eq!(parts.status, StatusCode::BAD_REQUEST);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "cursor is malformed"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn responds_422_when_the_cursor_sort_does_not_match(postgres: PgPool) {
        let router = todos_api(postgres);

        let parts = get(router.clone(), "/v1/todos?limit=2").await;

        assert_eq!(parts.status, StatusCode::OK);
        let cursor = next_cursor(&parts).expect("the first page should have a next cursor");

        let parts = get(
            router,
            &format!("/v1/todos?limit=2&sort=updatedAt&cursor={cursor}"),
        )
        .await;

        assert_eq!(parts.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "cursor does not match the requested sort"
        }));
    }
}
