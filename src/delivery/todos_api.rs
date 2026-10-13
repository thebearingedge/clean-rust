use axum::{Router, routing::post};

use crate::controllers::{app_state::AppState, todos_controller::add_todo};

pub fn create_todos_router(app_state: AppState) -> Router {
    Router::new()
        .route("/v1/todos", post(add_todo))
        .with_state(app_state)
}
