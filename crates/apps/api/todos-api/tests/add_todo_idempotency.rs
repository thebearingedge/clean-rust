//! HTTP integration tests for the optional `Idempotency-Key` header on the
//! `POST /v1/todos` (`add_todo`) endpoint, exercised through
//! `create_todos_api` against a migrated test database.

mod common;

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;
    use uuid::Uuid;

    use database::Database;
    use todos::models::todo::NewTodoModel;
    use todos::{Claim, IdempotencyKey, IdempotencyRepository, PgIdempotencyRepository};
    use web::test_support::{post_json, post_json_with_headers};

    use super::common::todos_api;

    #[must_use]
    fn key(raw: &str) -> IdempotencyKey {
        IdempotencyKey::parse(raw).expect("test key must be valid")
    }

    async fn todo_row_count(postgres: &PgPool) -> i64 {
        sqlx::query_scalar("select count(*) from todos")
            .fetch_one(postgres)
            .await
            .unwrap()
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn creates_with_a_fresh_key(postgres: PgPool) {
        let parts = post_json_with_headers(
            todos_api(postgres.clone()),
            "/v1/todos",
            &[("idempotency-key", "fresh")],
            &json!({ "task": "write some rust!" }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::CREATED);
        assert_eq!(parts.content_type, "application/json");
        assert_eq!(parts.header("idempotent-replayed"), None);

        let data = parts
            .body
            .as_ref()
            .expect("should have returned the added todo");

        let todo_id = data["todoId"].as_str().expect("todoId should be a string");
        let todo_id = Uuid::parse_str(todo_id).expect("todoId should be a valid uuid");

        assert_eq!(todo_id.get_version_num(), 7);
        assert_json_include!(actual: parts.body, expected: json!({
            "task": "write some rust!",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn replays_a_completed_key(postgres: PgPool) {
        let router = todos_api(postgres.clone());
        let body = json!({ "task": "write some rust!" });

        let first = post_json_with_headers(
            router.clone(),
            "/v1/todos",
            &[("idempotency-key", "replay")],
            &body,
        )
        .await;
        let second =
            post_json_with_headers(router, "/v1/todos", &[("idempotency-key", "replay")], &body)
                .await;

        assert_eq!(first.status, StatusCode::CREATED);
        assert_eq!(first.header("idempotent-replayed"), None);

        assert_eq!(second.status, StatusCode::CREATED);
        assert_eq!(second.content_type, "application/json");
        assert_eq!(
            second.header("idempotent-replayed"),
            Some("true".to_owned())
        );

        let first_id: Uuid = first
            .body
            .as_ref()
            .expect("should have returned the added todo")["todoId"]
            .as_str()
            .expect("todoId should be a string")
            .parse()
            .expect("todoId should be a valid uuid");
        let second_id: Uuid = second
            .body
            .as_ref()
            .expect("should have returned the added todo")["todoId"]
            .as_str()
            .expect("todoId should be a string")
            .parse()
            .expect("todoId should be a valid uuid");

        assert_eq!(second_id, first_id);
        assert_eq!(todo_row_count(&postgres).await, 1);
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn rejects_a_reused_key_with_a_different_body(postgres: PgPool) {
        let router = todos_api(postgres);
        let key_header = &[("idempotency-key", "reused")];

        let first = post_json_with_headers(
            router.clone(),
            "/v1/todos",
            key_header,
            &json!({ "task": "the original" }),
        )
        .await;

        assert_eq!(first.status, StatusCode::CREATED);

        let second = post_json_with_headers(
            router,
            "/v1/todos",
            key_header,
            &json!({ "task": "a different task" }),
        )
        .await;

        assert_eq!(second.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(second.content_type, "application/json");
        assert_json_include!(actual: second.body.expect("should have returned an error body"), expected: json!({
            "error": "idempotency key was already used for a different request"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn rejects_an_invalid_key(postgres: PgPool) {
        let router = todos_api(postgres);
        let body = json!({ "task": "write some rust!" });

        for invalid_key in ["k".repeat(65), "has a space".to_owned()] {
            let parts = post_json_with_headers(
                router.clone(),
                "/v1/todos",
                &[("idempotency-key", &invalid_key)],
                &body,
            )
            .await;

            assert_eq!(parts.status, StatusCode::BAD_REQUEST);
            assert_eq!(parts.content_type, "application/json");
            assert_json_include!(actual: parts.body.expect("should have returned an error body"), expected: json!({
                "error": "invalid idempotency key"
            }));
        }
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn absent_header_keeps_the_old_behavior(postgres: PgPool) {
        let router = todos_api(postgres.clone());
        let body = json!({ "task": "write some rust!" });

        let first = post_json(router.clone(), "/v1/todos", &body).await;
        let second = post_json(router, "/v1/todos", &body).await;

        assert_eq!(first.status, StatusCode::CREATED);
        assert_eq!(second.status, StatusCode::CREATED);
        assert_eq!(first.header("idempotent-replayed"), None);
        assert_eq!(second.header("idempotent-replayed"), None);

        let first_id: Uuid = first
            .body
            .as_ref()
            .expect("should have returned the added todo")["todoId"]
            .as_str()
            .expect("todoId should be a string")
            .parse()
            .expect("todoId should be a valid uuid");
        let second_id: Uuid = second
            .body
            .as_ref()
            .expect("should have returned the added todo")["todoId"]
            .as_str()
            .expect("todoId should be a string")
            .parse()
            .expect("todoId should be a valid uuid");

        assert_ne!(second_id, first_id);
        assert_eq!(todo_row_count(&postgres).await, 2);
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn in_flight_key_maps_to_409(postgres: PgPool) {
        let pending = NewTodoModel {
            task: "pending elsewhere".to_owned(),
            completed_at: None,
        };
        let claim = PgIdempotencyRepository::new(Database::Pool(postgres.clone()))
            .claim(&key("in-flight"), &pending.fingerprint())
            .await
            .expect("pending claim should succeed");
        assert_eq!(claim, Claim::Fresh);

        let parts = post_json_with_headers(
            todos_api(postgres.clone()),
            "/v1/todos",
            &[("idempotency-key", "in-flight")],
            &json!({ "task": "pending elsewhere" }),
        )
        .await;

        assert_eq!(parts.status, StatusCode::CONFLICT);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body.expect("should have returned an error body"), expected: json!({
            "error": "idempotency key is in flight"
        }));
        assert_eq!(todo_row_count(&postgres).await, 0);
    }
}
