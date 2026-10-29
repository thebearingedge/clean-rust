use std::sync::Arc;

use aide::axum::{ApiRouter, routing::get_with, routing::post_with};
use aide::openapi::{Info, OpenApi};
use axum::{Extension, Json, Router, routing::get};

use crate::{
    app_state::AppState,
    controller::{self, TodoHttp, TodoPageHttp},
    paths::{TODO_PATH, TODOS_PATH},
};
use web::api_error::ErrorBody;

/// Builds the todos HTTP router with aide-generated `OpenAPI` documentation.
///
/// All business routes are mounted via [`ApiRouter::api_route`] so handlers and
/// error responses are documented; the router carries the extension layer with
/// the generated [`OpenApi`] document served at `/openapi.json`, and JSON
/// fallback handlers replace axum's default empty 404 and 405 responses.
///
/// Aide cannot read doc comments, so the operation descriptions set in the
/// route-mount closures below are duplicated from the handler doc comments in
/// `controller.rs` and must be kept in sync with them.
pub fn create_todos_api(app_state: AppState) -> Router {
    let mut api = OpenApi {
        info: Info {
            title: "Todos API".into(),
            description: Some("CRUD API for managing todos, with cursor-based pagination.".into()),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    web::fallback::with_json_fallbacks(
        ApiRouter::new()
            .api_route(
                TODO_PATH,
                get_with(controller::get_todo, |op| {
                    web::openapi::standard_get_errors(
                        op.id("getTodo")
                            .summary("Get a todo")
                            .description("Gets a single todo by its ID.")
                            .response::<200, Json<TodoHttp>>()
                            .response::<404, Json<ErrorBody>>(),
                    )
                })
                .put_with(controller::update_todo, |op| {
                    web::openapi::standard_put_errors(
                        op.id("updateTodo")
                            .summary("Update a todo")
                            .description(
                                "Fully replaces a todo's mutable fields and returns the updated todo.",
                            )
                            .response::<200, Json<TodoHttp>>()
                            .response::<404, Json<ErrorBody>>(),
                    )
                })
                .delete_with(controller::delete_todo, |op| {
                    web::openapi::standard_delete_errors(
                        op.id("deleteTodo")
                            .summary("Delete a todo")
                            .description("Deletes a todo by its ID.")
                            .response::<204, ()>()
                            .response::<404, Json<ErrorBody>>(),
                    )
                }),
            )
            .api_route(
                TODOS_PATH,
                post_with(controller::add_todo, |op| {
                    web::openapi::standard_post_errors(
                        web::openapi::response_header::<Json<TodoHttp>, 201>(
                            op.id("addTodo")
                                .summary("Create a todo")
                                .description(
                                    "Adds a new todo and returns it with a `201 Created` status, \
                                     with the path of the new todo in the `Location` response \
                                     header. The optional `Idempotency-Key` request header \
                                     (visible ASCII, 1 to 64 characters) makes the creation \
                                     idempotent: the same key and body replay the original todo \
                                     and the response carries `Idempotent-Replayed: true`; the \
                                     same key with a different body is rejected with \
                                     `422 Unprocessable Entity`, and a key still held by an \
                                     unfinished request with `409 Conflict`. Without the header, \
                                     every request creates a new todo.",
                                )
                                .response::<409, Json<ErrorBody>>(),
                            "Location",
                            "The path of the new todo, relative to the service root.",
                        ),
                    )
                })
                .get_with(controller::list_todos, |op| {
                    web::openapi::standard_get_errors(
                        op.id("listTodos")
                            .summary("List todos")
                            .description("Lists todos by filter with cursor-based pagination.")
                            .response::<200, Json<TodoPageHttp>>(),
                    )
                }),
            )
            .route("/openapi.json", get(web::openapi::openapi_json))
            .with_state(app_state)
            .finish_api(&mut api)
            .layer(Extension(Arc::new(api))),
    )
}
