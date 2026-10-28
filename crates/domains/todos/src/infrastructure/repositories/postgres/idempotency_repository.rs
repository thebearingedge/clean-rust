use async_trait::async_trait;
use tracing::instrument;
use uuid::Uuid;

use database::{Database, DbContext, DbError};
use storage::StorageError;

use crate::models::idempotency_key::IdempotencyKey;
use crate::use_cases::repositories::{Claim, IdempotencyRepository as Interface};

const CLAIM_CONTEXT: &str = "claiming idempotency key";
const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETED: &str = "completed";

#[derive(sqlx::FromRow)]
struct KeyRow {
    fingerprint: String,
    status: String,
    entity_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
/// Postgres implementation of the
/// [`IdempotencyRepository`](crate::use_cases::repositories::IdempotencyRepository)
/// port.
pub struct PgIdempotencyRepository {
    database: Database,
}

impl PgIdempotencyRepository {
    /// Creates a repository backed by the given [`Database`] connection
    /// source.
    #[must_use]
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl Interface for PgIdempotencyRepository {
    #[instrument(skip_all)]
    async fn claim(&self, key: &IdempotencyKey, fingerprint: &str) -> Result<Claim, StorageError> {
        let mut conn = self.database.connect().await.map_err(StorageError::from)?;

        let claimed: Option<String> = sqlx::query_scalar(
            "insert into idempotency_keys (idempotency_key, fingerprint, status) \
             values ($1, $2, 'pending') on conflict (idempotency_key) do nothing \
             returning idempotency_key",
        )
        .bind(key.as_str())
        .bind(fingerprint)
        .fetch_optional(&mut *conn)
        .await
        .db_context(CLAIM_CONTEXT)
        .map_err(StorageError::from)?;

        if claimed.is_some() {
            return Ok(Claim::Fresh);
        }

        let row: KeyRow = sqlx::query_as(
            "select fingerprint, status, entity_id from idempotency_keys \
             where idempotency_key = $1",
        )
        .bind(key.as_str())
        .fetch_one(&mut *conn)
        .await
        .db_context(CLAIM_CONTEXT)
        .map_err(StorageError::from)?;

        if row.fingerprint != fingerprint {
            return Ok(Claim::Mismatch);
        }

        match row.status.as_str() {
            STATUS_PENDING => Ok(Claim::InFlight),
            STATUS_COMPLETED => {
                let entity_id = row.entity_id.ok_or_else(|| {
                    StorageError::from(DbError::protocol(
                        CLAIM_CONTEXT,
                        "completed idempotency key has no entity_id",
                    ))
                })?;
                Ok(Claim::Replay { entity_id })
            }
            _ => Err(StorageError::from(DbError::protocol(
                CLAIM_CONTEXT,
                format!("idempotency key has unknown status {:?}", row.status),
            ))),
        }
    }

    #[instrument(skip_all)]
    async fn record(&self, key: &IdempotencyKey, entity_id: Uuid) -> Result<(), StorageError> {
        let mut conn = self.database.connect().await.map_err(StorageError::from)?;

        sqlx::query(
            "update idempotency_keys set status = 'completed', entity_id = $2 \
             where idempotency_key = $1",
        )
        .bind(key.as_str())
        .bind(entity_id)
        .execute(&mut *conn)
        .await
        .db_context("recording idempotency key")
        .map(|_| ())
        .map_err(StorageError::from)
    }
}
