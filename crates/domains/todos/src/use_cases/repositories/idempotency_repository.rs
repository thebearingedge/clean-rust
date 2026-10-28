//! Repository port for claiming and completing idempotency keys,
//! implemented by infrastructure adapters.

use async_trait::async_trait;
use uuid::Uuid;

use storage::StorageError;

use crate::models::idempotency_key::IdempotencyKey;

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
    /// Returns a [`StorageError`] if the claim state cannot be read or
    /// written.
    async fn claim(&self, key: &IdempotencyKey, fingerprint: &str) -> Result<Claim, StorageError>;

    /// Marks `key` as completed, associating it with the id of the entity
    /// created by the request that held the claim.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError`] if the completion cannot be persisted.
    async fn record(&self, key: &IdempotencyKey, entity_id: Uuid) -> Result<(), StorageError>;
}
