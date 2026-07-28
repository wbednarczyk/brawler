use serde::{Deserialize, Serialize};

use super::database::Database;
use super::*;

// Research cockpit layouts (ADR 0053, decision 3A). A *layout* is a named,
// user-saved panel arrangement for the dockview docking shell. `panels_json` is
// the app-owned source of truth (what is open + the linked selection);
// `layout_json` is the opaque dockview geometry (may be absent); the production
// cockpit persists here (NOT localStorage). Domain store per Architecture v2
// (ADR 0050); reach it via `AppState::cockpit_layouts()`.

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CockpitLayout {
    pub id: String,
    pub name: String,
    pub ordinal: i64,
    pub panels_json: String,
    pub layout_json: Option<String>,
    pub dockview_version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for `save_cockpit_layout` — upserts a layout by `name`.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "SaveCockpitLayoutInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewCockpitLayout {
    pub name: String,
    pub panels_json: String,
    pub layout_json: Option<String>,
    pub dockview_version: Option<String>,
}

#[derive(Clone)]
pub struct CockpitLayoutStore {
    db: Database,
}

impl CockpitLayoutStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_cockpit_layouts(&self) -> StorageResult<Vec<CockpitLayout>> {
        let connection = self.db.checkout()?;
        list_cockpit_layouts(&connection)
    }

    pub fn save_cockpit_layout(&self, input: NewCockpitLayout) -> StorageResult<CockpitLayout> {
        let connection = self.db.checkout()?;
        save_cockpit_layout(&connection, input)
    }

    pub fn delete_cockpit_layout(&self, layout_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_cockpit_layout(&connection, layout_id)
    }

    pub fn rename_cockpit_layout(
        &self,
        layout_id: &str,
        new_name: &str,
    ) -> StorageResult<CockpitLayout> {
        let connection = self.db.checkout()?;
        rename_cockpit_layout(&connection, layout_id, new_name)
    }
}

const SELECT_COLUMNS: &str =
    "id, name, ordinal, panels_json, layout_json, dockview_version, created_at, updated_at";

fn row_to_layout(row: &rusqlite::Row<'_>) -> rusqlite::Result<CockpitLayout> {
    Ok(CockpitLayout {
        id: row.get(0)?,
        name: row.get(1)?,
        ordinal: row.get(2)?,
        panels_json: row.get(3)?,
        layout_json: row.get(4)?,
        dockview_version: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

pub(super) fn list_cockpit_layouts(connection: &Connection) -> StorageResult<Vec<CockpitLayout>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM cockpit_layouts ORDER BY ordinal, name"
    ))?;
    let rows = statement.query_map([], row_to_layout)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn fetch_by_id(connection: &Connection, id: &str) -> StorageResult<Option<CockpitLayout>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM cockpit_layouts WHERE id = ?1"
    ))?;
    let mut rows = statement.query_map([id], row_to_layout)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

fn fetch_by_name(connection: &Connection, name: &str) -> StorageResult<Option<CockpitLayout>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM cockpit_layouts WHERE name = ?1"
    ))?;
    let mut rows = statement.query_map([name], row_to_layout)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// A stable id derived from the name, disambiguated if the slug is already taken
/// — so renaming a layout away and later re-creating its old name never collides
/// on the primary key.
fn unique_layout_id(connection: &Connection, name: &str) -> StorageResult<String> {
    let base = format!("layout_{}", slug_part(name));
    let mut candidate = base.clone();
    let mut suffix = 2;
    while fetch_by_id(connection, &candidate)?.is_some() {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    Ok(candidate)
}

pub(super) fn save_cockpit_layout(
    connection: &Connection,
    input: NewCockpitLayout,
) -> StorageResult<CockpitLayout> {
    let name = input.name.trim().to_owned();
    if name.is_empty() {
        return Err(StorageError::InvalidCockpitLayoutName { name: input.name });
    }

    // Upsert by name: an existing name updates that row in place (keeping its id
    // and created_at); a new name inserts at the end of the ordinal order.
    if let Some(existing) = fetch_by_name(connection, &name)? {
        connection.execute(
            "UPDATE cockpit_layouts
             SET panels_json = ?2,
                 layout_json = ?3,
                 dockview_version = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![
                existing.id,
                input.panels_json,
                input.layout_json,
                input.dockview_version
            ],
        )?;
        return fetch_by_id(connection, &existing.id)?
            .ok_or(StorageError::CockpitLayoutNotFound { id: existing.id });
    }

    let id = unique_layout_id(connection, &name)?;
    let next_ordinal: i64 = connection.query_row(
        "SELECT COALESCE(MAX(ordinal) + 1, 0) FROM cockpit_layouts",
        [],
        |row| row.get(0),
    )?;
    connection.execute(
        "INSERT INTO cockpit_layouts
            (id, name, ordinal, panels_json, layout_json, dockview_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            name,
            next_ordinal,
            input.panels_json,
            input.layout_json,
            input.dockview_version
        ],
    )?;
    fetch_by_id(connection, &id)?.ok_or(StorageError::CockpitLayoutNotFound { id })
}

/// Rename a saved layout in place (issue #89). Keeps the row id/ordinal — the
/// slug-style id is a naming convention, not a name dependency (see
/// `unique_layout_id`'s comment). The name must be non-empty and not collide
/// with another layout: `save_cockpit_layout` upserts BY NAME, so allowing a
/// duplicate here would silently fuse two layouts on the next save.
pub(super) fn rename_cockpit_layout(
    connection: &Connection,
    layout_id: &str,
    new_name: &str,
) -> StorageResult<CockpitLayout> {
    let name = new_name.trim().to_owned();
    if name.is_empty() {
        return Err(StorageError::InvalidCockpitLayoutName {
            name: new_name.to_owned(),
        });
    }
    let existing =
        fetch_by_id(connection, layout_id)?.ok_or(StorageError::CockpitLayoutNotFound {
            id: layout_id.to_owned(),
        })?;
    if let Some(other) = fetch_by_name(connection, &name)? {
        if other.id != existing.id {
            return Err(StorageError::DuplicateCockpitLayoutName { name });
        }
    }
    connection.execute(
        "UPDATE cockpit_layouts
         SET name = ?2,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        params![existing.id, name],
    )?;
    fetch_by_id(connection, &existing.id)?
        .ok_or(StorageError::CockpitLayoutNotFound { id: existing.id })
}

pub(super) fn delete_cockpit_layout(connection: &Connection, layout_id: &str) -> StorageResult<()> {
    connection.execute("DELETE FROM cockpit_layouts WHERE id = ?1", [layout_id])?;
    Ok(())
}
