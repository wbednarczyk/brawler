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

    let mut statement = connection.prepare(
        "SELECT concept_local_name, COUNT(DISTINCT company_id), GROUP_CONCAT(DISTINCT period_type)
         FROM report_tagged_facts
         GROUP BY concept_local_name
         ORDER BY COUNT(DISTINCT company_id) DESC, concept_local_name ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut harvested = Vec::new();
    for row in rows {
        let (concept_local_name, company_count, period_types_raw) = row?;
        if crosswalked.contains(concept_local_name.as_str()) {
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
/// in Fundamentals. `dimensional` is a direct column count; the other four
/// non-`raw_stored` buckets are computed by re-running
/// [`crate::fundamentals::extraction::esef::projection::project_period`] per
/// `(report_document, period_end)` — including comparative periods, which
/// Layer 1 stores but Layer 2 never writes (decision 7). So `projected` here
/// means "the deterministic rule would keep it", not "it is live in
/// `financial_facts` right now" (a stricter count is a direct
/// `financial_facts` query, unrelated to this read model). `raw_stored` may
/// exceed the other five buckets' sum by the rare count of unparsed-value
/// rows, which fit none of them — visible on the raw fact itself, out of
/// scope for this compact line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TaggedFactCoverageCounts {
    pub raw_stored: i64,
    pub projected: i64,
    pub dimensional: i64,
    pub note_level: i64,
    pub awaiting_name: i64,
    pub conflicting: i64,
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
    let mut note_level = 0i64;
    let mut awaiting_name = 0i64;
    let mut conflicting = 0i64;
    for doc_facts in by_document.values() {
        let has_presentation_linkbase = doc_facts.iter().any(|f| !f.roles.is_empty());
        let mut period_ends: Vec<&str> = doc_facts.iter().map(|f| f.period_end.as_str()).collect();
        period_ends.sort_unstable();
        period_ends.dedup();
        for period_end in period_ends {
            let result = crate::fundamentals::extraction::esef::projection::project_period(
                doc_facts,
                period_end,
                has_presentation_linkbase,
            );
            projected += result.facts.len() as i64;
            conflicting += result.conflicts.len() as i64;
            note_level += result.non_primary_statement_skipped as i64;
            awaiting_name += result.uncrosswalked_fact_count as i64;
        }
    }

    Ok(TaggedFactCoverageCounts {
        raw_stored,
        projected,
        dimensional,
        note_level,
        awaiting_name,
        conflicting,
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

    let mut global_counts: HashMap<String, i64> = HashMap::new();
    {
        let mut statement = connection.prepare(
            "SELECT concept_local_name, COUNT(DISTINCT company_id)
             FROM report_tagged_facts GROUP BY concept_local_name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (concept, count) = row?;
            global_counts.insert(concept, count);
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
    let mut by_concept: HashMap<String, Agg> = HashMap::new();
    for row in rows {
        let (concept, namespace, period_type, role_kind) = row?;
        if crosswalked.contains(concept.as_str()) {
            continue;
        }
        let entry = by_concept.entry(concept).or_insert_with(|| Agg {
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
        .map(|(concept, agg)| {
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
                company_count: *global_counts.get(&concept).unwrap_or(&0),
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
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    fn seed_company_and_document(connection: &Connection, company: &str, doc: &str) {
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES (?1, 'gpw', ?1, ?2, ?3)",
                params![company, format!("GPW:{company}"), format!("{company} SA")],
            )
            .expect("company");
        connection
            .execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
                params![doc, company, format!("https://x/{doc}.zip")],
            )
            .expect("document");
    }

    fn basic_fact(package: &str, identity: &str, concept: &str) -> NewTaggedFact {
        NewTaggedFact {
            package_entry_path: package.to_owned(),
            fact_identity: identity.to_owned(),
            identity_kind: "xml_id".to_owned(),
            concept_namespace_uri: "http://xbrl.ifrs.org/taxonomy/2021-03-24/ifrs-full".to_owned(),
            concept_local_name: concept.to_owned(),
            context_ref: "c1".to_owned(),
            period_type: "duration".to_owned(),
            period_start: Some("2024-01-01".to_owned()),
            period_end: "2024-12-31".to_owned(),
            unit_ref: Some("u1".to_owned()),
            unit_measure: Some("iso4217:PLN".to_owned()),
            value_raw: "1000".to_owned(),
            value_numeric: Some("1000".to_owned()),
            scale: Some(0),
            sign: None,
            decimals: Some("0".to_owned()),
            is_dimensional: false,
            dimensions_json: None,
            parse_status: "ok".to_owned(),
            parse_error: None,
            roles: Vec::new(),
        }
    }

    fn extraction_with(facts: Vec<NewTaggedFact>) -> TaggedFactExtraction {
        TaggedFactExtraction {
            source_content_hash: Some("hash1".to_owned()),
            extractor_version: 1,
            state: "extracted".to_owned(),
            encountered_count: facts.len() as i64,
            stored_count: facts.len() as i64,
            dimensional_count: 0,
            no_linkbase_fallback_count: 0,
            facts,
        }
    }

    #[test]
    fn replace_writes_facts_and_extraction_record() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let extraction = extraction_with(vec![basic_fact(
            "reports/instance.xhtml",
            "xml_id:f1",
            "ProfitLoss",
        )]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

        let facts = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].concept_local_name, "ProfitLoss");
        assert_eq!(facts[0].report_document_id, "doc1");
        assert_eq!(facts[0].company_id, "c1");

        let record = get_extraction(&connection, "doc1")
            .expect("extraction")
            .expect("some");
        assert_eq!(record.source_content_hash.as_deref(), Some("hash1"));
        assert_eq!(record.extractor_version, 1);
        assert_eq!(record.stored_count, 1);
    }

    #[test]
    fn replace_is_atomic_and_idempotent_one_generation() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let extraction = extraction_with(vec![basic_fact(
            "reports/instance.xhtml",
            "xml_id:f1",
            "ProfitLoss",
        )]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("first");
        let first_id = get_facts(&connection, "doc1").expect("facts")[0].id.clone();

        // Writing the SAME extraction again must yield one generation: old
        // rows deleted, new ones inserted (fresh ids), never doubled.
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("second");
        let facts = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(facts.len(), 1, "must not double the generation");
        assert_ne!(
            facts[0].id, first_id,
            "replace mints a fresh id, it does not reuse the old row"
        );
    }

    #[test]
    fn a_mid_transaction_failure_leaves_the_previous_generation_intact() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let first = extraction_with(vec![basic_fact(
            "reports/instance.xhtml",
            "xml_id:f1",
            "ProfitLoss",
        )]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &first).expect("first replace");
        let before = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(before.len(), 1);

        // A poisoned second generation: two facts sharing the SAME identity
        // within the same (document, package_entry_path) — the UNIQUE
        // constraint fails partway through the insert loop, forcing the
        // whole transaction (including the leading DELETE) to roll back.
        let poisoned = extraction_with(vec![
            basic_fact("reports/instance.xhtml", "xml_id:dup", "Revenue"),
            basic_fact("reports/instance.xhtml", "xml_id:dup", "OperatingProfit"),
        ]);
        let error = replace_tagged_facts(&mut connection, "doc1", "c1", &poisoned)
            .expect_err("duplicate fact_identity within one generation must fail");
        assert!(matches!(error, StorageError::Sqlite(_)));

        let after = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(
            after, before,
            "a failed rebuild must never leave a half-generation — the prior generation survives untouched"
        );
    }

    #[test]
    fn unique_key_admits_same_concept_and_context_twice_when_identity_differs() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        // Real shape (ADR 0100 decision 4): ProfitLoss tagged 2x at an
        // identical (concept, context) — distinguished by fact_identity.
        let mut a = basic_fact("reports/instance.xhtml", "xml_id:f1", "ProfitLoss");
        a.context_ref = "ctxA".to_owned();
        let mut b = basic_fact("reports/instance.xhtml", "xml_id:f2", "ProfitLoss");
        b.context_ref = "ctxA".to_owned();

        let extraction = extraction_with(vec![a, b]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction)
            .expect("distinct identities at the same concept/context must both be stored");

        let facts = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(facts.len(), 2);
    }

    #[test]
    fn two_package_instances_can_share_a_fact_identity() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let a = basic_fact("reports/instanceA.xhtml", "xml_id:f1", "ProfitLoss");
        let b = basic_fact("reports/instanceB.xhtml", "xml_id:f1", "ProfitLoss");

        let extraction = extraction_with(vec![a, b]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction)
            .expect("differing package_entry_path must not collide on the same fact_identity");

        let facts = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(facts.len(), 2);
    }

    #[test]
    fn a_failed_normalization_occurrence_is_stored_never_dropped() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let mut broken = basic_fact("reports/instance.xhtml", "xml_id:f1", "ProfitLoss");
        broken.value_raw = "N/A".to_owned();
        broken.value_numeric = None;
        broken.parse_status = "unparsed_value".to_owned();
        broken.parse_error = Some("could not parse 'N/A' as a decimal".to_owned());

        let extraction = extraction_with(vec![broken]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

        let facts = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(facts.len(), 1, "decision 9: never a silent drop");
        assert_eq!(facts[0].value_numeric, None);
        assert_eq!(facts[0].parse_status, "unparsed_value");
        assert!(facts[0].parse_error.is_some());
    }

    #[test]
    fn freshness_check_skips_unchanged_and_rebuilds_on_hash_or_version_change() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let extraction = extraction_with(vec![basic_fact(
            "reports/instance.xhtml",
            "xml_id:f1",
            "ProfitLoss",
        )]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

        assert!(extraction_is_current(&connection, "doc1", "hash1", 1).expect("current"));
        assert!(
            !extraction_is_current(&connection, "doc1", "hash1", 2).expect("version changed"),
            "an extractor_version bump must invalidate"
        );
        assert!(
            !extraction_is_current(&connection, "doc1", "hash2", 1).expect("hash changed"),
            "recapture (a changed source_content_hash) must invalidate"
        );
    }

    #[test]
    fn one_fact_with_several_roles_round_trips() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let mut fact = basic_fact("reports/instance.xhtml", "xml_id:f1", "ProfitLoss");
        fact.roles = vec![
            NewTaggedFactRole {
                role_uri: "ias_1_role-320000".to_owned(),
                role_kind: "income".to_owned(),
            },
            NewTaggedFactRole {
                role_uri: "ias_1_role-610000".to_owned(),
                role_kind: "equity_changes".to_owned(),
            },
        ];

        let extraction = extraction_with(vec![fact]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

        let facts = get_facts(&connection, "doc1").expect("facts");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].roles.len(), 2);
        let kinds: Vec<&str> = facts[0]
            .roles
            .iter()
            .map(|r| r.role_kind.as_str())
            .collect();
        assert!(kinds.contains(&"income"));
        assert!(kinds.contains(&"equity_changes"));
    }

    #[test]
    fn harvest_returns_uncrosswalked_concepts_ranked_by_company_count_and_omits_crosswalked_ones() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");
        seed_company_and_document(&connection, "c2", "doc2");
        seed_company_and_document(&connection, "c3", "doc3");

        // "Assets" IS in the crosswalk (moved verbatim from esef.rs) — must
        // never appear in the harvest output, however many companies tag it.
        let crosswalked = extraction_with(vec![basic_fact(
            "reports/instance.xhtml",
            "xml_id:a",
            "Assets",
        )]);
        replace_tagged_facts(&mut connection, "doc1", "c1", &crosswalked).expect("c1 replace");
        replace_tagged_facts(&mut connection, "doc2", "c2", &crosswalked).expect("c2 replace");

        // A novel concept absent from the crosswalk, tagged by one company —
        // must appear, ranked, with its observed period_type.
        let novel = extraction_with(vec![basic_fact(
            "reports/instance.xhtml",
            "xml_id:n",
            "SomeNovelExtensionConceptNotInTheCrosswalk",
        )]);
        replace_tagged_facts(&mut connection, "doc3", "c3", &novel).expect("c3 replace");

        let harvested = harvest_uncrosswalked_concepts(&connection).expect("harvest");

        assert!(
            harvested.iter().all(|h| h.concept_local_name != "Assets"),
            "a crosswalked concept must never appear in the harvest: {harvested:?}"
        );

        let novel_row = harvested
            .iter()
            .find(|h| h.concept_local_name == "SomeNovelExtensionConceptNotInTheCrosswalk")
            .expect("the novel concept must be present");
        assert_eq!(novel_row.company_count, 1);
        assert_eq!(novel_row.period_types, vec!["duration".to_owned()]);
    }

    fn role(kind: &str) -> NewTaggedFactRole {
        NewTaggedFactRole {
            role_uri: format!("http://x/role/{kind}"),
            role_kind: kind.to_owned(),
        }
    }

    fn seed_document(connection: &Connection, doc: &str, company: &str) {
        connection
            .execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
                params![doc, company, format!("https://x/{doc}.zip")],
            )
            .expect("document");
    }

    #[test]
    fn get_facts_for_company_spans_every_document_that_company_owns() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");
        seed_document(&connection, "doc2", "c1");
        seed_company_and_document(&connection, "c2", "doc3");

        replace_tagged_facts(
            &mut connection,
            "doc1",
            "c1",
            &extraction_with(vec![basic_fact("reports/i.xhtml", "f1", "Assets")]),
        )
        .expect("doc1");
        replace_tagged_facts(
            &mut connection,
            "doc2",
            "c1",
            &extraction_with(vec![basic_fact("reports/i.xhtml", "f2", "Revenue")]),
        )
        .expect("doc2");
        replace_tagged_facts(
            &mut connection,
            "doc3",
            "c2",
            &extraction_with(vec![basic_fact("reports/i.xhtml", "f3", "Equity")]),
        )
        .expect("doc3");

        let facts = get_facts_for_company(&connection, "c1").expect("facts");
        assert_eq!(facts.len(), 2, "must span both of c1's documents");
        assert!(facts.iter().all(|f| f.company_id == "c1"));
        assert!(facts
            .iter()
            .any(|f| f.concept_local_name == "Assets" && f.report_document_id == "doc1"));
        assert!(facts
            .iter()
            .any(|f| f.concept_local_name == "Revenue" && f.report_document_id == "doc2"));
    }

    /// One document, one instant balance-sheet concept ("Assets" — crosswalked,
    /// via the balance role) plus one novel dimensionless concept with no
    /// crosswalk entry and a primary-statement role ("awaiting a name") plus
    /// one dimensional row plus one note-level (no primary-statement role)
    /// row — exercises every non-conflicting bucket in one pass.
    #[test]
    fn coverage_counts_buckets_a_documents_facts_by_projection_outcome() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let mut projected_fact = basic_fact("reports/i.xhtml", "f1", "Assets");
        projected_fact.period_type = "instant".to_owned();
        projected_fact.roles = vec![role("balance")];

        let mut awaiting_name_fact = basic_fact("reports/i.xhtml", "f2", "SomeNovelBalanceConcept");
        awaiting_name_fact.period_type = "instant".to_owned();
        awaiting_name_fact.roles = vec![role("balance")];

        let mut dimensional_fact = basic_fact("reports/i.xhtml", "f3", "Assets");
        dimensional_fact.is_dimensional = true;
        dimensional_fact.roles = vec![role("balance")];

        let mut note_level_fact = basic_fact("reports/i.xhtml", "f4", "SomeNoteConcept");
        note_level_fact.roles = vec![role("other")];

        replace_tagged_facts(
            &mut connection,
            "doc1",
            "c1",
            &extraction_with(vec![
                projected_fact,
                awaiting_name_fact,
                dimensional_fact,
                note_level_fact,
            ]),
        )
        .expect("replace");

        let counts = coverage_counts(&connection, "c1").expect("coverage counts");
        assert_eq!(counts.raw_stored, 4);
        assert_eq!(counts.dimensional, 1);
        assert_eq!(counts.projected, 1, "the crosswalked Assets fact");
        assert_eq!(counts.awaiting_name, 1, "the novel uncrosswalked concept");
        assert_eq!(counts.note_level, 1, "the no-primary-statement-role fact");
        assert_eq!(counts.conflicting, 0);
    }

    #[test]
    fn coverage_counts_reports_a_genuine_value_disagreement_as_conflicting_never_projected() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");

        let mut a = basic_fact("reports/i.xhtml", "f1", "Assets");
        a.period_type = "instant".to_owned();
        a.roles = vec![role("balance")];
        a.value_numeric = Some("1000".to_owned());
        let mut b = basic_fact("reports/i.xhtml", "f2", "Assets");
        b.period_type = "instant".to_owned();
        b.roles = vec![role("balance")];
        b.value_numeric = Some("1500".to_owned());

        replace_tagged_facts(&mut connection, "doc1", "c1", &extraction_with(vec![a, b]))
            .expect("replace");

        let counts = coverage_counts(&connection, "c1").expect("coverage counts");
        assert_eq!(counts.raw_stored, 2);
        assert_eq!(counts.conflicting, 1);
        assert_eq!(
            counts.projected, 0,
            "a genuine disagreement must never resolve into a projected count"
        );
    }

    #[test]
    fn harvest_for_company_ranks_by_the_global_company_count_and_carries_this_companys_occurrences()
    {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection, "c1", "doc1");
        seed_company_and_document(&connection, "c2", "doc2");

        // "WidelyTaggedNovelConcept": tagged by c1 AND c2 — global company_count 2.
        let mut wide_a = basic_fact("reports/i.xhtml", "wide_a", "WidelyTaggedNovelConcept");
        wide_a.period_type = "instant".to_owned();
        wide_a.roles = vec![role("balance")];
        let mut wide_b = basic_fact("reports/i.xhtml", "wide_b", "WidelyTaggedNovelConcept");
        wide_b.period_type = "instant".to_owned();
        wide_b.roles = vec![role("balance")];

        // "NarrowNovelConcept": tagged by c1 only, twice (occurrence_count 2).
        let mut narrow_a = basic_fact("reports/i.xhtml", "narrow_a", "NarrowNovelConcept");
        narrow_a.roles = vec![role("income")];
        let mut narrow_b = basic_fact("reports/i.xhtml", "narrow_b", "NarrowNovelConcept");
        narrow_b.roles = vec![role("income")];

        replace_tagged_facts(
            &mut connection,
            "doc1",
            "c1",
            &extraction_with(vec![wide_a, narrow_a, narrow_b]),
        )
        .expect("c1 replace");
        replace_tagged_facts(
            &mut connection,
            "doc2",
            "c2",
            &extraction_with(vec![wide_b]),
        )
        .expect("c2 replace");

        let rows = harvest_uncrosswalked_concepts_for_company(&connection, "c1")
            .expect("harvest for company");

        assert_eq!(rows.len(), 2, "only c1's own concepts: {rows:?}");
        assert_eq!(
            rows[0].concept_local_name, "WidelyTaggedNovelConcept",
            "ranked by the GLOBAL company count first, {rows:?}"
        );
        assert_eq!(rows[0].company_count, 2);
        assert_eq!(rows[0].occurrence_count, 1, "c1 tagged it once");
        assert_eq!(rows[0].statement_group, "balance");
        assert_eq!(rows[0].period_nature, "instant");

        let narrow = &rows[1];
        assert_eq!(narrow.concept_local_name, "NarrowNovelConcept");
        assert_eq!(narrow.company_count, 1);
        assert_eq!(narrow.occurrence_count, 2, "c1 tagged it twice");
        assert_eq!(narrow.statement_group, "income");
        assert_eq!(narrow.period_nature, "duration");
    }
}
