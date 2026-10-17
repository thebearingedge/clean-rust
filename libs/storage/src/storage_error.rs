use thiserror::Error;

type Source = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage unavailable")]
    Unavailable(#[source] Source),

    #[error("storage conflict")]
    Conflict(#[source] Source),

    #[error("storage error")]
    Other(#[source] Source),
}
