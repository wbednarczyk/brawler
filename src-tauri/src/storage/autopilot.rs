//! Autonomous report pipeline storage operations (North Star, v0.49.0, ADR 0055).
//!
//! Two tables, one domain store ([`AutopilotStore`], AV1 pattern):
//! - `company_autopilot_settings` — the per-company trust-ladder mode
//!   (`off` | `assist` | `autopilot`). A company with no row reads as `off`.
//! - `autopilot_run` — one durable row per autonomous run, backing the single
//!   notification, the review queue, and run-level undo.
//!
//! The global confirm-before-commit guarantee is unchanged; these tables only
//! scope automation to companies the user explicitly opts in. Reach the store via
//! `AppState::autopilot()`.

use super::database::Database;
use super::*;

/// The per-company trust-ladder mode. Stored as a string with a DB `CHECK`.
pub const MODE_OFF: &str = "off";
pub const MODE_ASSIST: &str = "assist";
pub const MODE_AUTOPILOT: &str = "autopilot";

/// Returns `true` if `mode` is a valid trust-ladder value.
pub fn is_valid_mode(mode: &str) -> bool {
    matches!(mode, MODE_OFF | MODE_ASSIST | MODE_AUTOPILOT)
}

/// A company's trust-ladder mode (read model). A company with no setting reports
/// `off`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "CompanyAutopilot"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyAutopilot {
    pub company_id: String,
    pub mode: String,
}

/// One autonomous run: the durable record behind the single notification, the
/// review queue, and run-level undo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "AutopilotRun"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotRun {
    pub id: String,
    pub company_id: String,
    pub report_document_id: String,
    pub trigger: String,
    pub mode: String,
    /// The history sweep that spawned this run, if any (ADR 0077 §6). `None` for
    /// a detection/manual run, and for legacy rows predating the column. Lets a
    /// sweep-triggered run charge the sweep's tier-4 budget in `stage_extract`.
    pub sweep_id: Option<String>,
    pub status: String,
    pub stage: String,
    pub summary_text: Option<String>,
    pub kpi_delta_json: Option<String>,
    pub report_diff_ref: Option<String>,
    pub cross_refs_json: Option<String>,
    /// `financial_facts` ids this run produced (parsed from the JSON column), so a
    /// single undo reverts exactly those facts.
    pub produced_fact_ids: Vec<String>,
    pub notification_state: String,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Filter for [`AutopilotStore::list_runs`]. All fields optional; `None` = no
/// filter on that dimension.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "ListAutopilotRunsInput",
        optional_fields
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ListAutopilotRunsInput {
    pub company_id: Option<String>,
    pub notification_state: Option<String>,
    pub limit: Option<i64>,
}

/// Autonomous report pipeline domain store.
#[derive(Clone)]
pub struct AutopilotStore {
    db: Database,
}

impl AutopilotStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Read a company's trust-ladder mode. A company with no row reads as `off`
    /// (a safe default — reads tolerate a missing row).
    pub fn get_mode(&self, company_id: &str) -> StorageResult<String> {
        let connection = self.db.checkout()?;
        let mode: Option<String> = connection
            .query_row(
                "SELECT mode FROM company_autopilot_settings WHERE company_id = ?1",
                [company_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(mode.unwrap_or_else(|| MODE_OFF.to_owned()))
    }

    /// Upsert a company's trust-ladder mode. Changing the mode never touches
    /// already-produced facts or runs.
    pub fn set_mode(&self, company_id: &str, mode: &str) -> StorageResult<CompanyAutopilot> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            INSERT INTO company_autopilot_settings (company_id, mode)
            VALUES (?1, ?2)
            ON CONFLICT(company_id) DO UPDATE
                SET mode = excluded.mode,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![company_id, mode],
        )?;
        Ok(CompanyAutopilot {
            company_id: company_id.to_owned(),
            mode: mode.to_owned(),
        })
    }

    /// Every company that has an explicit autopilot mode row (i.e. not the default
    /// `off`). The per-company settings surface (ADR 0056) defaults the rest to
    /// `off` from the full company list.
    pub fn list_modes(&self) -> StorageResult<Vec<CompanyAutopilot>> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(
            "SELECT company_id, mode FROM company_autopilot_settings ORDER BY company_id",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(CompanyAutopilot {
                    company_id: row.get(0)?,
                    mode: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Set the same mode on many companies at once (bulk action from the settings
    /// surface, ADR 0056). Returns the number of companies updated.
    pub fn set_modes(&self, company_ids: &[String], mode: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        for company_id in company_ids {
            connection.execute(
                "
                INSERT INTO company_autopilot_settings (company_id, mode)
                VALUES (?1, ?2)
                ON CONFLICT(company_id) DO UPDATE
                    SET mode = excluded.mode,
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                ",
                params![company_id, mode],
            )?;
        }
        Ok(company_ids.len())
    }

    /// Company ids opted into automation (`mode != off`). The detection sweep
    /// scans these after each source refresh.
    pub fn opted_in_company_ids(&self) -> StorageResult<Vec<String>> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(
            "SELECT company_id FROM company_autopilot_settings WHERE mode != 'off' ORDER BY company_id",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// Document ids whose autopilot run is a **budget-denied history sweep**: the
    /// `trigger='history_sweep'` run for that document recorded
    /// `{ extractionAvailable: false, reason: "skipped_budget" }` on its
    /// `kpi_delta_json` (ADR 0077 §6). Feeds the coverage read model's
    /// `skipped_budget` projection.
    ///
    /// Run ids are per-`(company, document)` deterministic
    /// (`create_run_if_absent`), so there is at most ONE run row per document — a
    /// plain lookup, no windowing. A later successful re-run overwrites that run's
    /// delta and drops it from this set; a repaired extraction also makes the
    /// period a non-gap regardless, so the flag clears naturally either way.
    ///
    /// JSON parsing is tolerant: an absent/garbled `kpi_delta_json`, or a `reason`
    /// other than `skipped_budget`, excludes the document.
    pub fn documents_skipped_by_budget(
        &self,
        company_id: &str,
    ) -> StorageResult<std::collections::BTreeSet<String>> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(
            "SELECT report_document_id, kpi_delta_json FROM autopilot_run \
             WHERE company_id = ?1 AND trigger = 'history_sweep'",
        )?;
        let rows = statement
            .query_map([company_id], |row| {
                Ok((
                    row.get::<_, String>("report_document_id")?,
                    row.get::<_, Option<String>>("kpi_delta_json")?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(document_id, delta)| {
                kpi_delta_reason_is_skipped_budget(delta.as_deref()).then_some(document_id)
            })
            .collect())
    }

    /// Create a run for a detected report, idempotently. Returns `Some(run)` when a
    /// new row was inserted, `None` when one already exists for this
    /// `(company_id, report_document_id)` — the detection dedup that guarantees at
    /// most one run per report.
    ///
    /// Self-heal: a prior run that **failed without producing any fact** (e.g. a
    /// transient "Gemini service unavailable" 503 during extraction) is deleted
    /// first, so the same detection sweep re-creates and re-runs it. The `id` is
    /// deterministic (`autopilot_run:{company}:{document}`), so this only ever
    /// clears the exact stale run for this report; a `succeeded`/`partial` run, or
    /// a `failed` run that already committed facts, is preserved and still dedups.
    pub fn create_run_if_absent(
        &self,
        id: &str,
        company_id: &str,
        report_document_id: &str,
        trigger: &str,
        mode: &str,
        sweep_id: Option<&str>,
    ) -> StorageResult<Option<AutopilotRun>> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            DELETE FROM autopilot_run
            WHERE id = ?1
                AND status = 'failed'
                AND (produced_fact_ids_json IS NULL OR produced_fact_ids_json IN ('[]', ''))
            ",
            params![id],
        )?;
        let inserted = connection.execute(
            "
            INSERT OR IGNORE INTO autopilot_run
                (id, company_id, report_document_id, trigger, mode, sweep_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![id, company_id, report_document_id, trigger, mode, sweep_id],
        )?;
        if inserted == 0 {
            return Ok(None);
        }
        drop(connection);
        self.get_run(id).map(Some)
    }

    /// Fetch one run by id.
    pub fn get_run(&self, id: &str) -> StorageResult<AutopilotRun> {
        let connection = self.db.checkout()?;
        connection
            .query_row(
                "SELECT * FROM autopilot_run WHERE id = ?1",
                [id],
                map_run_row,
            )
            .map_err(StorageError::from)
    }

    /// List recent runs for the notification/review surface, newest first.
    pub fn list_runs(&self, input: &ListAutopilotRunsInput) -> StorageResult<Vec<AutopilotRun>> {
        let connection = self.db.checkout()?;
        let limit = input.limit.unwrap_or(50).clamp(1, 500);
        let mut statement = connection.prepare(
            "
            SELECT * FROM autopilot_run
            WHERE (?1 IS NULL OR company_id = ?1)
                AND (?2 IS NULL OR notification_state = ?2)
            ORDER BY created_at DESC, id DESC
            LIMIT ?3
            ",
        )?;
        let runs = statement
            .query_map(
                params![input.company_id, input.notification_state, limit],
                map_run_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }

    /// Move a run to `running` at a given stage.
    pub fn set_run_stage(&self, id: &str, stage: &str, status: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE autopilot_run
            SET stage = ?2,
                status = ?3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, stage, status],
        )?;
        Ok(())
    }

    /// Append produced `financial_facts` ids to the run (for run-level undo). Ids
    /// are merged into the existing JSON array, de-duplicated.
    pub fn add_produced_facts(&self, id: &str, fact_ids: &[String]) -> StorageResult<()> {
        if fact_ids.is_empty() {
            return Ok(());
        }
        let mut run = self.get_run(id)?;
        for fact_id in fact_ids {
            if !run.produced_fact_ids.contains(fact_id) {
                run.produced_fact_ids.push(fact_id.clone());
            }
        }
        let json =
            serde_json::to_string(&run.produced_fact_ids).unwrap_or_else(|_| "[]".to_owned());
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE autopilot_run
            SET produced_fact_ids_json = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, json],
        )?;
        Ok(())
    }

    /// Record the extract stage's KPI delta summary (JSON).
    pub fn set_kpi_delta_json(&self, id: &str, json: &str) -> StorageResult<()> {
        self.set_field(id, "kpi_delta_json", json)
    }

    /// Record the diff stage's report-diff reference (JSON: the document pair).
    pub fn set_report_diff_ref(&self, id: &str, json: &str) -> StorageResult<()> {
        self.set_field(id, "report_diff_ref", json)
    }

    /// Record the cross-reference stage's claims/questions summary (JSON).
    pub fn set_cross_refs_json(&self, id: &str, json: &str) -> StorageResult<()> {
        self.set_field(id, "cross_refs_json", json)
    }

    fn set_field(&self, id: &str, column: &str, value: &str) -> StorageResult<()> {
        // `column` is a fixed internal literal (never user input), so the format is safe.
        let connection = self.db.checkout()?;
        connection.execute(
            &format!(
                "UPDATE autopilot_run SET {column} = ?2, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1"
            ),
            params![id, value],
        )?;
        Ok(())
    }

    /// Finalize the run's status and stage with the composed summary. `status` is
    /// `succeeded` | `partial` | `failed`; a `partial`/`failed` run still carries a
    /// summary of how far it got. The per-stage result columns (kpi delta, diff
    /// ref, cross-refs) are written incrementally as stages complete.
    pub fn finalize_run(
        &self,
        id: &str,
        status: &str,
        stage: &str,
        summary_text: Option<&str>,
        last_error: Option<&str>,
    ) -> StorageResult<AutopilotRun> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE autopilot_run
            SET status = ?2,
                stage = ?3,
                summary_text = ?4,
                last_error = ?5,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, status, stage, summary_text, last_error],
        )?;
        drop(connection);
        self.get_run(id)
    }

    /// Re-arm a TERMINAL run for a fresh extraction attempt (ADR 0077 §3,
    /// 2026-07-10): reset it to `pending`/`fetch` and re-stamp the CURRENT driving
    /// `trigger` + `sweep_id`, so a re-run enters `stage_extract` again (a
    /// finalized run's stages are idempotent no-ops otherwise) and charges the
    /// *current* sweep's budget, never an exhausted prior one. Only ever called for
    /// a succeeded run that recorded `extractionAvailable:false` — it produced no
    /// facts, so the produced-fact list is left untouched (a later emit merges into
    /// it, de-duplicated). `last_error` is cleared.
    pub fn rearm_run(&self, id: &str, trigger: &str, sweep_id: Option<&str>) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE autopilot_run
            SET status = 'pending',
                stage = 'fetch',
                trigger = ?2,
                sweep_id = ?3,
                last_error = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, trigger, sweep_id],
        )?;
        Ok(())
    }

    /// Clear the run's produced-fact list after a run-level undo, so a repeated
    /// undo is a no-op (idempotent).
    pub fn clear_produced_facts(&self, id: &str) -> StorageResult<()> {
        self.set_field(id, "produced_fact_ids_json", "[]")
    }

    /// Set a run's notification state (`unread` | `read` | `dismissed`).
    pub fn set_notification_state(&self, id: &str, state: &str) -> StorageResult<AutopilotRun> {
        let connection = self.db.checkout()?;
        connection.execute(
            "
            UPDATE autopilot_run
            SET notification_state = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![id, state],
        )?;
        drop(connection);
        self.get_run(id)
    }
}

/// `true` iff a run's `kpi_delta_json` decodes to an object whose `reason` is
/// exactly `skipped_budget`. Tolerant: `None`, non-JSON, or any other reason
/// returns `false` (a denied run is never a silent skip, but a garbled column is
/// never a false positive either).
fn kpi_delta_reason_is_skipped_budget(kpi_delta_json: Option<&str>) -> bool {
    kpi_delta_json
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| {
            value
                .get("reason")
                .and_then(|reason| reason.as_str())
                .map(|reason| reason == "skipped_budget")
        })
        .unwrap_or(false)
}

fn map_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutopilotRun> {
    let produced_json: String = row.get("produced_fact_ids_json")?;
    let produced_fact_ids = serde_json::from_str::<Vec<String>>(&produced_json).unwrap_or_default();
    Ok(AutopilotRun {
        id: row.get("id")?,
        company_id: row.get("company_id")?,
        report_document_id: row.get("report_document_id")?,
        trigger: row.get("trigger")?,
        mode: row.get("mode")?,
        sweep_id: row.get("sweep_id")?,
        status: row.get("status")?,
        stage: row.get("stage")?,
        summary_text: row.get("summary_text")?,
        kpi_delta_json: row.get("kpi_delta_json")?,
        report_diff_ref: row.get("report_diff_ref")?,
        cross_refs_json: row.get("cross_refs_json")?,
        produced_fact_ids,
        notification_state: row.get("notification_state")?,
        last_error: row.get("last_error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
