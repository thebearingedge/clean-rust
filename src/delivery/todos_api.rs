use axum::{Router, routing::post};

use crate::controllers::{app_state::AppState, todos_controller::add_todo};

pub fn create_todos_router(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/todos", post(add_todo))
        .with_state(app_state)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        Router,
        body::Body,
        http::{Method, Request, Response, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use serde_json_assert::assert_json_include;
    use sqlx::postgres::PgPoolOptions;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    use crate::{
        common::database::Database, controllers::app_state::AppState,
        delivery::todos_api::create_todos_router,
    };

    async fn create_test_todos_router() -> Router {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let tx = Arc::new(Mutex::new(pool.begin().await.unwrap()));
        create_todos_router(AppState::new(Database::Test(tx)))
    }

    fn build_request(
        method: Method,
        uri: &'static str,
        content_type: &'static str,
        body: Value,
    ) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn get_response_data(res: Response<Body>) -> (StatusCode, String, Value) {
        let status = res.status();
        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap();
        (status, content_type, json)
    }

    #[tokio::test]
    async fn responds_with_the_added_todo() {
        let router = create_test_todos_router().await;
        let req = build_request(
            Method::POST,
            "/v1/todos",
            "application/json",
            serde_json::json!({
                "task": "write some rust!"
            }),
        );
        let res = router.oneshot(req).await.unwrap();
        let (status, content_type, data) = get_response_data(res).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type, "application/json".to_string());
        assert_json_include!(actual: data, expected: serde_json::json!({ "task": "write some rust!" }));
    }

    #[tokio::test]
    async fn rejects_a_missing_task() {
        let router = create_test_todos_router().await;
        let req = build_request(
            Method::POST,
            "/v1/todos",
            "application/json",
            serde_json::json!({
                "task": ""
            }),
        );
        let res = router.oneshot(req).await.unwrap();
        let (status, _content_type, data) = get_response_data(res).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
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
}
