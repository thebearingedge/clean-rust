//! HTTP integration tests for the todos-api endpoints, exercised through
//! `create_todos_api` against a migrated test database.

mod common;

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;

    use web::test_support::{delete, get, post_json};

    use super::common::todos_api;

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

        let body = parts
            .body
            .as_ref()
            .expect("should have returned the document");

        let description = body["info"]["description"]
            .as_str()
            .expect("info should have a description");
        assert!(
            !description.is_empty(),
            "info.description should not be empty"
        );
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

        let found = responses
            .get("200")
            .expect("updateTodo should document a 200 response");

        let description = found["description"]
            .as_str()
            .expect("the 200 response should have a description");
        assert!(
            !description.is_empty(),
            "the 200 response description should not be empty"
        );

        let parameters = put["parameters"]
            .as_array()
            .expect("updateTodo should document its parameters");

        let todo_id = parameters
            .iter()
            .find(|parameter| parameter["in"] == "path" && parameter["name"] == "todo_id")
            .expect("updateTodo should document the todo_id path parameter");

        let description = todo_id["description"]
            .as_str()
            .expect("the todo_id path parameter should have a description");
        assert!(
            !description.is_empty(),
            "the todo_id path parameter description should not be empty"
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn documents_descriptions_across_the_spec(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/openapi.json").await;

        assert_eq!(parts.status, StatusCode::OK);

        let body = parts
            .body
            .as_ref()
            .expect("should have returned the document");

        for (path, method, operation_id) in [
            ("/v1/todos/{todo_id}", "get", "getTodo"),
            ("/v1/todos/{todo_id}", "put", "updateTodo"),
            ("/v1/todos", "get", "listTodos"),
            ("/v1/todos", "post", "addTodo"),
        ] {
            let operation = &body["paths"][path][method];

            assert_json_include!(actual: operation.clone(), expected: json!({
                "operationId": operation_id
            }));

            let description = operation["description"].as_str().unwrap_or_default();
            assert!(
                !description.is_empty(),
                "{operation_id} should have a description"
            );
        }

        let responses = body["paths"]["/v1/todos"]["post"]["responses"]
            .as_object()
            .expect("addTodo should document its responses");

        let created = responses
            .get("201")
            .expect("addTodo should document a 201 response");

        let description = created["description"].as_str().unwrap_or_default();
        assert!(
            !description.is_empty(),
            "the addTodo 201 response should have a description"
        );

        let responses = body["paths"]["/v1/todos"]["get"]["responses"]
            .as_object()
            .expect("listTodos should document its responses");

        let ok = responses
            .get("200")
            .expect("listTodos should document a 200 response");

        let description = ok["description"].as_str().unwrap_or_default();
        assert!(
            !description.is_empty(),
            "the listTodos 200 response should have a description"
        );

        let list_todos_query = &body["components"]["schemas"]["ListTodosQuery"]["properties"];

        for field in ["todoId", "isCompleted", "sort", "order", "limit", "cursor"] {
            let description = list_todos_query[field]["description"]
                .as_str()
                .unwrap_or_default();
            assert!(
                !description.is_empty(),
                "ListTodosQuery.{field} should have a description"
            );
        }

        for schema in ["NewTodoHttp", "TodoUpdateHttp"] {
            let description = body["components"]["schemas"][schema]["description"]
                .as_str()
                .unwrap_or_default();
            assert!(
                !description.is_empty(),
                "{schema} should have a type description"
            );
        }
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn serves_json_404_for_unknown_routes(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/nope").await;

        assert_eq!(parts.status, StatusCode::NOT_FOUND);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "route not found"
        }));
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn serves_json_405_for_wrong_methods(postgres: PgPool) {
        let parts = delete(todos_api(postgres), "/v1/todos").await;

        assert_eq!(parts.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(parts.content_type, "application/json");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "method not allowed"
        }));
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
}
