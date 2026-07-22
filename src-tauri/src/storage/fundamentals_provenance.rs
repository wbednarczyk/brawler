//! Structured-first extraction provenance + per-company profiles (ADR 0061 S5).
//!
//! Two additive tables, one domain store ([`FundamentalsProvenanceStore`]):
//! - `financial_fact_provenance` — per fact, which tier produced it and its
//!   validation verdict. Modeled separately from `financial_facts` (joined by id)
//!   so the central facts contract is untouched.
//! - `fundamentals_extraction_outcomes` — per attempted (company, document,
//!   period) SLOT, what the pipeline concluded, including the runs that emitted
//!   nothing. This is the persistence half of the ADR 0061 guardrail ("flagged,
//!   never silently stored"): provenance above is keyed by `fact_id`, so it
//!   structurally cannot record a Flagged/Empty/unreadable run, and
//!   `diagnostic_events` is developer-mode gated and trimmed to 7 days — neither
//!   can answer "was this period ever attempted, and what objected?".
//!
//! The AI-era `company_ocr_extraction_profile` table is DROPPED by migration
//! 0102 (ADR 0084 decision 5, clean cut). The `company_extraction_profile` table
//! is KEPT append-only (never DROP), but its read/write CODE is RETIRED with the
//! PDF fact arm (ADR 0086 dec. 1) — no path reads or writes a per-company PDF
//! layout profile anymore.
//!
//! Reach the store via `AppState::fundamentals_provenance()`.

use super::database::Database;
use super::*;

/// Per-fact structured-extraction provenance (read model).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FactProvenance {
    pub fact_id: String,
    /// esef | structured_xhtml | espi_cover_note | pdf | html_aggregator
    /// (`ai_text` appears only on rows written before ADR 0084 retired the AI
    /// tier; they stay readable, never rewritten)
    pub source_tier: String,
    /// passed | witness_confirmed | unreviewed | flagged | none
    pub validation_status: String,
    /// Serialized `DriftReport` JSON when a drift accompanied the fact.
    pub drift_json: Option<String>,
    /// The source concept/label this value was read from (primary citation).
    pub citation: Option<String>,
}

/// Fact count for one `source_tier` — one row of the rebuild verdict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TierFactCount {
    /// esef | structured_xhtml | espi_cover_note | pdf | html_aggregator | ai_text
    pub source_tier: String,
    /// Number of stored facts carrying a provenance row for this tier.
    pub facts: i64,
}

/// The full per-tier fact breakdown plus the manual / no-provenance bucket — the
/// before/after verdict `rebuild fundamentals` reports (ADR 0086 dec. 6).
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FactTierBreakdown {
    /// One entry per `source_tier` present in `financial_fact_provenance`,
    /// ordered by tier name for a stable verdict.
    pub by_tier: Vec<TierFactCount>,
    /// Facts with NO provenance row: manual entries (`extraction_method='manual'`)
    /// and any pre-pipeline legacy fact. Counted separately — the automaton never
    /// stamps a tier onto a hand-entered value.
    pub manual_or_unprovenanced: i64,
}

/// Input for recording provenance for one produced fact.
pub struct NewFactProvenance<'a> {
    pub fact_id: &'a str,
    pub source_tier: &'a str,
    pub validation_status: &'a str,
    pub drift_json: Option<&'a str>,
    pub citation: Option<&'a str>,
}

/// Upserts provenance for a produced fact on a caller-owned connection — the
/// seam the ingest-time ESPI cover-note tier (which runs inside the Bankier
/// ingest's connection) shares with [`FundamentalsProvenanceStore`].
pub(super) fn set_fact_provenance(
    connection: &Connection,
    input: NewFactProvenance<'_>,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO financial_fact_provenance
            (fact_id, source_tier, validation_status, drift_json, citation)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(fact_id) DO UPDATE SET
            source_tier = excluded.source_tier,
            validation_status = excluded.validation_status,
            drift_json = excluded.drift_json,
            citation = excluded.citation,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            input.fact_id,
            input.source_tier,
            input.validation_status,
            input.drift_json,
            input.citation,
        ],
    )?;
    Ok(())
}

/// The `source_tier` recorded for a fact, if any. `None` for a fact with no
/// provenance row (manual entry / pre-pipeline legacy) — such a fact is never
/// outranked by a deterministic tier.
pub(super) fn fact_source_tier(
    connection: &Connection,
    fact_id: &str,
) -> StorageResult<Option<String>> {
    let tier: Option<String> = connection
        .query_row(
            "SELECT source_tier FROM financial_fact_provenance WHERE fact_id = ?1",
            [fact_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(tier)
}

/// One attempted extraction slot and what the pipeline concluded about it —
/// including (especially) the attempts that emitted nothing.
///
/// A row exists for **every** attempt, emitting or not, so absence of a row is
/// the unambiguous signal "never attempted" and a flagged period can never be
/// confused with one the pipeline never reached.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionOutcome {
    pub id: String,
    pub company_id: String,
    pub report_document_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
    /// The tier that produced (or attempted) the set; `None` when no
    /// deterministic tier could read the document at all.
    pub tier: Option<String>,
    /// accepted | accepted_via_witness | accepted_unreviewed | flagged | empty
    pub acceptance: String,
    /// The TYPED reason this run landed where it did (never prose, ADR 0084 §6):
    /// `emitted` | `validation_failed` | `structure_drift` |
    /// `witness_disagreement` | `witness_fallback` | `no_deterministic_tier` |
    /// `no_period_derived` | `document_unreadable`.
    ///
    /// `witness_fallback` (ADR 0085 amendment, 2026-07-21) means at least one
    /// value was SOURCED from the aggregator because no deterministic tier read
    /// it — distinct from `emitted` (the filing was parsed) and from
    /// `witness_disagreement` (the filing was parsed and the witness differed).
    /// The authoritative vocabulary is the CHECK in migration `0105`.
    pub reason_code: String,
    /// Structured detail behind `reason_code` — the failing identity checks and
    /// comparative cross-checks with their residuals, or the read/parse error.
    pub detail_json: Option<String>,
    /// The serialized `DriftReport` when layout drift was detected. Persisted
    /// even on a non-emitting run (ADR 0061 decision 3's learning loop reads it).
    pub drift_json: Option<String>,
    pub structure_changed: bool,
    pub fact_count: i64,
    pub attempt_count: i64,
    pub first_attempted_at: String,
    pub last_attempted_at: String,
}

/// Input for recording one extraction attempt's outcome.
pub struct NewExtractionOutcome<'a> {
    pub company_id: &'a str,
    pub report_document_id: &'a str,
    pub fiscal_year: i64,
    pub period_type: &'a str,
    pub period_end: &'a str,
    pub tier: Option<&'a str>,
    pub acceptance: &'a str,
    pub reason_code: &'a str,
    pub detail_json: Option<&'a str>,
    pub drift_json: Option<&'a str>,
    pub structure_changed: bool,
    pub fact_count: i64,
}

/// Deterministic id for an outcome slot, so re-runs upsert the same row whether
/// or not the unique index is consulted.
fn extraction_outcome_id(
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let key = format!("{company_id}|{report_document_id}|{fiscal_year}|{period_type}|{period_end}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("fxo_{hex}")
}

/// Connection-level extraction-outcome upsert — the single SQL body shared by
/// the pooled [`FundamentalsProvenanceStore::record_extraction_outcome`] and the
/// ingest-time cover-note witness path, which holds a raw `&Connection` post-
/// commit (no pool handle) and must record the SAME typed outcome rows the
/// structured pipeline does, keyed by the same deterministic slot id.
pub(crate) fn record_extraction_outcome(
    connection: &Connection,
    input: NewExtractionOutcome<'_>,
) -> StorageResult<String> {
    let id = extraction_outcome_id(
        input.company_id,
        input.report_document_id,
        input.fiscal_year,
        input.period_type,
        input.period_end,
    );
    connection.execute(
        "
        INSERT INTO fundamentals_extraction_outcomes (
            id, company_id, report_document_id, fiscal_year, period_type,
            period_end, tier, acceptance, reason_code, detail_json,
            drift_json, structure_changed, fact_count
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(id) DO UPDATE SET
            tier = excluded.tier,
            acceptance = excluded.acceptance,
            reason_code = excluded.reason_code,
            detail_json = excluded.detail_json,
            drift_json = excluded.drift_json,
            structure_changed = excluded.structure_changed,
            fact_count = excluded.fact_count,
            attempt_count = fundamentals_extraction_outcomes.attempt_count + 1,
            last_attempted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            id,
            input.company_id,
            input.report_document_id,
            input.fiscal_year,
            input.period_type,
            input.period_end,
            input.tier,
            input.acceptance,
            input.reason_code,
            input.detail_json,
            input.drift_json,
            i64::from(input.structure_changed),
            input.fact_count,
        ],
    )?;
    Ok(id)
}

pub struct FundamentalsProvenanceStore {
    db: Database,
}

impl FundamentalsProvenanceStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Upserts provenance for a produced fact.
    pub fn set_fact_provenance(&self, input: NewFactProvenance<'_>) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        set_fact_provenance(&connection, input)
    }

    /// Fact counts by `source_tier`, plus the manual / no-provenance bucket.
    /// Powers the `rebuild fundamentals` before/after verdict (ADR 0086 dec. 6):
    /// the operator sees `html_aggregator ≫ 0`, `esef`/`espi_cover_note` rebuilt,
    /// and no stray `pdf` (`extraction_method='api'`) facts, without extra queries.
    pub fn count_facts_by_tier(&self) -> StorageResult<FactTierBreakdown> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(
            "SELECT source_tier, COUNT(*) FROM financial_fact_provenance
             GROUP BY source_tier ORDER BY source_tier",
        )?;
        let by_tier = statement
            .query_map([], |row| {
                Ok(TierFactCount {
                    source_tier: row.get(0)?,
                    facts: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // Facts with no provenance row: manual entries + pre-pipeline legacy.
        let manual_or_unprovenanced = connection.query_row(
            "SELECT COUNT(*) FROM financial_facts f
             LEFT JOIN financial_fact_provenance p ON p.fact_id = f.id
             WHERE p.fact_id IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(FactTierBreakdown {
            by_tier,
            manual_or_unprovenanced,
        })
    }

    /// Reads provenance for a fact. Absent for legacy facts predating the
    /// pipeline (safe: caller treats as unknown tier / unvalidated).
    pub fn get_fact_provenance(&self, fact_id: &str) -> StorageResult<Option<FactProvenance>> {
        let connection = self.db.checkout()?;
        let row = connection
            .query_row(
                "SELECT fact_id, source_tier, validation_status, drift_json, citation
                 FROM financial_fact_provenance WHERE fact_id = ?1",
                [fact_id],
                fact_provenance_from_row,
            )
            .optional()?;
        Ok(row)
    }

    /// Provenance for many facts at once (batch read for the KPI display).
    pub fn get_many(&self, fact_ids: &[String]) -> StorageResult<Vec<FactProvenance>> {
        if fact_ids.is_empty() {
            return Ok(Vec::new());
        }
        let connection = self.db.checkout()?;
        let mut out = Vec::new();
        for id in fact_ids {
            if let Some(p) = connection
                .query_row(
                    "SELECT fact_id, source_tier, validation_status, drift_json, citation
                     FROM financial_fact_provenance WHERE fact_id = ?1",
                    [id],
                    fact_provenance_from_row,
                )
                .optional()?
            {
                out.push(p);
            }
        }
        Ok(out)
    }

    /// Every fact currently flagged (a drift or contradiction) — the review /
    /// "structure changed" surface.
    pub fn list_flagged(&self) -> StorageResult<Vec<FactProvenance>> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(
            "SELECT fact_id, source_tier, validation_status, drift_json, citation
             FROM financial_fact_provenance
             WHERE validation_status = 'flagged'
             ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], fact_provenance_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Records the outcome of one extraction attempt, upserting the slot.
    ///
    /// Re-running a period **updates** its row rather than appending, so the
    /// read model always reports the pipeline's current verdict — a flag that
    /// has since been fixed must not linger in the review surface. Only
    /// `attempt_count` and `first_attempted_at` accumulate across re-runs.
    pub fn record_extraction_outcome(
        &self,
        input: NewExtractionOutcome<'_>,
    ) -> StorageResult<String> {
        let connection = self.db.checkout()?;
        record_extraction_outcome(&connection, input)
    }

    /// One outcome by id — the retry command's lookup, so a re-run targets the
    /// exact recorded slot instead of re-deriving a period the UI would have to
    /// invent.
    pub fn get_extraction_outcome(&self, id: &str) -> StorageResult<Option<ExtractionOutcome>> {
        let connection = self.db.checkout()?;
        let row = connection
            .query_row(
                &format!("{EXTRACTION_OUTCOME_COLUMNS} WHERE id = ?1"),
                [id],
                extraction_outcome_from_row,
            )
            .optional()?;
        Ok(row)
    }

    /// The recorded outcome for one extraction SLOT — the emitting ones too, so
    /// "what did the pipeline conclude about this exact period?" is answerable
    /// without knowing the derived row id.
    pub fn get_extraction_outcome_for_slot(
        &self,
        company_id: &str,
        report_document_id: &str,
        fiscal_year: i64,
        period_type: &str,
        period_end: &str,
    ) -> StorageResult<Option<ExtractionOutcome>> {
        self.get_extraction_outcome(&extraction_outcome_id(
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
        ))
    }

    /// A company's **non-emitting** extraction outcomes — the flagged/failed
    /// review surface. Clean runs are deliberately excluded: an accepted period
    /// needs no review, and including it would drown the signal.
    pub fn list_flagged_extraction_outcomes(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<ExtractionOutcome>> {
        let connection = self.db.checkout()?;
        let mut statement = connection.prepare(&format!(
            "{EXTRACTION_OUTCOME_COLUMNS}
             WHERE company_id = ?1 AND acceptance IN ('flagged', 'empty')
             ORDER BY last_attempted_at DESC, period_end DESC, id"
        ))?;
        let rows = statement.query_map([company_id], extraction_outcome_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

/// Shared column list + FROM for every outcome read, so the projection and
/// [`extraction_outcome_from_row`]'s indexes can never drift apart.
const EXTRACTION_OUTCOME_COLUMNS: &str = "
    SELECT id, company_id, report_document_id, fiscal_year, period_type,
           period_end, tier, acceptance, reason_code, detail_json, drift_json,
           structure_changed, fact_count, attempt_count, first_attempted_at,
           last_attempted_at
    FROM fundamentals_extraction_outcomes";

fn extraction_outcome_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExtractionOutcome> {
    Ok(ExtractionOutcome {
        id: row.get(0)?,
        company_id: row.get(1)?,
        report_document_id: row.get(2)?,
        fiscal_year: row.get(3)?,
        period_type: row.get(4)?,
        period_end: row.get(5)?,
        tier: row.get(6)?,
        acceptance: row.get(7)?,
        reason_code: row.get(8)?,
        detail_json: row.get(9)?,
        drift_json: row.get(10)?,
        structure_changed: row.get::<_, i64>(11)? != 0,
        fact_count: row.get(12)?,
        attempt_count: row.get(13)?,
        first_attempted_at: row.get(14)?,
        last_attempted_at: row.get(15)?,
    })
}

fn fact_provenance_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FactProvenance> {
    Ok(FactProvenance {
        fact_id: row.get(0)?,
        source_tier: row.get(1)?,
        validation_status: row.get(2)?,
        drift_json: row.get(3)?,
        citation: row.get(4)?,
    })
}
