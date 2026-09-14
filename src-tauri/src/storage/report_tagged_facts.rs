//! Storage for the Layer 1 raw tagged-fact capture (`report_tagged_facts` +
//! `report_tagged_fact_roles` + `report_tagged_fact_extractions`, ADR 0100
//! decisions 1, 8, 9; epic #398).
//!
//! Every row is derived from a report document's stored bytes by the (future)
//! ESEF/iXBRL tagged-fact extractor — never a source of truth, same doctrine
//! as `report_sections.rs` (ADR 0052). One extraction row per document
//! records the outcome plus the ship-gate counters (decision 9:
//! `encountered_count`/`stored_count`/`dimensional_count`); fact rows are
//! written 1:1 from the instance, including occurrences whose normalization
//! failed (`parse_status != 'ok'`, nullable `value_numeric` — never a silent
//! drop). Freshness is `(source_content_hash, extractor_version)` (decision
//! 8), never `extractor_version` alone.
//!
//! This module is storage only: nothing calls `replace_tagged_facts` yet —
//! the parser/pipeline wiring lands in a later slice.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::database::Database;
use super::{StorageError, StorageResult};

/// One fact's participation in a presentation role — a concept can appear in
/// several (decision 3), so this is a relation, never a scalar column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTaggedFactRole {
    pub role_uri: String,
    pub role_kind: String,
}

/// One tagged fact occurrence to write, as the (future) extractor produces it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewTaggedFact {
    pub package_entry_path: String,
    pub fact_identity: String,
    pub identity_kind: String,
    pub concept_namespace_uri: String,
    pub concept_local_name: String,
    pub context_ref: String,
    pub period_type: String,
    pub period_start: Option<String>,
    pub period_end: String,
    pub unit_ref: Option<String>,
    pub unit_measure: Option<String>,
    pub value_raw: String,
    pub value_numeric: Option<String>,
    pub scale: Option<i64>,
    pub sign: Option<String>,
    pub decimals: Option<String>,
    pub is_dimensional: bool,
    pub dimensions_json: Option<String>,
    pub parse_status: String,
    pub parse_error: Option<String>,
    pub roles: Vec<NewTaggedFactRole>,
}

/// One document's tagged-fact extraction generation to write atomically.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaggedFactExtraction {
    pub source_content_hash: Option<String>,
    pub extractor_version: i64,
    pub state: String,
    pub encountered_count: i64,
    pub stored_count: i64,
    pub dimensional_count: i64,
    /// Count of dimensionless, valued facts that used the no-linkbase
    /// fallback (ADR 0100 decision 3 regression fix, epic #398) — `0` for a
    /// document that carried presentation-linkbase evidence, the number of
    /// affected facts otherwise. The visible, never-silent record of the
    /// fallback's use.
    pub no_linkbase_fallback_count: i64,
    pub facts: Vec<NewTaggedFact>,
}

/// A stored role row, read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTaggedFactRole {
    pub role_uri: String,
    pub role_kind: String,
}

/// A stored fact row, read back, with its roles joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTaggedFact {
    pub id: String,
    pub report_document_id: String,
    pub company_id: String,
    pub package_entry_path: String,
    pub fact_identity: String,
    pub identity_kind: String,
    pub concept_namespace_uri: String,
    pub concept_local_name: String,
    pub context_ref: String,
    pub period_type: String,
    pub period_start: Option<String>,
    pub period_end: String,
    pub unit_ref: Option<String>,
    pub unit_measure: Option<String>,
    pub value_raw: String,
    pub value_numeric: Option<String>,
    pub scale: Option<i64>,
    pub sign: Option<String>,
    pub decimals: Option<String>,
    pub is_dimensional: bool,
    pub dimensions_json: Option<String>,
    pub parse_status: String,
    pub parse_error: Option<String>,
    pub roles: Vec<StoredTaggedFactRole>,
}

/// The persisted extraction record for one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTaggedFactExtraction {
    pub report_document_id: String,
    pub source_content_hash: Option<String>,
    pub extractor_version: i64,
    pub state: String,
    pub encountered_count: i64,
    pub stored_count: i64,
    pub dimensional_count: i64,
    pub no_linkbase_fallback_count: i64,
}

/// Collision-safe, non-deterministic id (the `generate_observation_id` idiom,
/// `kpi_ingest_staging.rs`): `rtf_` + 32 hex chars of sha256 over the identity
/// plus a per-batch ordinal and a nanosecond time component. Not deterministic
/// on the fact's business key alone — re-extraction always mints fresh ids;
/// `UNIQUE (report_document_id, package_entry_path, fact_identity)` is the
/// real business-key invariant, not this id.
fn generate_tagged_fact_id(
    report_document_id: &str,
    package_entry_path: &str,
    fact_identity: &str,
    ordinal: usize,
) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!(
        "rtf:{report_document_id}\u{1f}{package_entry_path}\u{1f}{fact_identity}\u{1f}{ordinal}\u{1f}{now_nanos}"
    );
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("rtf_{hex}")
}

/// Replace one document's tagged-fact extraction generation atomically: delete
/// its existing rows + extraction record, insert the new generation + record,
/// all in ONE transaction — a failed rebuild never leaves a half-generation
/// (ADR 0100 decision 8, the `report_sections.rs` replace idiom). Idempotent:
/// re-running with the same input converges on the same rows (fresh ids, same
/// business-key set).
pub(crate) fn replace_tagged_facts(
    connection: &mut Connection,
    report_document_id: &str,
    company_id: &str,
    extraction: &TaggedFactExtraction,
) -> StorageResult<()> {
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    tx.execute(
        "DELETE FROM report_tagged_facts WHERE report_document_id = ?1",
        params![report_document_id],
    )?;

    tx.execute(
        "
        INSERT INTO report_tagged_fact_extractions
            (report_document_id, source_content_hash, extractor_version, state,
             encountered_count, stored_count, dimensional_count, no_linkbase_fallback_count)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT (report_document_id) DO UPDATE SET
            source_content_hash = excluded.source_content_hash,
            extractor_version = excluded.extractor_version,
            state = excluded.state,
            encountered_count = excluded.encountered_count,
            stored_count = excluded.stored_count,
            dimensional_count = excluded.dimensional_count,
            no_linkbase_fallback_count = excluded.no_linkbase_fallback_count,
            extracted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            report_document_id,
            extraction.source_content_hash,
            extraction.extractor_version,
            extraction.state,
            extraction.encountered_count,
            extraction.stored_count,
            extraction.dimensional_count,
            extraction.no_linkbase_fallback_count,
        ],
    )?;

    for (ordinal, fact) in extraction.facts.iter().enumerate() {
        let fact_id = generate_tagged_fact_id(
            report_document_id,
            &fact.package_entry_path,
            &fact.fact_identity,
            ordinal,
        );
        tx.execute(
            "
            INSERT INTO report_tagged_facts
                (id, report_document_id, company_id, package_entry_path, fact_identity,
                 identity_kind, concept_namespace_uri, concept_local_name, context_ref,
                 period_type, period_start, period_end, unit_ref, unit_measure,
                 value_raw, value_numeric, scale, sign, decimals,
                 is_dimensional, dimensions_json, parse_status, parse_error)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
            ",
            params![
                fact_id,
                report_document_id,
                company_id,
                fact.package_entry_path,
                fact.fact_identity,
                fact.identity_kind,
                fact.concept_namespace_uri,
                fact.concept_local_name,
                fact.context_ref,
                fact.period_type,
                fact.period_start,
                fact.period_end,
                fact.unit_ref,
                fact.unit_measure,
                fact.value_raw,
                fact.value_numeric,
                fact.scale,
                fact.sign,
                fact.decimals,
                fact.is_dimensional as i64,
                fact.dimensions_json,
                fact.parse_status,
                fact.parse_error,
            ],
        )?;

        for role in &fact.roles {
            tx.execute(
                "INSERT INTO report_tagged_fact_roles (fact_id, role_uri, role_kind) VALUES (?1, ?2, ?3)",
                params![fact_id, role.role_uri, role.role_kind],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

const SELECT_FACT_COLUMNS: &str = "
    id, report_document_id, company_id, package_entry_path, fact_identity,
    identity_kind, concept_namespace_uri, concept_local_name, context_ref,
    period_type, period_start, period_end, unit_ref, unit_measure,
    value_raw, value_numeric, scale, sign, decimals,
    is_dimensional, dimensions_json, parse_status, parse_error
";

fn row_to_fact(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredTaggedFact> {
    Ok(StoredTaggedFact {
        id: row.get(0)?,
        report_document_id: row.get(1)?,
        company_id: row.get(2)?,
        package_entry_path: row.get(3)?,
        fact_identity: row.get(4)?,
        identity_kind: row.get(5)?,
        concept_namespace_uri: row.get(6)?,
        concept_local_name: row.get(7)?,
        context_ref: row.get(8)?,
        period_type: row.get(9)?,
        period_start: row.get(10)?,
        period_end: row.get(11)?,
        unit_ref: row.get(12)?,
        unit_measure: row.get(13)?,
        value_raw: row.get(14)?,
        value_numeric: row.get(15)?,
        scale: row.get(16)?,
        sign: row.get(17)?,
        decimals: row.get(18)?,
        is_dimensional: row.get::<_, i64>(19)? != 0,
        dimensions_json: row.get(20)?,
        parse_status: row.get(21)?,
        parse_error: row.get(22)?,
        roles: Vec::new(),
    })
}

/// All stored facts for a document, with their roles joined. Order is
/// insertion order (rowid), stable for a given generation.
pub(crate) fn get_facts(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Vec<StoredTaggedFact>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {SELECT_FACT_COLUMNS} FROM report_tagged_facts
         WHERE report_document_id = ?1 ORDER BY rowid"
    ))?;
    let rows = statement.query_map(params![report_document_id], row_to_fact)?;
    let mut facts = Vec::new();
    for row in rows {
        facts.push(row?);
    }
    drop(statement);

    if facts.is_empty() {
        return Ok(facts);
    }

    let mut roles_by_fact: HashMap<String, Vec<StoredTaggedFactRole>> = HashMap::new();
    let mut role_statement = connection.prepare(
        "SELECT fact_id, role_uri, role_kind FROM report_tagged_fact_roles
         WHERE fact_id IN (SELECT id FROM report_tagged_facts WHERE report_document_id = ?1)",
    )?;
    let role_rows = role_statement.query_map(params![report_document_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            StoredTaggedFactRole {
                role_uri: row.get(1)?,
                role_kind: row.get(2)?,
            },
        ))
    })?;
    for row in role_rows {
        let (fact_id, role) = row?;
        roles_by_fact.entry(fact_id).or_default().push(role);
    }

    for fact in &mut facts {
        if let Some(roles) = roles_by_fact.remove(&fact.id) {
            fact.roles = roles;
        }
    }

    Ok(facts)
}

/// The stored extraction record for a document, if extracted.
pub(crate) fn get_extraction(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Option<StoredTaggedFactExtraction>> {
    connection
        .query_row(
            "SELECT report_document_id, source_content_hash, extractor_version, state,
                    encountered_count, stored_count, dimensional_count, no_linkbase_fallback_count
             FROM report_tagged_fact_extractions WHERE report_document_id = ?1",
            params![report_document_id],
            |row| {
                Ok(StoredTaggedFactExtraction {
                    report_document_id: row.get(0)?,
                    source_content_hash: row.get(1)?,
                    extractor_version: row.get(2)?,
                    state: row.get(3)?,
                    encountered_count: row.get(4)?,
                    stored_count: row.get(5)?,
                    dimensional_count: row.get(6)?,
                    no_linkbase_fallback_count: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

/// True when a current extraction exists for `(source_content_hash,
/// extractor_version)` (decision 8), so a rebuild can skip an unchanged
/// document.
pub(crate) fn extraction_is_current(
    connection: &Connection,
    report_document_id: &str,
    source_content_hash: &str,
    extractor_version: i64,
) -> StorageResult<bool> {
    let found: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM report_tagged_fact_extractions
             WHERE report_document_id = ?1 AND source_content_hash = ?2 AND extractor_version = ?3",
            params![report_document_id, source_content_hash, extractor_version],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// One taxonomy concept observed in Layer 1 with no crosswalk entry yet
/// (`fundamentals::extraction::ifrs_crosswalk`) — the harvest command's
/// output row, the input for the next seed migration (ADR 0100 decision 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestedConcept {
    pub concept_local_name: String,
    /// Distinct companies whose filings tag this concept.
    pub company_count: i64,
    /// Distinct `period_type` values observed for this concept, sorted.
    pub period_types: Vec<String>,
}

/// Ranks every Layer 1 concept absent from the curated crosswalk by distinct
/// issuer count, highest first — mirrors the corpus harvest that produced
/// `ifrs-crosswalk-candidates.txt` in the first place, but reads the
/// maintainer's own stored facts instead of a one-off sample. A concept
/// already in the crosswalk is intentionally omitted: this is a TODO list
/// for the next seed migration, not a full inventory.
fn harvest_uncrosswalked_concepts(connection: &Connection) -> StorageResult<Vec<HarvestedConcept>> {
    let crosswalked: std::collections::HashSet<&str> =
        crate::fundamentals::extraction::ifrs_crosswalk::entries()
            .iter()
            .map(|entry| entry.concept)
            .collect();

    // Grouped by (namespace standardness, local name), never local name
    // alone (sol review findings 1/11): an issuer extension reusing a
    // standard local name is a DIFFERENT concept — it must neither hide
    // behind the standard entry nor inflate the standard concept's
    // distinct-issuer count.
    let sql = format!(
        "SELECT concept_local_name,
                {pred} AS is_standard,
                COUNT(DISTINCT company_id),
                GROUP_CONCAT(DISTINCT period_type)
         FROM report_tagged_facts
         GROUP BY concept_local_name, is_standard
         ORDER BY COUNT(DISTINCT company_id) DESC, concept_local_name ASC",
        pred =
            crate::fundamentals::extraction::ifrs_crosswalk::STANDARD_IFRS_NAMESPACE_SQL_PREDICATE
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, bool>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut harvested = Vec::new();
    for row in rows {
        let (concept_local_name, is_standard, company_count, period_types_raw) = row?;
        if is_standard && crosswalked.contains(concept_local_name.as_str()) {
            continue;
        }
        let mut period_types: Vec<String> =
            period_types_raw.split(',').map(str::to_owned).collect();
        period_types.sort_unstable();
        period_types.dedup();
        harvested.push(HarvestedConcept {
            concept_local_name,
            company_count,
            period_types,
        });
    }
    Ok(harvested)
}

/// All stored facts for a COMPANY, with their roles joined — the `get_facts`
/// idiom, scoped by `company_id` instead of one document (epic #398: the
/// Coverage read model and the promotion list both need a company's whole
/// Layer 1, across every report it has ever tagged).
pub(crate) fn get_facts_for_company(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<StoredTaggedFact>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {SELECT_FACT_COLUMNS} FROM report_tagged_facts
         WHERE company_id = ?1 ORDER BY rowid"
    ))?;
    let rows = statement.query_map(params![company_id], row_to_fact)?;
    let mut facts = Vec::new();
    for row in rows {
        facts.push(row?);
    }
    drop(statement);

    if facts.is_empty() {
        return Ok(facts);
    }

    let mut roles_by_fact: HashMap<String, Vec<StoredTaggedFactRole>> = HashMap::new();
    let mut role_statement = connection.prepare(
        "SELECT fact_id, role_uri, role_kind FROM report_tagged_fact_roles
         WHERE fact_id IN (SELECT id FROM report_tagged_facts WHERE company_id = ?1)",
    )?;
    let role_rows = role_statement.query_map(params![company_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            StoredTaggedFactRole {
                role_uri: row.get(1)?,
                role_kind: row.get(2)?,
            },
        ))
    })?;
    for row in role_rows {
        let (fact_id, role) = row?;
        roles_by_fact.entry(fact_id).or_default().push(role);
    }

    for fact in &mut facts {
        if let Some(roles) = roles_by_fact.remove(&fact.id) {
            fact.roles = roles;
        }
    }

    Ok(facts)
}

/// [`StoredTaggedFact`] -> [`NewTaggedFact`], dropping the storage-assigned
/// identity fields (`id`, `report_document_id`, `company_id`) the read-model
/// helpers below carry separately — [`crate::fundamentals::extraction::esef::
/// projection::project_period`] is pure over the write shape, so a read model
/// that wants to re-run it converts back first.
fn to_new_fact(stored: &StoredTaggedFact) -> NewTaggedFact {
    NewTaggedFact {
        package_entry_path: stored.package_entry_path.clone(),
        fact_identity: stored.fact_identity.clone(),
        identity_kind: stored.identity_kind.clone(),
        concept_namespace_uri: stored.concept_namespace_uri.clone(),
        concept_local_name: stored.concept_local_name.clone(),
        context_ref: stored.context_ref.clone(),
        period_type: stored.period_type.clone(),
        period_start: stored.period_start.clone(),
        period_end: stored.period_end.clone(),
        unit_ref: stored.unit_ref.clone(),
        unit_measure: stored.unit_measure.clone(),
        value_raw: stored.value_raw.clone(),
        value_numeric: stored.value_numeric.clone(),
        scale: stored.scale,
        sign: stored.sign.clone(),
        decimals: stored.decimals.clone(),
        is_dimensional: stored.is_dimensional,
        dimensions_json: stored.dimensions_json.clone(),
        parse_status: stored.parse_status.clone(),
        parse_error: stored.parse_error.clone(),
        roles: stored
            .roles
            .iter()
            .map(|r| NewTaggedFactRole {
                role_uri: r.role_uri.clone(),
                role_kind: r.role_kind.clone(),
            })
            .collect(),
    }
}

/// The Coverage panel's compact read model (ADR 0100, epic #398 UI slice):
/// every number Layer 1 captured for a company, split into what the
/// deterministic projection rule keeps and the reasons the rest is not (yet)
/// in Fundamentals. `dimensional` and `unparsed` are direct row counts; the
/// projection buckets are computed by re-running
/// [`crate::fundamentals::extraction::esef::projection::project_period`] per
/// `(report_document, period_end)`. `projected` covers ONLY each filing's
/// own (latest) period end — the one Layer 2 writes; comparative periods'
/// candidates go to `comparative`, never inflated into `projected` (sol
/// review finding 8). `projected` still means "the deterministic rule would
/// keep it", not "it is live in `financial_facts` right now" — the
/// validation gate downstream can refuse a candidate (a stricter count is a
/// direct `financial_facts` query, unrelated to this read model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TaggedFactCoverageCounts {
    pub raw_stored: i64,
    /// Candidate slots at each document's OWN (latest) period end — the only
    /// periods Layer 2 actually writes (ADR 0100 decision 7). Comparative
    /// periods are counted separately below, never inflated into this number
    /// (sol review finding 8).
    pub projected: i64,
    /// Candidate slots at every EARLIER period end in a filing — captured in
    /// Layer 1, deliberately not written (decision 7).
    pub comparative: i64,
    pub dimensional: i64,
    pub note_level: i64,
    pub awaiting_name: i64,
    pub conflicting: i64,
    /// Dimensionless rows whose value never parsed (typed parse status in
    /// Layer 1, decision 9) — visible here so an unreadable number still has
    /// a stated reason it is not in Fundamentals.
    pub unparsed: i64,
    /// Raw occurrences folded into another candidate rather than becoming
    /// one themselves (sol round 2, finding 8): equal-value repeats beyond
    /// the first, and shorter duration windows superseded by the cumulative
    /// figure sharing their end date. Their reason is "already counted once".
    pub repeated: i64,
    /// Crosswalk-resolved, primary-statement occurrences dropped because they
    /// belong to the document's OTHER statement basis, or to a genuinely
    /// ambiguous instance (ADR 0100 decisions 1/2, #508) — kept as Layer 1
    /// evidence, never projected. Sum of `non_primary_basis_skipped` +
    /// `ambiguous_basis_skipped` over every period end in the document.
    pub other_basis: i64,
}

fn coverage_counts(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<TaggedFactCoverageCounts> {
    let facts = get_facts_for_company(connection, company_id)?;
    let raw_stored = facts.len() as i64;
    let dimensional = facts.iter().filter(|f| f.is_dimensional).count() as i64;

    let mut by_document: HashMap<&str, Vec<NewTaggedFact>> = HashMap::new();
    for fact in &facts {
        by_document
            .entry(fact.report_document_id.as_str())
            .or_default()
            .push(to_new_fact(fact));
    }

    let mut projected = 0i64;
    let mut comparative = 0i64;
    let mut note_level = 0i64;
    let mut awaiting_name = 0i64;
    let mut conflicting = 0i64;
    let mut unparsed = 0i64;
    let mut repeated = 0i64;
    let mut other_basis = 0i64;
    for (document_id, doc_facts) in &by_document {
        // The SAME linkbase evidence the job sees (astra r1 #1): the parsed
        // presentation-linkbase role map can be non-empty (a real `*_pre.xml`
        // exists) while covering none of THIS document's tagged concepts —
        // every stored fact then carries an empty role vector even though
        // `compute_layer1_generation` correctly saw `has_presentation_
        // linkbase = true`. Inferring the flag from "did any stored fact end
        // up with a role" would disagree with the job in exactly that case,
        // letting the no-linkbase fallback here crosswalk-resolve facts the
        // job's strict role filter rejects. `no_linkbase_fallback_count` is
        // the persisted, reliable per-document signal — `0` whenever a
        // linkbase existed, unconditionally, regardless of what it covered.
        let has_presentation_linkbase = get_extraction(connection, document_id)?
            .map(|extraction| extraction.no_linkbase_fallback_count == 0)
            .unwrap_or_else(|| doc_facts.iter().any(|f| !f.roles.is_empty()));
        // The SAME document-wide selection `run_pipeline` applies (ADR 0100
        // decision 2, #508) — never a second, independently-derived basis,
        // so the Coverage read model can never disagree with what actually
        // got projected.
        let basis = crate::fundamentals::extraction::esef::projection::select_primary_basis(
            doc_facts,
            has_presentation_linkbase,
        );
        // A dimensionless row whose value never parsed reaches no projection
        // bucket (step 1 skips it) — count it here so every raw number keeps
        // a stated reason (decision 9; sol review finding 8).
        unparsed += doc_facts
            .iter()
            .filter(|f| {
                // The projection's own step-1 predicate: a value the rule
                // cannot use, absent OR non-decimal.
                !f.is_dimensional
                    && f.value_numeric
                        .as_deref()
                        .and_then(|s| s.parse::<rust_decimal::Decimal>().ok())
                        .is_none()
            })
            .count() as i64;
        let mut period_ends: Vec<&str> = doc_facts.iter().map(|f| f.period_end.as_str()).collect();
        period_ends.sort_unstable();
        period_ends.dedup();
        // The filing's OWN period comes from the derived-period cache when
        // one exists (the document's DECLARED reporting date, sol round 2 —
        // a later-dated note instant must not demote the real reporting
        // period to a comparative); the latest tagged date is the fallback
        // for a document never derived.
        let declared: Option<String> = connection
            .query_row(
                "SELECT period_end FROM document_derived_periods
                 WHERE report_document_id = ?1 AND has_period = 1",
                params![*document_id],
                |row| row.get(0),
            )
            .optional()?;
        // A present derived period wins UNCONDITIONALLY (sol round 3): if
        // the declared reporting date is not among the tagged dates, the
        // honest outcome is zero projected and everything comparative —
        // never a silent switch to the lexicographically latest date, which
        // would let a subsequent-events note instant impersonate the
        // reporting period.
        let own_period_end: Option<&str> =
            declared.as_deref().or_else(|| period_ends.last().copied());
        for period_end in period_ends.iter().copied() {
            let result = crate::fundamentals::extraction::esef::projection::project_period(
                doc_facts,
                period_end,
                has_presentation_linkbase,
                basis,
            );
            if Some(period_end) == own_period_end {
                projected += result.facts.len() as i64;
            } else {
                comparative += result.facts.len() as i64;
            }
            other_basis +=
                result.non_primary_basis_skipped as i64 + result.ambiguous_basis_skipped as i64;
            // Fact-level counts (sol round 2, finding 8): a conflict slot
            // built from two raw rows explains TWO numbers; an equal-value
            // repeat's extra occurrences and a superseded shorter window
            // have "already counted once" as their reason.
            conflicting += result
                .conflicts
                .iter()
                .map(|c| c.contributing_fact_identities.len() as i64)
                .sum::<i64>();
            repeated += result
                .facts
                .iter()
                .map(|f| f.contributing_fact_identities.len() as i64 - 1)
                .sum::<i64>();
            repeated += result.shorter_window_skipped as i64;
            note_level += result.non_primary_statement_skipped as i64;
            awaiting_name += result.uncrosswalked_fact_count as i64;
        }
    }

    Ok(TaggedFactCoverageCounts {
        raw_stored,
        projected,
        comparative,
        dimensional,
        note_level,
        awaiting_name,
        conflicting,
        unparsed,
        repeated,
        other_basis,
    })
}

/// One uncrosswalked concept as observed AT ONE COMPANY (ADR 0100 decision
/// 10): the "positions the program doesn't know yet" list's row shape.
/// `company_count` stays the GLOBAL signal [`harvest_uncrosswalked_concepts`]
/// ranks by — how many issuers across the whole corpus tag this concept — so
/// the list still surfaces the concepts most worth eventually curating first,
/// even though only rows THIS company actually captured are shown (a
/// promotion is company-scoped only — decision 10 never lets a machine widen
/// this to a canonical crosswalk entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyHarvestedConcept {
    pub concept_local_name: String,
    pub concept_namespace_uri: String,
    pub company_count: i64,
    pub occurrence_count: i64,
    /// `balance | income | cash_flow | other` — the dominant presentation
    /// role this company's occurrences carry (ties broken by insertion order).
    pub statement_group: String,
    /// `instant | duration` — every issuer's occurrence of a concept agrees
    /// on this axis (ADR 0100 decision 6 measured zero corpus conflicts);
    /// `duration` when this company's own rows happen to disagree.
    pub period_nature: String,
}

/// `report_tagged_fact_roles.role_kind` -> the `kpi_definitions.statement_group`
/// vocabulary (`income | balance | cash_flow | per_share | other`) — a
/// promoted concept never resolves `per_share` (Layer 1 role classification
/// has no such family), so that arm is simply never reached here.
fn statement_group_for_role(role_kind: &str) -> &'static str {
    match role_kind {
        "balance" => "balance",
        "income" | "comprehensive_income" => "income",
        "cash_flow" => "cash_flow",
        _ => "other",
    }
}

fn harvest_uncrosswalked_concepts_for_company(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<CompanyHarvestedConcept>> {
    let crosswalked: std::collections::HashSet<&str> =
        crate::fundamentals::extraction::ifrs_crosswalk::entries()
            .iter()
            .map(|entry| entry.concept)
            .collect();

    // Keyed by (normalized namespace, local name) — sol rounds 2/3: an
    // issuer extension reusing a standard local name is a DIFFERENT concept
    // and must neither inflate the standard concept's distinct-issuer count
    // nor borrow it — while the annual IFRS taxonomy releases are VERSIONS
    // of one standard vocabulary, so three issuers on three taxonomy years
    // are three issuers of ONE concept, never three one-issuer concepts.
    let mut global_counts: HashMap<(String, String), i64> = HashMap::new();
    {
        let sql = format!(
            "SELECT CASE WHEN {pred} THEN '' ELSE concept_namespace_uri END AS ns,
                    concept_local_name, COUNT(DISTINCT company_id)
             FROM report_tagged_facts GROUP BY ns, concept_local_name",
            pred = crate::fundamentals::extraction::ifrs_crosswalk::STANDARD_IFRS_NAMESPACE_SQL_PREDICATE
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (namespace, concept, count) = row?;
            global_counts.insert((namespace, concept), count);
        }
    }

    let mut statement = connection.prepare(
        "SELECT f.concept_local_name, f.concept_namespace_uri, f.period_type,
                (SELECT r.role_kind FROM report_tagged_fact_roles r WHERE r.fact_id = f.id LIMIT 1)
         FROM report_tagged_facts f
         WHERE f.company_id = ?1 AND f.is_dimensional = 0",
    )?;
    let rows = statement.query_map(params![company_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    struct Agg {
        namespace: String,
        occurrences: i64,
        period_types: std::collections::HashSet<String>,
        roles: HashMap<String, i64>,
    }
    // Aggregation key is (namespace, local name) — two same-named concepts
    // from different namespaces are different rows in this list (sol round
    // 2: promotion must never blur a standard concept with an extension).
    let mut by_concept: HashMap<(String, String), Agg> = HashMap::new();
    for row in rows {
        let (concept, namespace, period_type, role_kind) = row?;
        // Only a STANDARD-namespace concept can be "already crosswalked" (sol
        // review finding 1): an issuer extension reusing a standard local
        // name must surface in this list under its own identity, never be
        // hidden by the standard entry it shadows.
        if crate::fundamentals::extraction::ifrs_crosswalk::is_standard_ifrs_namespace(&namespace)
            && crosswalked.contains(concept.as_str())
        {
            continue;
        }
        let normalized_namespace =
            if crate::fundamentals::extraction::ifrs_crosswalk::is_standard_ifrs_namespace(
                &namespace,
            ) {
                String::new()
            } else {
                namespace.clone()
            };
        let entry = by_concept
            .entry((normalized_namespace, concept))
            .or_insert_with(|| Agg {
                namespace: namespace.clone(),
                occurrences: 0,
                period_types: std::collections::HashSet::new(),
                roles: HashMap::new(),
            });
        entry.occurrences += 1;
        entry.period_types.insert(period_type);
        if let Some(role) = role_kind {
            *entry.roles.entry(role).or_insert(0) += 1;
        }
    }

    let mut out: Vec<CompanyHarvestedConcept> = by_concept
        .into_iter()
        .map(|((normalized_namespace, concept), agg)| {
            let period_nature =
                if agg.period_types.len() == 1 && agg.period_types.contains("instant") {
                    "instant"
                } else {
                    "duration"
                }
                .to_owned();
            let statement_group = agg
                .roles
                .iter()
                .max_by_key(|(_, count)| **count)
                .map(|(role_kind, _)| statement_group_for_role(role_kind).to_owned())
                .unwrap_or_else(|| "other".to_owned());
            CompanyHarvestedConcept {
                company_count: *global_counts
                    .get(&(normalized_namespace, concept.clone()))
                    .unwrap_or(&0),
                occurrence_count: agg.occurrences,
                concept_namespace_uri: agg.namespace,
                concept_local_name: concept,
                statement_group,
                period_nature,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.company_count
            .cmp(&a.company_count)
            .then_with(|| a.concept_local_name.cmp(&b.concept_local_name))
    });
    Ok(out)
}

/// Layer 1 tagged-fact domain store (Architecture v2 / ADR 0050). Owns a
/// [`Database`] and exposes only this domain's operations. Reach it via
/// `AppState::report_tagged_facts()`.
#[derive(Clone)]
pub struct ReportTaggedFactStore {
    db: Database,
}

impl ReportTaggedFactStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn replace_tagged_facts(
        &self,
        report_document_id: &str,
        company_id: &str,
        extraction: &TaggedFactExtraction,
    ) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        replace_tagged_facts(&mut connection, report_document_id, company_id, extraction)
    }

    pub fn facts(&self, report_document_id: &str) -> StorageResult<Vec<StoredTaggedFact>> {
        let connection = self.db.checkout()?;
        get_facts(&connection, report_document_id)
    }

    pub fn extraction(
        &self,
        report_document_id: &str,
    ) -> StorageResult<Option<StoredTaggedFactExtraction>> {
        let connection = self.db.checkout()?;
        get_extraction(&connection, report_document_id)
    }

    /// Concept-harvest command (ADR 0100 decision 2): every Layer 1 concept
    /// with no crosswalk entry, ranked by distinct-company count, with its
    /// observed `period_type`(s) — the input for the next seed migration.
    pub fn harvest_uncrosswalked_concepts(&self) -> StorageResult<Vec<HarvestedConcept>> {
        let connection = self.db.checkout()?;
        harvest_uncrosswalked_concepts(&connection)
    }

    pub fn is_extraction_current(
        &self,
        report_document_id: &str,
        source_content_hash: &str,
        extractor_version: i64,
    ) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        extraction_is_current(
            &connection,
            report_document_id,
            source_content_hash,
            extractor_version,
        )
    }

    /// All of a company's Layer 1 rows, across every report it has ever
    /// tagged (epic #398: the Coverage read model and the promotion action
    /// both need this).
    pub fn facts_for_company(&self, company_id: &str) -> StorageResult<Vec<StoredTaggedFact>> {
        let connection = self.db.checkout()?;
        get_facts_for_company(&connection, company_id)
    }

    /// The Coverage panel's compact read model (epic #398 UI slice) — see
    /// [`coverage_counts`] (the free function) for the exact bucket
    /// semantics.
    pub fn coverage_counts(&self, company_id: &str) -> StorageResult<TaggedFactCoverageCounts> {
        let connection = self.db.checkout()?;
        coverage_counts(&connection, company_id)
    }

    /// The "positions the program doesn't know yet" list for one company
    /// (ADR 0100 decision 10), ranked by the global company-count signal.
    pub fn harvest_uncrosswalked_concepts_for_company(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<CompanyHarvestedConcept>> {
        let connection = self.db.checkout()?;
        harvest_uncrosswalked_concepts_for_company(&connection, company_id)
    }
}

#[cfg(test)]
mod tests;
