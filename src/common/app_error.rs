use axum::response::{IntoResponse, Response};
use thiserror::Error;

use crate::{
    controllers::api_error::ApiError, repositories::db_error::DbError,
    use_cases::todos_interactor::TodoError,
};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Todo(#[from] TodoError),

    #[error(transparent)]
    Db(#[from] DbError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let api_error: ApiError = match self {
            AppError::Todo(e) => e.into(),
            AppError::Db(e) => e.into(),
        };
        api_error.into_response()
    }
}
