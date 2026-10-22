#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, Response, StatusCode, header},
    };
    use chrono::DateTime;
    use claims::assert_ok;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;
    use tower::ServiceExt;

    use todos_api::{api::create_todos_api, app_state::AppState};
    use uuid::Uuid;

    async fn get_response_parts(res: Response<Body>) -> (StatusCode, String, Option<Value>) {
        let status = res.status();
        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).ok();
        (status, content_type, json)
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn serves_openapi_document(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "openapi": "3.1.0",
            "info": { "title": "Todos API" },
            "paths": {
                "/v1/todos": { "post": { "operationId": "addTodo" } },
                "/v1/todos/{todo_id}": { "get": { "operationId": "getTodo" } }
            }
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn responds_503_when_the_database_is_unavailable(postgres: PgPool) {
        let router = create_todos_api(AppState::new(postgres.clone()));
        postgres.close().await;
        // POST a valid todo as in your other tests, then expect:
        // status 503, body { "error": "service unavailable" }
        let res = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/todos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({ "task": "write some rust!" }))
                            .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "service unavailable"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn requires_a_json_body(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/todos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "Expected request with `Content-Type: application/json`"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn requires_a_valid_json_body(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/todos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "Failed to parse the request body as JSON: EOF while parsing an object at line 1 column 1"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn rejects_a_missing_task(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/todos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({ "task": "" })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
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
    async fn responds_with_the_added_todo(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/todos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({ "task": "write some rust!" }))
                            .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type, "application/json".to_string());

        let todo_id = data
            .as_ref()
            .and_then(|d| d["todoId"].as_str())
            .expect("todoId should be a string");
        let todo_id = Uuid::parse_str(todo_id).expect("todoId should be a valid uuid");

        assert_eq!(todo_id.get_version_num(), 7);

        let updated_at = data
            .as_ref()
            .and_then(|d| d["updatedAt"].as_str())
            .expect("updatedAt should be a string");

        assert_ok!(DateTime::parse_from_rfc3339(updated_at));
        assert_json_include!(actual: data, expected: serde_json::json!({
            "task": "write some rust!",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn reads_a_created_todo(postgres: PgPool) {
        let router = create_todos_api(AppState::new(postgres));
        let res = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/todos")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({ "task": "write some rust!" }))
                            .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (_status, _content_type, data) = get_response_parts(res).await;

        let todo_id = data
            .map(|d| d["todoId"].as_str().unwrap().to_owned())
            .unwrap();

        let res = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/v1/todos/{todo_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (_status, _content_type, data) = get_response_parts(res).await;

        let updated_at = data
            .as_ref()
            .and_then(|d| d["updatedAt"].as_str())
            .expect("updatedAt should be a string");

        assert_ok!(DateTime::parse_from_rfc3339(updated_at));
        assert_json_include!(actual: data, expected: serde_json::json!({
            "task": "write some rust!",
            "completedAt": null
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn returns_404_for_a_missing_todo(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/v1/todos/{}", Uuid::now_v7()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "error": "todo not found"
        }))
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn returns_422_for_a_malformed_todo_id(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/v1/todos/not-a-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(content_type, "application/json");
        assert_json_include!(actual: data, expected: serde_json::json!({
            "details": [{ "path": "{todo_id}" }],
            "error": "schema violation"
        }))
    }

    fn get_ids(data: &Value) -> Vec<String> {
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["todoId"].as_str().unwrap().to_owned())
            .collect()
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn returns_todos_ordered_by_id(postgres: PgPool) {
        let res = create_todos_api(AppState::new(postgres))
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/v1/todos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");

        let ids = get_ids(&data.expect("should have returned seeded todos"));

        assert_eq!(ids.len(), 5);
        assert!(ids.is_sorted());
    }

    #[sqlx::test(migrator = "todos::MIGRATOR", fixtures("list_todos"))]
    async fn returns_pages_of_todos(postgres: PgPool) {
        let router = create_todos_api(AppState::new(postgres));
        let res = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/v1/todos?limit=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;
        let cursor = data.unwrap()["next"].as_str().unwrap_or("null").to_owned();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");
        assert_ne!(cursor, "null");

        let res = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/v1/todos?limit=2&cursor={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;
        let cursor = data.unwrap()["next"].as_str().unwrap_or("null").to_owned();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");
        assert_ne!(cursor, "null");

        let res = router
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/v1/todos?limit=2&cursor={cursor}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, content_type, data) = get_response_parts(res).await;
        let cursor = data.unwrap()["next"].as_str().unwrap_or("null").to_owned();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/json");
        assert_eq!(cursor, "null");
    }
}
