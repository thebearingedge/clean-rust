//! A transport-agnostic storage error taxonomy shared by persistence
//! backends, which classify their source errors into its variants via
//! `From` impls.

/// The storage error taxonomy.
pub mod storage_error;

/// The transport-agnostic storage error taxonomy.
pub use storage_error::StorageError;
