use std::sync::Arc;

use aide::axum::{ApiRouter, routing::post_with};
use aide::openapi::{Info, OpenApi};
use axum::routing::get;
use axum::{Extension, Json, Router};

use crate::{
    app_state::AppState,
    controller::{self, TodoHttp},
};

pub fn create_todos_api(app_state: AppState) -> Router {
    let mut api = OpenApi {
        info: Info {
            title: "Todos API".into(),
            version: "0.1.0".into(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    ApiRouter::new()
        .api_route(
            "/v1/todos",
            post_with(controller::add_todo, |op| {
                web::openapi::standard_post_errors(
                    op.id("addTodo")
                        .summary("Create a todo")
                        .response::<201, Json<TodoHttp>>(),
                )
            }),
        )
        .route("/openapi.json", get(web::openapi::serve))
        .with_state(app_state)
        .finish_api(&mut api)
        .layer(Extension(Arc::new(api)))
}
