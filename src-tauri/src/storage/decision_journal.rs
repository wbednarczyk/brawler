//! Decision journal storage (ADR 0071, v0.52.0) — the early slice of the ADR
//! 0043 thesis-workbench journal. Records the user's own judgments so the
//! calibration record starts accumulating now; the v0.64 workbench extends
//! this table, never migrates away from it.
//!
//! Entries are IMMUTABLE once saved (DB `BEFORE UPDATE`/`BEFORE DELETE`
//! triggers `RAISE(ABORT)`, no update API). Corrections are appended as
//! follow-up entries: the follow-up's `superseded_by_entry_id` names the entry
//! superseded BY it, so the old row is never touched. Decision support only —
//! the app mirrors judgments back, it never grades them (ADR 0042).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{slug_part, StorageError, StorageResult};

/// The closed set of recorded judgment kinds (ADR 0071): the user's own
/// actions/judgments, never advice.
pub const DECISION_ENTRY_KINDS: &[&str] = &["buy", "pass", "keep_watching", "sell_note"];

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DecisionEntry {
    pub id: String,
    pub company_id: String,
    pub kind: String,
    pub rationale_md: String,
    /// Domain date (`YYYY-MM-DD`) the decision was made — the journal's
    /// chronology, distinct from the row's `created_at`.
    pub decided_at: String,
    /// Set on a FOLLOW-UP entry: the id of the entry superseded by this one.
    pub superseded_by_entry_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewDecisionEntry {
    pub company_id: String,
    pub kind: String,
    pub rationale_md: String,
    pub decided_at: String,
    #[serde(default)]
    pub superseded_by_entry_id: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct DecisionEntryListInput {
    /// Restrict to one company; `None` lists the global journal.
    #[serde(default)]
    pub company_id: Option<String>,
    /// Restrict to one decision kind.
    #[serde(default)]
    pub kind: Option<String>,
}

pub(super) fn create_decision_entry(
    connection: &Connection,
    input: NewDecisionEntry,
) -> StorageResult<DecisionEntry> {
    ensure_company_exists(connection, &input.company_id)?;
    validate_kind(&input.kind)?;
    let rationale_md = input.rationale_md.trim().to_owned();
    if rationale_md.is_empty() {
        return Err(invalid("rationaleMd", &input.rationale_md));
    }
    let decided_at = validate_decided_at(&input.decided_at)?;

    if let Some(superseded) = input.superseded_by_entry_id.as_deref() {
        // The superseded entry must exist and belong to the same company.
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM decision_entries WHERE id = ?1 AND company_id = ?2)",
            params![superseded, input.company_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StorageError::MissingResearchReference {
                table: "decision_entries".to_owned(),
                id: superseded.to_owned(),
            });
        }
    }

    let id = next_decision_entry_id(connection, &input.company_id)?;
    connection.execute(
        "
        INSERT INTO decision_entries
            (id, company_id, kind, rationale_md, decided_at, superseded_by_entry_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            id,
            input.company_id,
            input.kind,
            rationale_md,
            decided_at,
            input.superseded_by_entry_id,
        ],
    )?;

    get_decision_entry(connection, &id)
}

pub(super) fn list_decision_entries(
    connection: &Connection,
    input: DecisionEntryListInput,
) -> StorageResult<Vec<DecisionEntry>> {
    if let Some(kind) = input.kind.as_deref() {
        validate_kind(kind)?;
    }
    // The journal is a chronology of DECISIONS: ordered by the domain date the
    // decision was made (id as a stable tiebreak), never by row insertion.
    let mut statement = connection.prepare(
        "
        SELECT id, company_id, kind, rationale_md, decided_at, superseded_by_entry_id, created_at
        FROM decision_entries
        WHERE (?1 IS NULL OR company_id = ?1)
          AND (?2 IS NULL OR kind = ?2)
        ORDER BY decided_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map(params![input.company_id, input.kind], entry_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn get_decision_entry(connection: &Connection, id: &str) -> StorageResult<DecisionEntry> {
    connection
        .query_row(
            "
            SELECT id, company_id, kind, rationale_md, decided_at, superseded_by_entry_id,
                   created_at
            FROM decision_entries
            WHERE id = ?1
            ",
            [id],
            entry_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingResearchReference {
            table: "decision_entries".to_owned(),
            id: id.to_owned(),
        })
}

fn entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DecisionEntry> {
    Ok(DecisionEntry {
        id: row.get(0)?,
        company_id: row.get(1)?,
        kind: row.get(2)?,
        rationale_md: row.get(3)?,
        decided_at: row.get(4)?,
        superseded_by_entry_id: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn validate_kind(kind: &str) -> StorageResult<()> {
    if DECISION_ENTRY_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(invalid("kind", kind))
    }
}

/// The decided-at is a plain domain date, `YYYY-MM-DD`.
fn validate_decided_at(decided_at: &str) -> StorageResult<String> {
    let trimmed = decided_at.trim();
    let bytes = trimmed.as_bytes();
    let well_formed = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit());
    if well_formed {
        Ok(trimmed.to_owned())
    } else {
        Err(invalid("decidedAt", decided_at))
    }
}

fn invalid(key: &'static str, value: &str) -> StorageError {
    StorageError::InvalidResearchValue {
        key,
        value: value.to_owned(),
    }
}

fn ensure_company_exists(connection: &Connection, company_id: &str) -> StorageResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM companies WHERE id = ?1)",
        [company_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::MissingResearchReference {
            table: "companies".to_owned(),
            id: company_id.to_owned(),
        })
    }
}

/// Sequential per-company id. Safe because entries are append-only by
/// construction (the immutability triggers forbid DELETE), so the count never
/// shrinks.
fn next_decision_entry_id(connection: &Connection, company_id: &str) -> StorageResult<String> {
    let prefix = format!("decision_entry_{}", slug_part(company_id));
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM decision_entries WHERE id LIKE ?1",
        [format!("{prefix}_%")],
        |row| row.get(0),
    )?;
    Ok(format!("{prefix}_{}", count + 1))
}

use super::database::Database;
/// decision_journal domain store (Architecture v2 / ADR 0050). Owns a
/// [`Database`] and exposes only this domain's operations. Reach it via
/// `AppState::decision_journal()`.
#[derive(Clone)]
pub struct DecisionJournalStore {
    db: Database,
}

impl DecisionJournalStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_decision_entry(&self, input: NewDecisionEntry) -> StorageResult<DecisionEntry> {
        let connection = self.db.checkout()?;

        create_decision_entry(&connection, input)
    }

    pub fn list_decision_entries(
        &self,
        input: DecisionEntryListInput,
    ) -> StorageResult<Vec<DecisionEntry>> {
        let connection = self.db.checkout()?;

        list_decision_entries(&connection, input)
    }
}
