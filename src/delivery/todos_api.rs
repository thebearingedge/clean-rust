use axum::{Router, routing::post};

use crate::controllers::{app_state::AppState, todos_controller::add_todo};

pub fn create_todos_api(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/todos", post(add_todo))
        .with_state(app_state)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, Response, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use serde_json_assert::assert_json_include;
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::{controllers::app_state::AppState, delivery::todos_api::create_todos_api};

    fn build_request(method: Method, uri: &'static str, data: Option<Value>) -> Request<Body> {
        let builder = Request::builder().method(method).uri(uri);
        if let Some(body) = data {
            return builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap();
        }
        builder.body(Body::empty()).unwrap()
    }

    async fn get_response_data(res: Response<Body>) -> (StatusCode, String, Option<Value>) {
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

    #[sqlx::test]
    async fn rejects_a_missing_task(postgres: PgPool) {
        let router = create_todos_api(AppState::new(postgres));
        let req = build_request(
            Method::POST,
            "/v1/todos",
            Some(serde_json::json!({
                "task": ""
            })),
        );
        let res = router.oneshot(req).await.unwrap();
        let (status, content_type, data) = get_response_data(res).await;

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

    #[sqlx::test]
    async fn responds_with_the_added_todo(postgres: PgPool) {
        let router = create_todos_api(AppState::new(postgres));
        let req = build_request(
            Method::POST,
            "/v1/todos",
            Some(serde_json::json!({
                "task": "write some rust!"
            })),
        );
        let res = router.oneshot(req).await.unwrap();
        let (status, content_type, data) = get_response_data(res).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type, "application/json".to_string());
        assert_json_include!(actual: data, expected: serde_json::json!({
            "task": "write some rust!",
            "completedAt": null
        }));
    }
}
