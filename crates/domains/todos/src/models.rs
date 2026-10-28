//! Models describing todos: creation and update inputs, the stored
//! representation, the idempotency key guarding create requests, and the
//! filter/sort/cursor parameters for listing them.

/// Client-supplied idempotency keys.
pub mod idempotency_key;
/// Concrete todo models.
pub mod todo;
