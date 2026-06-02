use super::*;
use std::path::Path;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "feed_item_display_company",
        sql: include_str!("../../migrations/0002_feed_item_display_company.sql"),
    },
    Migration {
        version: 3,
        name: "notebook_entry_origins",
        sql: include_str!("../../migrations/0003_notebook_entry_origins.sql"),
    },
    Migration {
        version: 4,
        name: "notebook_follow_ups",
        sql: include_str!("../../migrations/0004_notebook_follow_ups.sql"),
    },
    Migration {
        version: 5,
        name: "feed_item_attachments",
        sql: include_str!("../../migrations/0005_feed_item_attachments.sql"),
    },
    Migration {
        version: 6,
        name: "company_registry",
        sql: include_str!("../../migrations/0006_company_registry.sql"),
    },
    Migration {
        version: 7,
        name: "bankier_market_rss",
        sql: include_str!("../../migrations/0007_bankier_market_rss.sql"),
    },
    Migration {
        version: 8,
        name: "portal_analiz_source_placeholder",
        sql: include_str!("../../migrations/0008_portal_analiz_source_placeholder.sql"),
    },
    Migration {
        version: 9,
        name: "feed_item_duplicate_signatures",
        sql: include_str!("../../migrations/0009_feed_item_duplicate_signatures.sql"),
    },
    Migration {
        version: 10,
        name: "bankier_company_komunikaty",
        sql: include_str!("../../migrations/0010_bankier_company_komunikaty.sql"),
    },
    Migration {
        version: 11,
        name: "disable_gpw_espi_ebi",
        sql: include_str!("../../migrations/0011_disable_gpw_espi_ebi.sql"),
    },
    Migration {
        version: 12,
        name: "bankier_reviewed_rss_placeholders",
        sql: include_str!("../../migrations/0012_bankier_reviewed_rss_placeholders.sql"),
    },
    Migration {
        version: 13,
        name: "company_events",
        sql: include_str!("../../migrations/0013_company_events.sql"),
    },
    Migration {
        version: 14,
        name: "gpw_market_events_rss",
        sql: include_str!("../../migrations/0014_gpw_market_events_rss.sql"),
    },
    Migration {
        version: 15,
        name: "event_source_candidates",
        sql: include_str!("../../migrations/0015_event_source_candidates.sql"),
    },
    Migration {
        version: 16,
        name: "enable_bankier_kalendarium",
        sql: include_str!("../../migrations/0016_enable_bankier_kalendarium.sql"),
    },
    Migration {
        version: 17,
        name: "transcript_storage_foundation",
        sql: include_str!("../../migrations/0017_transcript_storage_foundation.sql"),
    },
    Migration {
        version: 18,
        name: "youtube_transcription_provider_id",
        sql: include_str!("../../migrations/0018_youtube_transcription_provider_id.sql"),
    },
    Migration {
        version: 19,
        name: "youtube_transcription_model",
        sql: include_str!("../../migrations/0019_youtube_transcription_model.sql"),
    },
    Migration {
        version: 20,
        name: "youtube_transcription_timeout",
        sql: include_str!("../../migrations/0020_youtube_transcription_timeout.sql"),
    },
    Migration {
        version: 21,
        name: "gemini_default_model_to_validated_flash",
        sql: include_str!("../../migrations/0021_gemini_default_model_to_validated_flash.sql"),
    },
    Migration {
        version: 22,
        name: "app_locale",
        sql: include_str!("../../migrations/0022_app_locale.sql"),
    },
];

pub fn open_database(path: impl AsRef<Path>) -> StorageResult<Connection> {
    let mut connection = Connection::open(path)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub fn open_in_memory_database() -> StorageResult<Connection> {
    let mut connection = Connection::open_in_memory()?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub(super) fn database_status(connection: &Connection) -> StorageResult<DatabaseStatus> {
    Ok(DatabaseStatus {
        applied_migrations: count_rows(connection, "schema_migrations")?,
        companies: count_rows(connection, "companies")?,
        source_adapters: count_rows(connection, "source_adapters")?,
        settings: count_rows(connection, "settings")?,
    })
}

pub(super) fn apply_migrations(connection: &mut Connection) -> StorageResult<()> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;

    let transaction = connection.transaction()?;

    for migration in MIGRATIONS {
        let already_applied: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;

        if already_applied {
            continue;
        }

        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            (migration.version, migration.name),
        )?;
    }

    transaction.commit()?;
    Ok(())
}

pub(super) fn count_rows(connection: &Connection, table_name: &str) -> StorageResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");

    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(StorageError::from)
}
