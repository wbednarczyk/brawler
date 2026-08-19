//! Parsed management-holdings storage + the founder/insider stamping join
//! (ADR 0083 Decision 6, plan v0.57 T5, card `9730f5f`).
//!
//! Mirrors the ownership report-extraction seam
//! ([`super::ownership::append_snapshot`] + `ownership_extraction_residual`): the
//! deterministic [`fundamentals::management_holdings`](crate::fundamentals::management_holdings)
//! parser turns a report's holdings section into per-person rows, each upserted in
//! place by a **deterministic id** from `(report_document_id, person_normalized)`
//! so a re-parse never duplicates. A document whose section the deterministic tier
//! cannot read parks once in `management_holdings_residual` (never guessed).
//!
//! **Founder stamping** ([`stamp_founder_insiders`]) is the reusable join over
//! **both** insider substrates (management holdings + MAR art. 19 transactions):
//! a still-`NULL` `ownership_stakes.holder_type` whose canonical holder identity
//! matches a management/insider person — or the `indirect_via` vehicle a founder
//! holds through — is stamped `founder_insider`. Exact canonical identity only
//! (never a shared surname: Adam vs Michał Kiciński stay distinct); manual and
//! dictionary labels are preserved (NULL-only, mirroring
//! [`super::ownership::classify_unclassified_for_company`]).

use std::collections::BTreeMap;

use crate::fundamentals::management_holdings::MgmtRole;
use crate::fundamentals::ownership::classify::canonical_holder_identity;

use super::*;

/// The reserved `person_normalized` sentinel for a document-level zero-aggregate
/// row (a prose "nie posiadają akcji" statement with no named persons).
fn zero_sentinel(role: Option<MgmtRole>) -> (&'static str, &'static str) {
    // (raw display, normalized key)
    match role {
        Some(MgmtRole::Management) => ("(zarząd — brak akcji)", "__ZERO_MANAGEMENT__"),
        Some(MgmtRole::Supervisory) => ("(rada nadzorcza — brak akcji)", "__ZERO_SUPERVISORY__"),
        None => (
            "(osoby zarządzające i nadzorujące — brak akcji)",
            "__ZERO_AGGREGATE__",
        ),
    }
}

/// A stored management-holdings row (headless in T5 — the read-model exposure is
/// the skin-in-the-game badge join, not a standalone DTO).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementHoldingRow {
    pub id: String,
    pub company_id: String,
    pub report_document_id: String,
    pub person_name_raw: String,
    pub person_normalized: String,
    pub role: Option<String>,
    pub shares: Option<String>,
    pub indirect_via_raw: Option<String>,
    pub indirect_via_normalized: Option<String>,
    pub prior_shares: Option<String>,
    pub prior_as_of: Option<String>,
    pub as_of: String,
    pub is_zero_aggregate: bool,
}

/// A new by-person holdings row to upsert. `person_normalized` /
/// `indirect_via_normalized` are derived at the boundary (uppercase
/// `normalize_holder_name`), so the caller passes only raw text.
#[derive(Debug, Clone)]
pub struct NewManagementHolding {
    pub company_id: String,
    pub report_document_id: String,
    pub person_name_raw: String,
    pub role: Option<String>,
    pub shares: Option<String>,
    pub indirect_via_raw: Option<String>,
    pub prior_shares: Option<String>,
    pub prior_as_of: Option<String>,
    pub as_of: String,
}

/// The residual for a document whose holdings section the deterministic parser
/// could not read (mirrors `OwnershipExtractionResidual`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementHoldingsResidual {
    pub report_document_id: String,
    pub company_id: String,
    /// `section_missing | table_unparsable | glyph_encoded`.
    pub parse_state: String,
    pub detected_as_of: Option<String>,
    pub matched_heading: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A fetched periodic report lacking management-holdings coverage — neither a row
/// nor a residual. The extraction triggers enqueue exactly these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentNeedingManagementExtraction {
    pub company_id: String,
    pub report_document_id: String,
}

/// One skin-in-the-game corroboration for a holder identity: the natural person
/// behind the stake, and the vehicle when the founder holds indirectly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinInTheGameMatch {
    pub person: String,
    pub via: Option<String>,
}

const MH_COLUMNS: &str = "id, company_id, report_document_id, person_name_raw, person_normalized, \
     role, shares, indirect_via_raw, indirect_via_normalized, prior_shares, prior_as_of, as_of, \
     is_zero_aggregate";

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<ManagementHoldingRow> {
    Ok(ManagementHoldingRow {
        id: row.get(0)?,
        company_id: row.get(1)?,
        report_document_id: row.get(2)?,
        person_name_raw: row.get(3)?,
        person_normalized: row.get(4)?,
        role: row.get(5)?,
        shares: row.get(6)?,
        indirect_via_raw: row.get(7)?,
        indirect_via_normalized: row.get(8)?,
        prior_shares: row.get(9)?,
        prior_as_of: row.get(10)?,
        as_of: row.get(11)?,
        is_zero_aggregate: row.get::<_, i64>(12)? != 0,
    })
}

/// Deterministic id: one row per `(report_document_id, person_normalized)`.
fn holding_id(report_document_id: &str, person_normalized: &str) -> String {
    format!(
        "mgmthld_{}_{}",
        slug_part(report_document_id),
        slug_part(person_normalized)
    )
}

fn normalize_person(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

// One `INSERT … ON CONFLICT` per management-holdings column set; the two callers
// (by-person row, zero-aggregate sentinel) differ only in their field values.
#[allow(clippy::too_many_arguments)]
fn insert_row(
    connection: &Connection,
    company_id: &str,
    report_document_id: &str,
    person_name_raw: &str,
    person_normalized: &str,
    role: Option<&str>,
    shares: Option<&str>,
    indirect_via_raw: Option<&str>,
    indirect_via_normalized: Option<&str>,
    prior_shares: Option<&str>,
    prior_as_of: Option<&str>,
    as_of: &str,
    is_zero_aggregate: bool,
) -> StorageResult<ManagementHoldingRow> {
    let id = holding_id(report_document_id, person_normalized);
    connection.execute(
        "
        INSERT INTO management_holdings (
            id, company_id, report_document_id, person_name_raw, person_normalized,
            role, shares, indirect_via_raw, indirect_via_normalized, prior_shares,
            prior_as_of, as_of, is_zero_aggregate
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(id) DO UPDATE SET
            person_name_raw = excluded.person_name_raw,
            role = excluded.role,
            shares = excluded.shares,
            indirect_via_raw = excluded.indirect_via_raw,
            indirect_via_normalized = excluded.indirect_via_normalized,
            prior_shares = excluded.prior_shares,
            prior_as_of = excluded.prior_as_of,
            as_of = excluded.as_of,
            is_zero_aggregate = excluded.is_zero_aggregate,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            id,
            company_id,
            report_document_id,
            person_name_raw,
            person_normalized,
            role,
            shares,
            indirect_via_raw,
            indirect_via_normalized,
            prior_shares,
            prior_as_of,
            as_of,
            i64::from(is_zero_aggregate),
        ],
    )?;
    connection
        .query_row(
            &format!("SELECT {MH_COLUMNS} FROM management_holdings WHERE id = ?1"),
            [&id],
            row_from,
        )
        .map_err(StorageError::from)
}

/// Upsert one parsed by-person holding row (idempotent by deterministic id).
pub(super) fn upsert_holding(
    connection: &Connection,
    holding: &NewManagementHolding,
) -> StorageResult<ManagementHoldingRow> {
    let person_normalized = normalize_person(&holding.person_name_raw);
    let indirect_via_normalized = holding
        .indirect_via_raw
        .as_deref()
        .map(normalize_person)
        .filter(|s| !s.is_empty());
    insert_row(
        connection,
        &holding.company_id,
        &holding.report_document_id,
        &holding.person_name_raw,
        &person_normalized,
        holding.role.as_deref(),
        holding.shares.as_deref(),
        holding.indirect_via_raw.as_deref(),
        indirect_via_normalized.as_deref(),
        holding.prior_shares.as_deref(),
        holding.prior_as_of.as_deref(),
        &holding.as_of,
        false,
    )
}

/// Upsert one document-level zero-aggregate marker (a prose "nie posiadają akcji"
/// statement). One sentinel row per organ (or the whole board when `role` is
/// `None`) — a zero holding is an explicit picture, kept, not dropped.
pub(super) fn upsert_zero_aggregate(
    connection: &Connection,
    company_id: &str,
    report_document_id: &str,
    role: Option<MgmtRole>,
    as_of: &str,
) -> StorageResult<ManagementHoldingRow> {
    let (raw, normalized) = zero_sentinel(role);
    insert_row(
        connection,
        company_id,
        report_document_id,
        raw,
        normalized,
        role.map(|r| r.as_str()),
        Some("0"),
        None,
        None,
        None,
        None,
        as_of,
        true,
    )
}

/// All by-person holdings for a company (excludes zero-aggregate sentinels),
/// newest disclosure first.
pub(super) fn list_by_company(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<ManagementHoldingRow>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {MH_COLUMNS} FROM management_holdings \
         WHERE company_id = ?1 AND is_zero_aggregate = 0 \
         ORDER BY as_of DESC, person_normalized ASC"
    ))?;
    let rows = statement.query_map([company_id], row_from)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

// ---------------------------------------------------------------------------
// Residual
// ---------------------------------------------------------------------------

const RESIDUAL_COLUMNS: &str =
    "report_document_id, company_id, parse_state, detected_as_of, matched_heading, \
     created_at, updated_at";

fn residual_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<ManagementHoldingsResidual> {
    Ok(ManagementHoldingsResidual {
        report_document_id: row.get(0)?,
        company_id: row.get(1)?,
        parse_state: row.get(2)?,
        detected_as_of: row.get(3)?,
        matched_heading: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// Upsert the residual for one document (idempotent per `report_document_id`).
pub(super) fn record_residual(
    connection: &Connection,
    residual: &ManagementHoldingsResidual,
) -> StorageResult<ManagementHoldingsResidual> {
    connection.execute(
        "
        INSERT INTO management_holdings_residual (
            report_document_id, company_id, parse_state, detected_as_of, matched_heading
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(report_document_id) DO UPDATE SET
            company_id = excluded.company_id,
            parse_state = excluded.parse_state,
            detected_as_of = excluded.detected_as_of,
            matched_heading = excluded.matched_heading,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            residual.report_document_id,
            residual.company_id,
            residual.parse_state,
            residual.detected_as_of,
            residual.matched_heading,
        ],
    )?;
    connection
        .query_row(
            &format!(
                "SELECT {RESIDUAL_COLUMNS} FROM management_holdings_residual \
                 WHERE report_document_id = ?1"
            ),
            [&residual.report_document_id],
            residual_from,
        )
        .map_err(StorageError::from)
}

/// Remove a document's residual (self-heal on a later successful parse).
pub(super) fn clear_residual(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<usize> {
    let affected = connection.execute(
        "DELETE FROM management_holdings_residual WHERE report_document_id = ?1",
        [report_document_id],
    )?;
    Ok(affected)
}

pub(super) fn list_residuals(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<ManagementHoldingsResidual>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {RESIDUAL_COLUMNS} FROM management_holdings_residual \
         WHERE company_id = ?1 ORDER BY created_at DESC, report_document_id DESC"
    ))?;
    let rows = statement.query_map([company_id], residual_from)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Fetched documents that carry a management-holdings table but lack coverage —
/// neither a row (of any kind, incl. a zero-aggregate) nor a residual. The table
/// lives in a periodic financial statement (combined QSr bundles) OR in the
/// management activity report (SzD), which is `doc_kind='other'` (F-A3): KRU
/// discloses management holdings ONLY in its SzD, so a periodic-only selection
/// silently produced zero rows. Candidate `other` documents are confirmed by the
/// pure [`is_management_report`] predicate in Rust (SQL can't express the marker
/// logic without drifting from the classifier).
pub(super) fn documents_needing_management_extraction(
    connection: &Connection,
    company_id: Option<&str>,
) -> StorageResult<Vec<DocumentNeedingManagementExtraction>> {
    let mut sql = String::from(
        "
        SELECT rd.company_id, rd.id, rd.doc_kind, rd.title, rd.url
        FROM report_documents rd
        WHERE rd.fetch_status = 'fetched'
          AND rd.local_path IS NOT NULL
          AND rd.doc_kind IN ('periodic_ssf', 'periodic_jsf', 'other')
          AND NOT EXISTS (
            SELECT 1 FROM management_holdings mh WHERE mh.report_document_id = rd.id
          )
          AND NOT EXISTS (
            SELECT 1 FROM management_holdings_residual r WHERE r.report_document_id = rd.id
          )
        ",
    );
    if company_id.is_some() {
        sql.push_str(" AND rd.company_id = ?1");
    }
    sql.push_str(" ORDER BY rd.created_at DESC");
    let mut statement = connection.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
        ))
    };
    let rows = match company_id {
        Some(id) => statement.query_map([id], map)?,
        None => statement.query_map([], map)?,
    };
    let mut out = Vec::new();
    for row in rows {
        let (company_id, report_document_id, doc_kind, title, url) =
            row.map_err(StorageError::from)?;
        let is_periodic = matches!(
            doc_kind.as_deref(),
            Some("periodic_ssf") | Some("periodic_jsf")
        );
        let is_mgmt_report = crate::fundamentals::extraction::classify::is_management_report(
            title.as_deref().unwrap_or(""),
            &url,
        );
        if is_periodic || is_mgmt_report {
            out.push(DocumentNeedingManagementExtraction {
                company_id,
                report_document_id,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Founder / insider stamping join (reusable over both substrates)
// ---------------------------------------------------------------------------

/// One evidence entry keyed by the canonical holder identity a stake could carry:
/// a natural person (direct holding), or a vehicle (a founder's indirect holding).
struct Evidence {
    identity: String,
    person: String,
    via: Option<String>,
}

/// Gather every skin-in-the-game evidence identity for a company from BOTH the
/// management-holdings and the insider-transaction substrates. A management row
/// with an `indirect_via` vehicle yields two identities (the person AND the
/// vehicle); an insider row yields the person (and its anchoring PDMR).
fn gather_evidence(connection: &Connection, company_id: &str) -> StorageResult<Vec<Evidence>> {
    let mut evidence: Vec<Evidence> = Vec::new();

    // Management holdings (skip zero-aggregate sentinels — no real person).
    for row in list_by_company(connection, company_id)? {
        let identity = canonical_holder_identity(&row.person_name_raw);
        if !identity.is_empty() {
            evidence.push(Evidence {
                identity,
                person: row.person_name_raw.clone(),
                via: None,
            });
        }
        if let Some(via_raw) = row.indirect_via_raw.as_deref() {
            let via_identity = canonical_holder_identity(via_raw);
            if !via_identity.is_empty() {
                evidence.push(Evidence {
                    identity: via_identity,
                    person: row.person_name_raw.clone(),
                    via: Some(via_raw.to_owned()),
                });
            }
        }
    }

    // Insider transactions (T4 substrate): person + anchoring PDMR.
    let mut statement = connection.prepare(
        "SELECT person_name_raw, related_pdmr_raw FROM insider_transactions WHERE company_id = ?1",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    for row in rows {
        let (person_raw, pdmr_raw) = row.map_err(StorageError::from)?;
        let identity = canonical_holder_identity(&person_raw);
        if !identity.is_empty() {
            evidence.push(Evidence {
                identity,
                person: person_raw.clone(),
                via: None,
            });
        }
        if let Some(pdmr) = pdmr_raw {
            let pdmr_identity = canonical_holder_identity(&pdmr);
            if !pdmr_identity.is_empty() {
                evidence.push(Evidence {
                    identity: pdmr_identity,
                    person: pdmr,
                    via: None,
                });
            }
        }
    }
    Ok(evidence)
}

/// Skin-in-the-game corroboration keyed by canonical holder identity — the
/// read-model join for the badge. First evidence per identity wins (management
/// before insider, in document order).
pub(super) fn skin_in_the_game(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<BTreeMap<String, SkinInTheGameMatch>> {
    let mut map: BTreeMap<String, SkinInTheGameMatch> = BTreeMap::new();
    for ev in gather_evidence(connection, company_id)? {
        map.entry(ev.identity).or_insert(SkinInTheGameMatch {
            person: ev.person,
            via: ev.via,
        });
    }
    Ok(map)
}

/// Stamp `founder_insider` on this company's still-`NULL` `ownership_stakes` rows
/// whose canonical holder identity matches a management/insider person or vehicle.
/// Exact identity only (a shared surname never matches); manual/dictionary labels
/// are preserved (NULL-only). Returns the number of stake rows stamped.
pub(super) fn stamp_founder_insiders(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<usize> {
    let evidence = gather_evidence(connection, company_id)?;
    if evidence.is_empty() {
        return Ok(0);
    }
    let identities: std::collections::HashSet<&str> =
        evidence.iter().map(|e| e.identity.as_str()).collect();

    // The distinct still-unclassified holders (NULL-only, mirrors classify).
    let mut statement = connection.prepare(
        "SELECT DISTINCT holder_name_normalized, holder_name_raw FROM ownership_stakes
         WHERE company_id = ?1 AND holder_type IS NULL",
    )?;
    let holders = statement
        .query_map([company_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)?;

    let mut stamped = 0usize;
    for (holder_normalized, holder_raw) in holders {
        let identity = canonical_holder_identity(&holder_raw);
        if identity.is_empty() || !identities.contains(identity.as_str()) {
            continue;
        }
        stamped += connection.execute(
            "UPDATE ownership_stakes SET holder_type = 'founder_insider'
             WHERE company_id = ?1 AND holder_name_normalized = ?2 AND holder_type IS NULL",
            params![company_id, holder_normalized],
        )?;
    }
    Ok(stamped)
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Parsed-management-holdings domain store (ADR 0083). `AppState::management_holdings()`.
#[derive(Clone)]
pub struct ManagementHoldingsStore {
    db: Database,
}

impl ManagementHoldingsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn upsert_holding(
        &self,
        holding: NewManagementHolding,
    ) -> StorageResult<ManagementHoldingRow> {
        let connection = self.db.checkout()?;
        upsert_holding(&connection, &holding)
    }

    /// Upsert a whole document's by-person holding rows in ONE `IMMEDIATE`
    /// transaction (#404 H2): a per-row `upsert_holding` loop is one pool
    /// checkout — and one fsync under `synchronous=FULL` — per row, so a
    /// document with hundreds of insiders amplified into hundreds of fsyncs.
    /// One tx per document means a constraint failure rolls back the whole
    /// document's rows (deliberate — atomic persist, consistent with the
    /// argument at `companies.rs:200`).
    pub fn upsert_holdings(
        &self,
        holdings: &[NewManagementHolding],
    ) -> StorageResult<Vec<ManagementHoldingRow>> {
        if holdings.is_empty() {
            return Ok(Vec::new());
        }
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut rows = Vec::with_capacity(holdings.len());
        for holding in holdings {
            rows.push(upsert_holding(&tx, holding)?);
        }
        tx.commit()?;
        Ok(rows)
    }

    pub fn upsert_zero_aggregate(
        &self,
        company_id: &str,
        report_document_id: &str,
        role: Option<MgmtRole>,
        as_of: &str,
    ) -> StorageResult<ManagementHoldingRow> {
        let connection = self.db.checkout()?;
        upsert_zero_aggregate(&connection, company_id, report_document_id, role, as_of)
    }

    /// Upsert a document's zero-holding-aggregate sentinels (one per organ, at
    /// most two: management + supervisory) in ONE `IMMEDIATE` transaction —
    /// same rationale as [`Self::upsert_holdings`], sized for the (small,
    /// bounded) `zero_organs` set rather than reusing `upsert_holdings`, whose
    /// `NewManagementHolding` shape would re-derive `person_normalized` from a
    /// raw string instead of the reserved `__ZERO_*__` sentinel key.
    pub fn upsert_zero_aggregates(
        &self,
        company_id: &str,
        report_document_id: &str,
        entries: &[(Option<MgmtRole>, String)],
    ) -> StorageResult<Vec<ManagementHoldingRow>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut rows = Vec::with_capacity(entries.len());
        for (role, as_of) in entries {
            rows.push(upsert_zero_aggregate(
                &tx,
                company_id,
                report_document_id,
                *role,
                as_of,
            )?);
        }
        tx.commit()?;
        Ok(rows)
    }

    pub fn list_by_company(&self, company_id: &str) -> StorageResult<Vec<ManagementHoldingRow>> {
        let connection = self.db.checkout()?;
        list_by_company(&connection, company_id)
    }

    pub fn record_residual(
        &self,
        residual: ManagementHoldingsResidual,
    ) -> StorageResult<ManagementHoldingsResidual> {
        let connection = self.db.checkout()?;
        record_residual(&connection, &residual)
    }

    pub fn clear_residual(&self, report_document_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        clear_residual(&connection, report_document_id)
    }

    pub fn list_residuals(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<ManagementHoldingsResidual>> {
        let connection = self.db.checkout()?;
        list_residuals(&connection, company_id)
    }

    pub fn documents_needing_management_extraction(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<DocumentNeedingManagementExtraction>> {
        let connection = self.db.checkout()?;
        documents_needing_management_extraction(&connection, company_id)
    }

    /// Skin-in-the-game corroboration map (read-model badge join).
    pub fn skin_in_the_game(
        &self,
        company_id: &str,
    ) -> StorageResult<BTreeMap<String, SkinInTheGameMatch>> {
        let connection = self.db.checkout()?;
        skin_in_the_game(&connection, company_id)
    }

    /// Stamp `founder_insider` from the management + insider substrates (NULL-only).
    pub fn stamp_founder_insiders(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        stamp_founder_insiders(&connection, company_id)
    }
}

use super::database::Database;
