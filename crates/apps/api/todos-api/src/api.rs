use std::sync::Arc;

use aide::axum::{ApiRouter, routing::get_with, routing::post_with};
use aide::openapi::{Info, OpenApi};
use axum::{Extension, Json, Router, routing::get};

use web::api_error::ErrorBody;

use crate::{
    app_state::AppState,
    controller::{self, TodoHttp, TodoPageHttp},
};

/// Builds the todos HTTP router with aide-generated `OpenAPI` documentation.
///
/// All business routes are mounted via [`ApiRouter::api_route`] so handlers and
/// error responses are documented; the router carries the extension layer with
/// the generated [`OpenApi`] document served at `/openapi.json`.
pub fn create_todos_api(app_state: AppState) -> Router {
    let mut api = OpenApi {
        info: Info {
            title: "Todos API".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    ApiRouter::new()
        .api_route(
            "/v1/todos/{todo_id}",
            get_with(controller::get_todo, |op| {
                web::openapi::standard_get_errors(
                    op.id("getTodo")
                        .summary("Get a todo")
                        .response::<200, Json<TodoHttp>>()
                        .response::<404, Json<ErrorBody>>(),
                )
            })
            .put_with(controller::update_todo, |op| {
                web::openapi::standard_put_errors(
                    op.id("updateTodo")
                        .summary("Update a todo")
                        .response::<200, Json<TodoHttp>>()
                        .response::<404, Json<ErrorBody>>(),
                )
            }),
        )
        .api_route(
            "/v1/todos",
            post_with(controller::add_todo, |op| {
                web::openapi::standard_post_errors(
                    op.id("addTodo")
                        .summary("Create a todo")
                        .response::<201, Json<TodoHttp>>(),
                )
            })
            .get_with(controller::list_todos, |op| {
                web::openapi::standard_get_errors(
                    op.id("listTodos")
                        .summary("List todos")
                        .response::<200, Json<TodoPageHttp>>(),
                )
            }),
        )
        .route("/openapi.json", get(web::openapi::openapi_json))
        .with_state(app_state)
        .finish_api(&mut api)
        .layer(Extension(Arc::new(api)))
}
