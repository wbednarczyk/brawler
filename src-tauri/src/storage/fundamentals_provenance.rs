//! Structured-first extraction provenance + per-company profiles (ADR 0061 S5).
//!
//! Two additive tables, one domain store ([`FundamentalsProvenanceStore`]):
//! - `financial_fact_provenance` — per fact, which tier produced it, its
//!   validation verdict, and any drift diff. Modeled separately from
//!   `financial_facts` (joined by id) so the central facts contract is untouched.
//! - `company_extraction_profile` — the confirmed, versioned PDF layout the
//!   deterministic parser learns per company.
//!
//! Reach the store via `AppState::fundamentals_provenance()`.

use super::database::Database;
use super::*;

use crate::fundamentals::extraction::ocr::OcrExtractionProfile;
use crate::fundamentals::extraction::profile::ExtractionProfile;

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
    /// esef | structured_xhtml | pdf | html_aggregator | ai_text
    pub source_tier: String,
    /// passed | witness_confirmed | unreviewed | flagged | none
    pub validation_status: String,
    /// Serialized `DriftReport` JSON when a drift accompanied the fact.
    pub drift_json: Option<String>,
    /// The source concept/label this value was read from (primary citation).
    pub citation: Option<String>,
}

/// Input for recording provenance for one produced fact.
pub struct NewFactProvenance<'a> {
    pub fact_id: &'a str,
    pub source_tier: &'a str,
    pub validation_status: &'a str,
    pub drift_json: Option<&'a str>,
    pub citation: Option<&'a str>,
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

    /// Reads a company's confirmed extraction profile, if bootstrapped.
    pub fn get_profile(&self, company_id: &str) -> StorageResult<Option<ExtractionProfile>> {
        let connection = self.db.checkout()?;
        let json: Option<String> = connection
            .query_row(
                "SELECT profile_json FROM company_extraction_profile WHERE company_id = ?1",
                [company_id],
                |row| row.get(0),
            )
            .optional()?;
        match json {
            Some(json) => Ok(serde_json::from_str(&json).ok()),
            None => Ok(None),
        }
    }

    /// Upserts a company's extraction profile (bootstrap or learned merge).
    pub fn upsert_profile(&self, profile: &ExtractionProfile) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        let profile_json =
            serde_json::to_string(profile).map_err(|e| StorageError::InvalidFinancialsValue {
                key: "profile_json",
                value: e.to_string(),
            })?;
        let unit_scale = serde_json::to_string(&profile.unit_scale)
            .unwrap_or_else(|_| "\"Thousands\"".to_owned());
        connection.execute(
            "
            INSERT INTO company_extraction_profile
                (company_id, template_hash, unit_scale, profile_json, version)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(company_id) DO UPDATE SET
                template_hash = excluded.template_hash,
                unit_scale = excluded.unit_scale,
                profile_json = excluded.profile_json,
                version = excluded.version,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                profile.company_id,
                profile.template_hash,
                unit_scale.trim_matches('"'),
                profile_json,
                profile.version,
            ],
        )?;
        Ok(())
    }

    /// Reads a company's confirmed OCR-markdown extraction profile, if
    /// bootstrapped (ADR 0077 §4). Absence is a safe default: tier-4 cannot yet
    /// parse deterministically for that company.
    pub fn get_ocr_profile(&self, company_id: &str) -> StorageResult<Option<OcrExtractionProfile>> {
        let connection = self.db.checkout()?;
        let json: Option<String> = connection
            .query_row(
                "SELECT profile_json FROM company_ocr_extraction_profile WHERE company_id = ?1",
                [company_id],
                |row| row.get(0),
            )
            .optional()?;
        match json {
            Some(json) => Ok(serde_json::from_str(&json).ok()),
            None => Ok(None),
        }
    }

    /// Upserts a company's OCR-markdown extraction profile (bootstrap or
    /// confirmed re-bootstrap).
    pub fn upsert_ocr_profile(&self, profile: &OcrExtractionProfile) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        let profile_json =
            serde_json::to_string(profile).map_err(|e| StorageError::InvalidFinancialsValue {
                key: "ocr_profile_json",
                value: e.to_string(),
            })?;
        let scale =
            serde_json::to_string(&profile.scale).unwrap_or_else(|_| "\"Thousands\"".to_owned());
        connection.execute(
            "
            INSERT INTO company_ocr_extraction_profile
                (company_id, template_hash, scale, profile_json, version)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(company_id) DO UPDATE SET
                template_hash = excluded.template_hash,
                scale = excluded.scale,
                profile_json = excluded.profile_json,
                version = excluded.version,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                profile.company_id,
                profile.template_hash,
                scale.trim_matches('"'),
                profile_json,
                profile.version,
            ],
        )?;
        Ok(())
    }
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
