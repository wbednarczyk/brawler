//! Valuation-runs domain store (ADR 0089 dec. 5, v0.61 §B2): the append-only
//! `valuation_runs` history. One row per persisted `(company, method)` run;
//! appended ONLY when the input signature (the canonical `inputs_json`) differs
//! from that method's latest stored run. Newest-run selection orders by the
//! DOMAIN `data_as_of` date (never `created_at`). Reach it via
//! `AppState::valuation_runs()`. The v0.62 DCF engine writes new `method` values
//! into the same table (rows, not columns).

use serde::Serialize;

use super::database::Database;
use super::*;

/// A run to append. `fair_*` are per-share decimal-exact TEXT (`None` on a typed
/// absence for that method).
#[derive(Debug, Clone)]
pub struct NewValuationRun {
    pub company_id: String,
    pub method: String,
    pub inputs_json: String,
    pub fair_low: Option<String>,
    pub fair_base: Option<String>,
    pub fair_high: Option<String>,
    pub data_as_of: String,
    pub confidence_grade: String,
}

/// A stored run (the `list_valuation_runs` row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct StoredValuationRun {
    pub id: String,
    pub company_id: String,
    pub method: String,
    pub inputs_json: String,
    pub fair_low: Option<String>,
    pub fair_base: Option<String>,
    pub fair_high: Option<String>,
    pub data_as_of: String,
    pub confidence_grade: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct ValuationRunsStore {
    db: Database,
}

impl ValuationRunsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Append one run. The id is a collision-safe deterministic hash over the
    /// run's identity (company, method, inputs, created_at). Returns the id.
    pub fn insert_run(&self, run: &NewValuationRun) -> StorageResult<String> {
        let connection = self.db.checkout()?;
        let created_at = now_iso();
        let id = run_id(run, &created_at);
        connection.execute(
            "INSERT INTO valuation_runs
                (id, company_id, method, inputs_json, fair_low, fair_base, fair_high,
                 data_as_of, confidence_grade, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                run.company_id,
                run.method,
                run.inputs_json,
                run.fair_low,
                run.fair_base,
                run.fair_high,
                run.data_as_of,
                run.confidence_grade,
                created_at,
            ],
        )?;
        Ok(id)
    }

    /// The latest stored run for one `(company, method)` — the append gate reads
    /// its `inputs_json` to compare signatures. Newest by the DOMAIN `data_as_of`
    /// (created_at only tie-breaks within an as-of date).
    pub fn latest_run_for_method(
        &self,
        company_id: &str,
        method: &str,
    ) -> StorageResult<Option<StoredValuationRun>> {
        let connection = self.db.checkout()?;
        let run = connection
            .query_row(
                "SELECT id, company_id, method, inputs_json, fair_low, fair_base, fair_high,
                        data_as_of, confidence_grade, created_at
                 FROM valuation_runs
                 WHERE company_id = ?1 AND method = ?2
                 ORDER BY data_as_of DESC, created_at DESC
                 LIMIT 1",
                params![company_id, method],
                map_run_row,
            )
            .optional()?;
        Ok(run)
    }

    /// Every run for one company, newest-first by the DOMAIN `data_as_of` date
    /// (never `created_at`), created_at tie-breaking within an as-of date.
    pub fn list_runs(&self, company_id: &str) -> StorageResult<Vec<StoredValuationRun>> {
        let connection = self.db.checkout()?;
        let mut stmt = connection.prepare(
            "SELECT id, company_id, method, inputs_json, fair_low, fair_base, fair_high,
                    data_as_of, confidence_grade, created_at
             FROM valuation_runs
             WHERE company_id = ?1
             ORDER BY data_as_of DESC, created_at DESC",
        )?;
        let rows = stmt
            .query_map([company_id], map_run_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

fn map_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredValuationRun> {
    Ok(StoredValuationRun {
        id: row.get(0)?,
        company_id: row.get(1)?,
        method: row.get(2)?,
        inputs_json: row.get(3)?,
        fair_low: row.get(4)?,
        fair_base: row.get(5)?,
        fair_high: row.get(6)?,
        data_as_of: row.get(7)?,
        confidence_grade: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// Collision-safe deterministic id (sha256 hex, truncated) over the run identity.
fn run_id(run: &NewValuationRun, created_at: &str) -> String {
    use sha2::{Digest, Sha256};
    let key = format!(
        "valrun:{}\u{1f}{}\u{1f}{}\u{1f}{}",
        run.company_id, run.method, run.inputs_json, created_at
    );
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("valrun_{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations::open_in_memory_database;

    fn new_run(company: &str, method: &str, inputs: &str, as_of: &str) -> NewValuationRun {
        NewValuationRun {
            company_id: company.to_owned(),
            method: method.to_owned(),
            inputs_json: inputs.to_owned(),
            fair_low: Some("10".to_owned()),
            fair_base: Some("12".to_owned()),
            fair_high: Some("14".to_owned()),
            data_as_of: as_of.to_owned(),
            confidence_grade: "B".to_owned(),
        }
    }

    #[test]
    fn insert_and_list_round_trip() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.valuation_runs();
        store
            .insert_run(&new_run("c1", "pe_multiple", "{\"a\":1}", "2026-07-01"))
            .expect("insert");
        let runs = store.list_runs("c1").expect("list");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].method, "pe_multiple");
        assert_eq!(runs[0].fair_base.as_deref(), Some("12"));
        assert_eq!(runs[0].confidence_grade, "B");
    }

    #[test]
    fn latest_run_for_method_returns_the_newest_by_data_as_of() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.valuation_runs();
        // Insert an OLDER-as-of run SECOND (created_at later). The newest by the
        // DOMAIN date must win, NOT the wall-clock insert order.
        store
            .insert_run(&new_run(
                "c1",
                "pe_multiple",
                "{\"v\":\"new\"}",
                "2026-07-10",
            ))
            .expect("insert new-as-of");
        store
            .insert_run(&new_run(
                "c1",
                "pe_multiple",
                "{\"v\":\"old\"}",
                "2026-01-01",
            ))
            .expect("insert old-as-of (later created_at)");
        let latest = store
            .latest_run_for_method("c1", "pe_multiple")
            .expect("query")
            .expect("some");
        assert_eq!(
            latest.data_as_of, "2026-07-10",
            "newest selection orders by data_as_of, never created_at"
        );
        assert_eq!(latest.inputs_json, "{\"v\":\"new\"}");
    }

    #[test]
    fn list_orders_by_domain_date_not_created_at() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.valuation_runs();
        // Insert out of domain-date order; the later insert has the OLDER as-of.
        store
            .insert_run(&new_run("c1", "pe_multiple", "{\"i\":2}", "2026-07-10"))
            .expect("i2");
        store
            .insert_run(&new_run("c1", "pbv_multiple", "{\"i\":1}", "2026-01-01"))
            .expect("i1");
        let runs = store.list_runs("c1").expect("list");
        assert_eq!(runs.len(), 2);
        // Newest domain date first, regardless of the later created_at on the old one.
        assert_eq!(runs[0].data_as_of, "2026-07-10");
        assert_eq!(runs[1].data_as_of, "2026-01-01");
    }

    #[test]
    fn latest_is_none_for_unknown_company_or_method() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let store = state.valuation_runs();
        assert!(store
            .latest_run_for_method("nope", "pe_multiple")
            .expect("q")
            .is_none());
    }
}
