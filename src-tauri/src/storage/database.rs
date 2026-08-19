//! The shared connection source for every storage domain (Architecture v2 / ADR 0050).
//!
//! Historically the `Db` enum, the `DbGuard` checkout, and `checkout()` lived on
//! `AppState`, which made `AppState` the only thing that could reach SQLite — the
//! root cause of the 207-method facade. Extracting them into a cheap-clone
//! [`Database`] handle lets each **domain store** (`WatchlistStore`,
//! `CompanyStore`, `FeedStore`, …) own its own connection source and expose only
//! its domain's operations, so a command can depend on the one store it needs
//! instead of the whole facade. This is the structural split of the *facade*, not
//! a repository port — the stores stay concrete and SQLite-coupled (ADR 0039).

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

use super::error::StorageResult;

pub(crate) type SqlitePool = r2d2::Pool<SqliteConnectionManager>;

/// How the app reaches SQLite. Production uses an r2d2 pool (concurrent readers
/// under WAL); tests use a single shared connection so an in-memory database is
/// not duplicated per pooled connection. See [ADR 0032].
#[derive(Clone)]
enum Db {
    Pool(SqlitePool),
    Single(Arc<Mutex<Connection>>),
}

/// A checked-out connection that derefs to `rusqlite::Connection`, regardless of
/// whether it came from the pool or the single test connection.
pub(crate) enum DbGuard<'a> {
    Pooled(PooledConnection<SqliteConnectionManager>),
    Locked(std::sync::MutexGuard<'a, Connection>),
}

impl std::ops::Deref for DbGuard<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        match self {
            DbGuard::Pooled(connection) => connection,
            DbGuard::Locked(connection) => connection,
        }
    }
}

impl std::ops::DerefMut for DbGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        match self {
            DbGuard::Pooled(connection) => connection,
            DbGuard::Locked(connection) => connection,
        }
    }
}

/// The connection source shared by every domain store. Cloning is cheap (an
/// `Arc`/pool-handle clone), so each store and the composition-root `AppState`
/// hold their own clone of the same underlying database.
#[derive(Clone)]
pub(crate) struct Database {
    db: Db,
    /// Test-only checkout counter (S2, #404 H1 skeptic m5): `Arc` because
    /// every domain store clones `self.db` (`AppState::kpi_ingest_runs()`
    /// etc.) — a non-shared counter would undercount. `#[cfg(test)]`-gated so
    /// it costs zero bytes in production; a call-count budget is a test-time
    /// assertion (`start_kpi_ingest_is_checkout_bounded`), never a runtime
    /// mechanism.
    #[cfg(test)]
    checkout_count: Arc<AtomicU64>,
}

impl Database {
    /// Wrap a single owned connection (test/in-memory path).
    pub(crate) fn from_connection(connection: Connection) -> Self {
        Self {
            db: Db::Single(Arc::new(Mutex::new(connection))),
            #[cfg(test)]
            checkout_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Wrap an r2d2 pool (production/concurrent path).
    pub(crate) fn from_pool(pool: SqlitePool) -> Self {
        Self {
            db: Db::Pool(pool),
            #[cfg(test)]
            checkout_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Check out a connection for a single storage operation.
    pub(crate) fn checkout(&self) -> StorageResult<DbGuard<'_>> {
        #[cfg(test)]
        self.checkout_count.fetch_add(1, Ordering::Relaxed);
        match &self.db {
            Db::Pool(pool) => Ok(DbGuard::Pooled(pool.get()?)),
            Db::Single(connection) => Ok(DbGuard::Locked(
                connection.lock().expect("database mutex poisoned"),
            )),
        }
    }

    /// Test-only read of the checkout counter (S2, #404 H1 skeptic m5) — the
    /// `start_kpi_ingest_is_checkout_bounded` guard reads a before/after
    /// delta through `AppState::checkout_count`.
    #[cfg(test)]
    pub(crate) fn checkout_count(&self) -> u64 {
        self.checkout_count.load(Ordering::Relaxed)
    }
}
