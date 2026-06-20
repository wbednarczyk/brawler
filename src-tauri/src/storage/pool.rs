use super::*;
use r2d2_sqlite::SqliteConnectionManager;
use std::time::Duration;

const DEFAULT_MAX_CONNECTIONS: u32 = 4;
const DEFAULT_BUSY_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_ACQUIRE_TIMEOUT_MS: u64 = 10_000;

const MIN_MAX_CONNECTIONS: i64 = 1;
const MAX_MAX_CONNECTIONS: i64 = 16;
const MIN_BUSY_TIMEOUT_MS: i64 = 0;
const MAX_BUSY_TIMEOUT_MS: i64 = 60_000;
const MIN_ACQUIRE_TIMEOUT_MS: i64 = 1_000;
const MAX_ACQUIRE_TIMEOUT_MS: i64 = 60_000;

const MAX_CONNECTIONS_KEY: &str = "db_max_connections";
const BUSY_TIMEOUT_KEY: &str = "db_busy_timeout_ms";
const ACQUIRE_TIMEOUT_KEY: &str = "db_acquire_timeout_ms";

/// Connection-pool tuning (ADR 0032). Read from settings with safe-range
/// clamping and default fallback so an invalid value can never block the
/// database from opening.
#[derive(Debug, Clone, Copy)]
pub(super) struct PoolConfig {
    pub max_connections: u32,
    pub busy_timeout_ms: u64,
    pub acquire_timeout_ms: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_MAX_CONNECTIONS,
            busy_timeout_ms: DEFAULT_BUSY_TIMEOUT_MS,
            acquire_timeout_ms: DEFAULT_ACQUIRE_TIMEOUT_MS,
        }
    }
}

pub(super) const MAX_CONNECTIONS_SETTING_KEY: &str = MAX_CONNECTIONS_KEY;
pub(super) const BUSY_TIMEOUT_SETTING_KEY: &str = BUSY_TIMEOUT_KEY;
pub(super) const ACQUIRE_TIMEOUT_SETTING_KEY: &str = ACQUIRE_TIMEOUT_KEY;

pub(super) fn clamp_max_connections(value: i64) -> i64 {
    value.clamp(MIN_MAX_CONNECTIONS, MAX_MAX_CONNECTIONS)
}

pub(super) fn clamp_busy_timeout_ms(value: i64) -> i64 {
    value.clamp(MIN_BUSY_TIMEOUT_MS, MAX_BUSY_TIMEOUT_MS)
}

pub(super) fn clamp_acquire_timeout_ms(value: i64) -> i64 {
    value.clamp(MIN_ACQUIRE_TIMEOUT_MS, MAX_ACQUIRE_TIMEOUT_MS)
}

fn read_setting_i64(connection: &Connection, key: &str) -> Option<i64> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())
}

pub(super) fn read_pool_config(connection: &Connection) -> PoolConfig {
    let defaults = PoolConfig::default();

    let max_connections = read_setting_i64(connection, MAX_CONNECTIONS_KEY)
        .map(|value| value.clamp(MIN_MAX_CONNECTIONS, MAX_MAX_CONNECTIONS) as u32)
        .unwrap_or(defaults.max_connections);
    let busy_timeout_ms = read_setting_i64(connection, BUSY_TIMEOUT_KEY)
        .map(|value| value.clamp(MIN_BUSY_TIMEOUT_MS, MAX_BUSY_TIMEOUT_MS) as u64)
        .unwrap_or(defaults.busy_timeout_ms);
    let acquire_timeout_ms = read_setting_i64(connection, ACQUIRE_TIMEOUT_KEY)
        .map(|value| value.clamp(MIN_ACQUIRE_TIMEOUT_MS, MAX_ACQUIRE_TIMEOUT_MS) as u64)
        .unwrap_or(defaults.acquire_timeout_ms);

    PoolConfig {
        max_connections,
        busy_timeout_ms,
        acquire_timeout_ms,
    }
}

/// Open the production database as a WAL connection pool (ADR 0032).
///
/// Bootstrap ordering: a single connection runs the pre-migration snapshot and
/// migrations and reads pool configuration, then the pool is built from that
/// configuration with per-connection pragmas.
pub fn open_pool(database_path: PathBuf, data_dir: PathBuf) -> StorageResult<AppState> {
    // Apply a staged restore (if requested last session) before opening anything.
    backup::apply_staged_restore(&database_path, &data_dir)?;

    let mut bootstrap = Connection::open(&database_path)?;
    bootstrap.pragma_update(None, "journal_mode", "WAL")?;
    bootstrap.pragma_update(None, "foreign_keys", "ON")?;

    backup::snapshot_before_migrations(&bootstrap, &data_dir)?;
    migrations::apply_migrations(&mut bootstrap)?;
    let config = read_pool_config(&bootstrap);

    // Automatic rotating backup at startup; best-effort so a backup failure never
    // blocks the app from opening (ADR 0032).
    if let Err(error) = backup::create_rotating_backup(&bootstrap, &data_dir) {
        log::warn!("startup backup failed: {error}");
    }

    drop(bootstrap);

    let busy_timeout_ms = config.busy_timeout_ms as i64;
    let manager = SqliteConnectionManager::file(&database_path).with_init(move |connection| {
        connection.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = ON;",
        )?;
        connection.pragma_update(None, "busy_timeout", busy_timeout_ms)?;
        Ok(())
    });

    let pool = r2d2::Pool::builder()
        .max_size(config.max_connections)
        .connection_timeout(Duration::from_millis(config.acquire_timeout_ms))
        .build(manager)?;

    Ok(AppState::with_pool(pool, data_dir))
}
