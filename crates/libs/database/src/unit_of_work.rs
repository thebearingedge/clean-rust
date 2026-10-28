//! The [`Database`] handle with transaction support.
//!
//! [`Database::Pool`] wraps a plain connection pool; [`Database::Unit`]
//! wraps a transaction shared with a [`UnitOfWork`]. Repositories built on
//! a [`Database`] work identically in both modes.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;

use sqlx::pool::PoolConnection;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::db_error::{DbContext, DbError};

/// The lock timeout [`Database::begin`] uses when no explicit timeout is
/// given: how long a repository call waits for the shared transaction before
/// the lock is reported as held.
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// Detail shared by the errors raised when a transaction is no longer open.
const TRANSACTION_FINISHED: &str = "transaction already finished";

/// A shared transaction together with the timeout repositories wait for its
/// lock.
type Shared = Arc<SharedUnit>;

/// The transaction and lock timeout shared behind a [`Database::Unit`]
/// handle.
pub struct SharedUnit {
    transaction: Arc<Mutex<Option<Transaction<'static, Postgres>>>>,
    lock_timeout: Duration,
}

/// A connection source: a plain pool or a transaction shared with a
/// [`UnitOfWork`].
///
/// Clones cheaply; every clone of a [`Database::Unit`] reaches the same
/// transaction.
#[derive(Clone)]
pub enum Database {
    /// A plain connection pool.
    Pool(PgPool),
    /// A transaction shared with a [`UnitOfWork`].
    Unit(Shared),
}

impl Database {
    /// Acquires a connection: from the pool, or by locking the shared
    /// transaction.
    ///
    /// # Errors
    ///
    /// Returns a [`DbError`] when a pool connection cannot be acquired,
    /// when another repository call holds the transaction lock for the
    /// timeout configured on the [`Database::Unit`] handle (see
    /// [`Database::begin_with`]), or when the transaction has already
    /// finished.
    pub async fn connect(&self) -> Result<Conn, DbError> {
        match self {
            Self::Pool(pool) => pool
                .acquire()
                .await
                .db_context("acquiring connection")
                .map(Conn::Pool),
            Self::Unit(shared) => {
                let guard = tokio::time::timeout(
                    shared.lock_timeout,
                    Arc::clone(&shared.transaction).lock_owned(),
                )
                .await
                .map_err(|_| {
                    DbError::protocol(
                        "waiting for the transaction lock",
                        "another repository call is holding the transaction",
                    )
                })?;
                if guard.is_none() {
                    return Err(DbError::protocol(
                        "acquiring connection",
                        TRANSACTION_FINISHED,
                    ));
                }
                Ok(Conn::Unit(guard))
            }
        }
    }

    /// Begins a transaction owned by the returned [`UnitOfWork`], with the
    /// default [`DEFAULT_LOCK_TIMEOUT`] on the shared transaction.
    ///
    /// # Errors
    ///
    /// Returns a [`DbError`] when called on a [`Database::Unit`] (nested
    /// transactions are not supported) or when the pool cannot begin a
    /// transaction.
    pub async fn begin(&self) -> Result<UnitOfWork, DbError> {
        self.begin_with(DEFAULT_LOCK_TIMEOUT).await
    }

    /// Begins a transaction owned by the returned [`UnitOfWork`], with
    /// `lock_timeout` bounding how long repository calls wait for the shared
    /// transaction (see [`Database::connect`]).
    ///
    /// # Errors
    ///
    /// Returns a [`DbError`] when called on a [`Database::Unit`] (nested
    /// transactions are not supported) or when the pool cannot begin a
    /// transaction.
    pub async fn begin_with(&self, lock_timeout: Duration) -> Result<UnitOfWork, DbError> {
        let Self::Pool(pool) = self else {
            return Err(DbError::protocol(
                "beginning transaction",
                "nested transactions are not supported",
            ));
        };
        let transaction = pool.begin().await.db_context("beginning transaction")?;
        Ok(UnitOfWork {
            shared: Arc::new(SharedUnit {
                transaction: Arc::new(Mutex::new(Some(transaction))),
                lock_timeout,
            }),
        })
    }
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pool(_) => f.write_str("Database::Pool(..)"),
            Self::Unit(_) => f.write_str("Database::Unit(..)"),
        }
    }
}

/// A checked-out connection: a pooled connection or a locked transaction.
pub enum Conn {
    /// A connection acquired from the pool.
    Pool(PoolConnection<Postgres>),
    /// A lock on the transaction of a [`UnitOfWork`].
    Unit(OwnedMutexGuard<Option<Transaction<'static, Postgres>>>),
}

impl Deref for Conn {
    type Target = PgConnection;

    /// Dereferences to the underlying connection.
    ///
    /// # Panics
    ///
    /// Panics when the transaction has already finished;
    /// [`Database::connect`] never returns a [`Conn::Unit`] in that state.
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Pool(c) => c,
            Self::Unit(g) => g
                .as_deref()
                .expect("connect() checked the transaction is open"),
        }
    }
}

impl DerefMut for Conn {
    /// Mutably dereferences to the underlying connection.
    ///
    /// # Panics
    ///
    /// Panics when the transaction has already finished;
    /// [`Database::connect`] never returns a [`Conn::Unit`] in that state.
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Pool(c) => c,
            Self::Unit(g) => g
                .as_deref_mut()
                .expect("connect() checked the transaction is open"),
        }
    }
}

impl fmt::Debug for Conn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pool(_) => f.write_str("Conn::Pool(..)"),
            Self::Unit(_) => f.write_str("Conn::Unit(..)"),
        }
    }
}

/// Owns a transaction begun from a [`Database::Pool`].
///
/// [`UnitOfWork::database`] hands out [`Database::Unit`] clones sharing the
/// transaction, and [`UnitOfWork::commit`] commits it. Dropping a
/// `UnitOfWork` without committing rolls the transaction back.
pub struct UnitOfWork {
    shared: Shared,
}

impl UnitOfWork {
    /// Returns a [`Database`] handle sharing this unit's transaction.
    #[must_use]
    pub fn database(&self) -> Database {
        Database::Unit(Arc::clone(&self.shared))
    }

    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Returns a [`DbError`] when the transaction has already finished or
    /// when the commit fails. Dropping the `UnitOfWork` without calling
    /// `commit` rolls the transaction back.
    pub async fn commit(self) -> Result<(), DbError> {
        let mut lock = self.shared.transaction.lock().await;
        let Some(transaction) = lock.take() else {
            return Err(DbError::protocol(
                "committing transaction",
                TRANSACTION_FINISHED,
            ));
        };
        transaction
            .commit()
            .await
            .db_context("committing transaction")
    }
}

impl fmt::Debug for UnitOfWork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnitOfWork(..)")
    }
}
