//! HTTP integration tests for request-id propagation, exercised through
//! `create_todos_api` against a test database.

mod common;

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::common::todos_api;
    use web::test_support::{get, get_with_headers};

    const REQUEST_ID_HEADER: &str = "x-request-id";

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn generates_a_request_id_when_none_is_sent(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/v1/todos").await;

        assert_eq!(parts.status, StatusCode::OK);
        let request_id = parts
            .header(REQUEST_ID_HEADER)
            .expect("response should carry an x-request-id");
        Uuid::parse_str(&request_id).expect("generated request id should be a UUID");
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn echoes_a_valid_incoming_request_id(postgres: PgPool) {
        let parts = get_with_headers(
            todos_api(postgres),
            "/v1/todos",
            &[(REQUEST_ID_HEADER, "gateway-id_XYZ-123")],
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.header(REQUEST_ID_HEADER),
            Some("gateway-id_XYZ-123".to_owned())
        );
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn replaces_an_overlong_request_id(postgres: PgPool) {
        let incoming = "a".repeat(70);
        let parts = get_with_headers(
            todos_api(postgres),
            "/v1/todos",
            &[(REQUEST_ID_HEADER, incoming.as_str())],
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        let request_id = parts
            .header(REQUEST_ID_HEADER)
            .expect("response should carry an x-request-id");
        let parsed = Uuid::parse_str(&request_id).expect("replaced request id should be a UUID");
        assert_ne!(parsed.to_string(), incoming);
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn replaces_a_request_id_containing_a_space(postgres: PgPool) {
        let parts = get_with_headers(
            todos_api(postgres),
            "/v1/todos",
            &[(REQUEST_ID_HEADER, "id with space")],
        )
        .await;

        assert_eq!(parts.status, StatusCode::OK);
        let request_id = parts
            .header(REQUEST_ID_HEADER)
            .expect("response should carry an x-request-id");
        let parsed = Uuid::parse_str(&request_id).expect("replaced request id should be a UUID");
        assert_ne!(parsed.to_string(), "id with space");
    }

    #[sqlx::test(migrator = "todos::MIGRATOR")]
    async fn not_found_bodies_carry_the_response_request_id(postgres: PgPool) {
        let parts = get(
            todos_api(postgres),
            &format!("/v1/todos/{}", Uuid::now_v7()),
        )
        .await;

        assert_eq!(parts.status, StatusCode::NOT_FOUND);
        let request_id = parts
            .header(REQUEST_ID_HEADER)
            .expect("response should carry an x-request-id");
        assert_json_include!(actual: parts.body, expected: json!({
            "error": "todo not found",
            "requestId": request_id,
        }));
    }

    #[sqlx::test]
    async fn documents_request_id_as_an_optional_error_body_property(postgres: PgPool) {
        let parts = get(todos_api(postgres), "/openapi.json").await;

        assert_eq!(parts.status, StatusCode::OK);
        let body = parts.body.expect("should have returned the document");

        for schema_name in ["ErrorBody", "ValidationErrorBody"] {
            let schema = &body["components"]["schemas"][schema_name];
            let properties = schema["properties"]
                .as_object()
                .expect("the schema should document properties");
            assert!(
                properties.contains_key("requestId"),
                "{schema_name} should document the requestId property"
            );
            let empty = Vec::new();
            let required = schema["required"].as_array().unwrap_or(&empty);
            assert!(
                !required.iter().any(|value| value == "requestId"),
                "requestId should be optional in {schema_name}"
            );
        }
    }
}
