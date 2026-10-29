//! Client-supplied idempotency keys, the repository port for claiming and
//! completing them, and a Postgres adapter implementing that port.

use async_trait::async_trait;
use sea_query::{Expr, ExprTrait, Iden, OnConflict, PostgresQueryBuilder, Query};
use sea_query_sqlx::SqlxBinder;
use sqlx::{self, AssertSqlSafe};
use tracing::instrument;
use uuid::Uuid;

use crate::{Database, DbContext, DbError};
use storage::StorageError;

/// The shortest valid key: one visible ASCII character.
pub const IDEMPOTENCY_MIN_LENGTH: usize = 1;

/// The longest valid key: 64 visible ASCII characters.
pub const IDEMPOTENCY_MAX_LENGTH: usize = 64;

/// A regex admitting exactly the keys [`IdempotencyKey::parse`] accepts: one
/// or more visible ASCII characters (the `!`–`~` range that
/// `u8::is_ascii_graphic` covers).
pub const IDEMPOTENCY_VISIBLE_ASCII_PATTERN: &str = "^[!-~]+$";

/// A client-supplied idempotency key: visible ASCII, 1 to 64 characters.
///
/// Construct via [`IdempotencyKey::parse`]; invalid input is rejected at the
/// boundary rather than stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Parses a raw key: visible ASCII, 1 to 64 characters. Returns
    /// [`Option::None`] when the input is shorter than
    /// [`IDEMPOTENCY_MIN_LENGTH`], longer than [`IDEMPOTENCY_MAX_LENGTH`], or
    /// contains characters outside the [`IDEMPOTENCY_VISIBLE_ASCII_PATTERN`]
    /// range.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let valid = (IDEMPOTENCY_MIN_LENGTH..=IDEMPOTENCY_MAX_LENGTH).contains(&raw.len())
            && raw.bytes().all(|b| b.is_ascii_graphic());
        valid.then(|| Self(raw.to_owned()))
    }

    /// The key as it was supplied by the client.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Outcome of attempting to claim an idempotency key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Claim {
    /// First time this key has been seen; go ahead.
    Fresh,
    /// The same request already succeeded; return its stored entity.
    Replay {
        /// Id of the entity created by the original request.
        entity_id: Uuid,
    },
    /// The key was used before for a different request.
    Mismatch,
    /// The key is in use by a request that has not finished yet.
    InFlight,
}

/// Persistence port for idempotency keys, backed by an infrastructure
/// adapter.
///
/// Implementations own the mapping between these domain types and their
/// backing store and must uphold the contract documented on each method.
#[async_trait]
pub trait IdempotencyRepository: std::fmt::Debug + Send + Sync {
    /// Attempts to claim `key` for a request with `fingerprint`, and is
    /// callable repeatedly on the same key without erroring: the first call
    /// for an unseen key returns [`Claim::Fresh`]; later calls return
    /// [`Claim::Replay`] when the same fingerprint was already completed,
    /// [`Claim::Mismatch`] when a different fingerprint was stored for the
    /// key, and [`Claim::InFlight`] when the same fingerprint is still
    /// pending.
    ///
    /// # Errors
    ///
    /// Returns a [`storage::StorageError`] if the claim state cannot be read
    /// or written.
    async fn claim(&self, key: &IdempotencyKey, fingerprint: &str) -> Result<Claim, StorageError>;

    /// Marks `key` as completed, associating it with the id of the entity
    /// created by the request that held the claim.
    ///
    /// # Errors
    ///
    /// Returns a [`storage::StorageError`] if the completion cannot be
    /// persisted.
    async fn record(&self, key: &IdempotencyKey, entity_id: Uuid) -> Result<(), StorageError>;
}

const CLAIM_CONTEXT: &str = "claiming idempotency key";
const STATUS_PENDING: &str = "pending";
const STATUS_COMPLETED: &str = "completed";

#[derive(Clone, Copy, Iden)]
enum IdempotencyKeys {
    Table,
    IdempotencyKey,
    Fingerprint,
    Status,
    EntityId,
}

#[derive(Clone, Copy, Iden)]
enum IdempotencyKeyStatus {
    #[iden = "idempotency_key_status"]
    Type,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "idempotency_key_status", rename_all = "lowercase")]
enum KeyStatus {
    Pending,
    Completed,
}

#[derive(sqlx::FromRow)]
struct KeyRow {
    fingerprint: String,
    status: KeyStatus,
    entity_id: Option<Uuid>,
}

/// Postgres implementation of the [`IdempotencyRepository`] port.
#[derive(Clone, Debug)]
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
impl IdempotencyRepository for PgIdempotencyRepository {
    #[instrument(skip_all)]
    async fn claim(&self, key: &IdempotencyKey, fingerprint: &str) -> Result<Claim, StorageError> {
        let (text, values) = Query::insert()
            .into_table(IdempotencyKeys::Table)
            .columns([
                IdempotencyKeys::IdempotencyKey,
                IdempotencyKeys::Fingerprint,
                IdempotencyKeys::Status,
            ])
            .values_panic([
                Expr::val(key.as_str()),
                Expr::val(fingerprint),
                Expr::val(STATUS_PENDING).as_enum(IdempotencyKeyStatus::Type),
            ])
            .on_conflict(
                OnConflict::column(IdempotencyKeys::IdempotencyKey)
                    .do_nothing()
                    .to_owned(),
            )
            .returning(Query::returning().columns([IdempotencyKeys::IdempotencyKey]))
            .build_sqlx(PostgresQueryBuilder);

        let mut conn = self.database.connect().await.map_err(StorageError::from)?;

        let claimed: Option<String> =
            sqlx::query_scalar_with::<_, String, _>(AssertSqlSafe(text), values)
                .fetch_optional(&mut *conn)
                .await
                .db_context(CLAIM_CONTEXT)
                .map_err(StorageError::from)?;

        if claimed.is_some() {
            return Ok(Claim::Fresh);
        }

        let (text, values) = Query::select()
            .columns([
                IdempotencyKeys::Fingerprint,
                IdempotencyKeys::Status,
                IdempotencyKeys::EntityId,
            ])
            .from(IdempotencyKeys::Table)
            .and_where(Expr::col(IdempotencyKeys::IdempotencyKey).eq(key.as_str()))
            .build_sqlx(PostgresQueryBuilder);

        let row: KeyRow = sqlx::query_as_with(AssertSqlSafe(text), values)
            .fetch_one(&mut *conn)
            .await
            .db_context(CLAIM_CONTEXT)
            .map_err(StorageError::from)?;

        if row.fingerprint != fingerprint {
            return Ok(Claim::Mismatch);
        }

        match row.status {
            KeyStatus::Pending => Ok(Claim::InFlight),
            KeyStatus::Completed => {
                let entity_id = row.entity_id.ok_or_else(|| {
                    StorageError::from(DbError::protocol(
                        CLAIM_CONTEXT,
                        "completed idempotency key has no entity_id",
                    ))
                })?;
                Ok(Claim::Replay { entity_id })
            }
        }
    }

    #[instrument(skip_all)]
    async fn record(&self, key: &IdempotencyKey, entity_id: Uuid) -> Result<(), StorageError> {
        let (text, values) = Query::update()
            .table(IdempotencyKeys::Table)
            .value(
                IdempotencyKeys::Status,
                Expr::val(STATUS_COMPLETED).as_enum(IdempotencyKeyStatus::Type),
            )
            .value(IdempotencyKeys::EntityId, Expr::val(entity_id))
            .and_where(Expr::col(IdempotencyKeys::IdempotencyKey).eq(key.as_str()))
            .build_sqlx(PostgresQueryBuilder);

        let mut conn = self.database.connect().await.map_err(StorageError::from)?;

        sqlx::query_with(AssertSqlSafe(text), values)
            .execute(&mut *conn)
            .await
            .db_context("recording idempotency key")
            .map(|_| ())
            .map_err(StorageError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_single_visible_character() {
        assert_eq!(IdempotencyKey::parse("a"), Some(IdempotencyKey("a".into())));
    }

    #[test]
    fn accepts_exactly_64_characters() {
        let raw = "k".repeat(64);
        assert_eq!(IdempotencyKey::parse(&raw), Some(IdempotencyKey(raw)),);
    }

    #[test]
    fn rejects_an_empty_key() {
        assert_eq!(IdempotencyKey::parse(""), None);
    }

    #[test]
    fn rejects_a_65_character_key() {
        assert_eq!(IdempotencyKey::parse(&"k".repeat(65)), None);
    }

    #[test]
    fn rejects_spaces_and_non_ascii() {
        assert_eq!(IdempotencyKey::parse("a b"), None);
        assert_eq!(IdempotencyKey::parse("café"), None);
        assert_eq!(IdempotencyKey::parse("é"), None);
    }

    #[test]
    fn rejects_control_characters() {
        assert_eq!(IdempotencyKey::parse("a\tb"), None);
        assert_eq!(IdempotencyKey::parse("a\nb"), None);
        assert_eq!(IdempotencyKey::parse("\u{7f}"), None);
    }
}
