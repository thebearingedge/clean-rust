use sqlx::{PgConnection, PgPool, Postgres, pool::PoolConnection};
use std::ops::{Deref, DerefMut};

use crate::common::errors::DbError;

#[cfg(test)]
use {
    sqlx::Transaction,
    std::sync::Arc,
    tokio::sync::{Mutex, OwnedMutexGuard},
};

#[derive(Clone)]
pub enum Database {
    Pool(PgPool),
    #[cfg(test)]
    Test(Arc<Mutex<Transaction<'static, Postgres>>>),
}

pub enum Conn {
    Pool(PoolConnection<Postgres>),
    #[cfg(test)]
    Test(OwnedMutexGuard<Transaction<'static, Postgres>>),
}

impl Database {
    pub async fn acquire(&self) -> Result<Conn, DbError> {
        match self {
            Database::Pool(pool) => pool
                .acquire()
                .await
                .map(Conn::Pool)
                .map_err(|e| DbError::new("acquiring connection", e)),
            #[cfg(test)]
            Database::Test(tx) => Ok(Conn::Test(tx.clone().lock_owned().await)),
        }
    }
}

impl Deref for Conn {
    type Target = PgConnection;
    fn deref(&self) -> &PgConnection {
        match self {
            Conn::Pool(c) => &**c,
            #[cfg(test)]
            Conn::Test(g) => &***g,
        }
    }
}

impl DerefMut for Conn {
    fn deref_mut(&mut self) -> &mut PgConnection {
        match self {
            Conn::Pool(c) => &mut **c,
            #[cfg(test)]
            Conn::Test(g) => &mut ***g,
        }
    }
}
