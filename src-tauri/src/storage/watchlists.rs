use super::database::Database;
use super::*;

/// Watchlist domain store (Architecture v2 / ADR 0050). Owns its own
/// [`Database`] connection source and exposes only watchlist operations, so a
/// command can depend on this store instead of the whole `AppState` facade. The
/// SQL lives in the free functions below; the store is the thin checkout-and-call
/// surface. Reach it via `AppState::watchlists()`.
#[derive(Clone)]
pub struct WatchlistStore {
    db: Database,
}

impl WatchlistStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_watchlists(&self) -> StorageResult<Vec<Watchlist>> {
        let connection = self.db.checkout()?;
        list_watchlists(&connection)
    }

    pub fn list_watchlist_memberships(&self) -> StorageResult<Vec<WatchlistMembership>> {
        let connection = self.db.checkout()?;
        list_watchlist_memberships(&connection)
    }

    pub fn create_watchlist(&self, input: NewWatchlist) -> StorageResult<Watchlist> {
        let connection = self.db.checkout()?;
        create_watchlist(&connection, input)
    }

    pub fn rename_watchlist(&self, input: WatchlistUpdate) -> StorageResult<Watchlist> {
        let connection = self.db.checkout()?;
        rename_watchlist(&connection, input)
    }

    pub fn delete_watchlist(&self, watchlist_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_watchlist(&connection, watchlist_id)
    }

    pub fn add_company_to_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        add_company_to_watchlist(&connection, input)
    }

    pub fn remove_company_from_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        remove_company_from_watchlist(&connection, input)
    }
}

pub(super) fn list_watchlists(connection: &Connection) -> StorageResult<Vec<Watchlist>> {
    let mut statement = connection.prepare(
        "
        SELECT
            watchlists.id,
            watchlists.name,
            watchlists.description,
            COUNT(watchlist_companies.company_id) AS company_count
        FROM watchlists
        LEFT JOIN watchlist_companies
            ON watchlist_companies.watchlist_id = watchlists.id
        GROUP BY watchlists.id, watchlists.name, watchlists.description
        ORDER BY watchlists.name
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(Watchlist {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            company_count: row.get(3)?,
        })
    })?;

    let watchlists = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(watchlists)
}

pub(super) fn list_watchlist_memberships(
    connection: &Connection,
) -> StorageResult<Vec<WatchlistMembership>> {
    let mut statement = connection.prepare(
        "
        SELECT
            watchlists.id,
            watchlists.name,
            watchlist_companies.company_id
        FROM watchlist_companies
        INNER JOIN watchlists
            ON watchlists.id = watchlist_companies.watchlist_id
        ORDER BY watchlists.name, watchlist_companies.company_id
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(WatchlistMembership {
            watchlist_id: row.get(0)?,
            watchlist_name: row.get(1)?,
            company_id: row.get(2)?,
        })
    })?;

    let memberships = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(memberships)
}

pub(super) fn create_watchlist(
    connection: &Connection,
    input: NewWatchlist,
) -> StorageResult<Watchlist> {
    let name = input.name.trim().to_owned();
    let id = watchlist_id(&name);
    let description = empty_string_to_none(input.description);

    connection.execute(
        "
        INSERT INTO watchlists (id, name, description)
        VALUES (?1, ?2, ?3)
        ",
        params![id, name, description],
    )?;

    connection
        .query_row(
            "
            SELECT id, name, description, 0 AS company_count
            FROM watchlists
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(Watchlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    company_count: row.get(3)?,
                })
            },
        )
        .map_err(StorageError::from)
}

pub(super) fn rename_watchlist(
    connection: &Connection,
    input: WatchlistUpdate,
) -> StorageResult<Watchlist> {
    let id = input.id;
    let name = input.name.trim().to_owned();
    let description = empty_string_to_none(input.description);

    connection.execute(
        "
        UPDATE watchlists
        SET name = ?2,
            description = ?3,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id, name, description],
    )?;

    connection
        .query_row(
            "
            SELECT
                watchlists.id,
                watchlists.name,
                watchlists.description,
                COUNT(watchlist_companies.company_id) AS company_count
            FROM watchlists
            LEFT JOIN watchlist_companies
                ON watchlist_companies.watchlist_id = watchlists.id
            WHERE watchlists.id = ?1
            GROUP BY watchlists.id, watchlists.name, watchlists.description
            ",
            [id],
            |row| {
                Ok(Watchlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    company_count: row.get(3)?,
                })
            },
        )
        .map_err(StorageError::from)
}

pub(super) fn delete_watchlist(connection: &Connection, watchlist_id: &str) -> StorageResult<()> {
    connection.execute(
        "
        DELETE FROM watchlists
        WHERE id = ?1
        ",
        [watchlist_id],
    )?;

    Ok(())
}

pub(super) fn add_company_to_watchlist(
    connection: &Connection,
    input: WatchlistCompanyInput,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT OR IGNORE INTO watchlist_companies (watchlist_id, company_id)
        VALUES (?1, ?2)
        ",
        params![input.watchlist_id, input.company_id],
    )?;

    Ok(())
}

pub(super) fn remove_company_from_watchlist(
    connection: &Connection,
    input: WatchlistCompanyInput,
) -> StorageResult<()> {
    connection.execute(
        "
        DELETE FROM watchlist_companies
        WHERE watchlist_id = ?1
            AND company_id = ?2
        ",
        params![input.watchlist_id, input.company_id],
    )?;

    Ok(())
}

pub(super) fn company_is_in_watchlist(
    connection: &Connection,
    watchlist_id: &str,
    company_id: &str,
) -> StorageResult<bool> {
    connection
        .query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM watchlist_companies
                WHERE watchlist_id = ?1 AND company_id = ?2
            )
            ",
            params![watchlist_id, company_id],
            |row| row.get(0),
        )
        .map_err(StorageError::from)
}
