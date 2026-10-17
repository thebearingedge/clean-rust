use axum::{Router, routing::post};

use crate::{app_state::AppState, controller::add_todo};

pub fn create_todos_api(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/todos", post(add_todo))
        .with_state(app_state)
}
