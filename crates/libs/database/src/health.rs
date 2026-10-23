use sqlx::PgPool;

use crate::db_error::{DbContext, DbError};

/// # Errors
///
/// Returns a [`DbError`] if database is unreachable.
pub async fn ping(pool: PgPool) -> Result<(), DbError> {
    sqlx::query("select 1")
        .execute(&pool)
        .await
        .map(|_| ())
        .db_context("pinging database")
}
