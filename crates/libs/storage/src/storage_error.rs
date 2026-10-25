use thiserror::Error;

type Source = Box<dyn std::error::Error + Send + Sync>;

/// Storage-layer error taxonomy; backends classify their source errors into
/// these variants via `From` impls.
#[derive(Debug, Error)]
pub enum StorageError {
    /// The storage backend is unreachable or degraded; callers typically map
    /// this to a 503.
    #[error("storage unavailable: {0}")]
    Unavailable(#[source] Source),

    /// A storage-level conflict (e.g. a uniqueness violation); callers
    /// typically map this to a 409.
    #[error("storage conflict: {0}")]
    Conflict(#[source] Source),

    /// Any storage failure that is neither availability loss nor a conflict;
    /// callers typically map this to a 500.
    #[error("storage error: {0}")]
    Other(#[source] Source),
}
