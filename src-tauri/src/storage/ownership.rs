//! Ownership-stakes storage (ADR 0072, plan v0.56 T2).
//!
//! Who owns a tracked company, and how that changes over time, as **append-only
//! snapshots** per `(company, source, as_of, holder)`. Re-ingesting the same
//! disclosure updates that row in place; a new `as_of` always inserts a fresh
//! row — history is the product and prior snapshots are never rewritten.
//!
//! - **Capital % and votes % are separate** (`capital_pct` / `votes_pct`), each
//!   nullable, stored as decimal-exact TEXT (the `financial_facts.value_numeric`
//!   convention). Preferred-vote shares are common on the GPW and the gap is
//!   itself investor signal.
//! - **Current state** is the latest disclosed stake per holder, selected by the
//!   DOMAIN date (`as_of`) — never `created_at`, which backfill makes diverge.
//! - **Free float is derived, never stored**: `100 − Σ disclosed capital`,
//!   returned with the component sum so the UI can show the uncertainty note.
//! - **Holder classification** comes from a seeded, extensible dictionary
//!   (migration 0082) plus a manual re-type that corrects the label across the
//!   holder's rows without touching pct/as_of/history.

use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use std::str::FromStr;

use super::database::Database;
use super::diagnostics::{record_diagnostic_event, DiagnosticScope, NewDiagnosticEvent};
use super::*;
use crate::fundamentals::ownership::classify::{classify_holder, HolderDictionary};
use crate::fundamentals::ownership::{parse_major_holding_notification, EspiHoldingParse};

/// A stake snapshot to append. `holder_name_normalized` is derived internally
/// from `holder_name_raw` so callers cannot desync the two.
#[derive(Debug, Clone)]
pub struct NewOwnershipStake {
    pub company_id: String,
    pub holder_name_raw: String,
    pub holder_type: Option<String>,
    pub capital_pct: Option<String>,
    pub votes_pct: Option<String>,
    pub as_of: String,
    pub source: String,
    pub report_document_id: Option<String>,
    pub feed_item_id: Option<String>,
}

/// A stored stake snapshot row.
// No `ts_rs` export yet: T2 is headless storage. The TS DTO lands with the read
// command in T6 (avoids an orphaned `src/api/generated/` export in the meantime).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipStakeRow {
    pub id: String,
    pub holder_name_raw: String,
    pub holder_name_normalized: String,
    pub holder_type: Option<String>,
    pub capital_pct: Option<String>,
    pub votes_pct: Option<String>,
    pub as_of: String,
    pub source: String,
    pub report_document_id: Option<String>,
    pub feed_item_id: Option<String>,
    pub created_at: String,
}

/// Current state plus derived free float (ADR 0072 decision 4). Free float is
/// never persisted — it is `100 − disclosed_capital_sum`, floored at 0, and
/// returned with the component sum so the UI can render the uncertainty note
/// (small institutional stakes below the disclosure threshold hide in the float).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipCurrentState {
    pub stakes: Vec<OwnershipStakeRow>,
    /// Σ of disclosed `capital_pct` across current stakes (decimal-exact TEXT).
    pub disclosed_capital_sum: String,
    /// Derived free float = `100 − disclosed_capital_sum`, floored at 0.
    pub free_float_pct: String,
}

/// One seeded holder-dictionary entry (normalized alias → holder type).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HolderDictionaryEntry {
    pub alias_normalized: String,
    pub holder_type: String,
    pub display_name: Option<String>,
}

/// A pending extraction residual (migration 0083, v0.56 T3): a fetched periodic
/// report whose mandatory shareholders table the **deterministic** parser could
/// not turn into stakes (glyph-mangled text, image table, or missing section).
/// NO stakes were written; the AI/OCR path (later slice) and the coverage view
/// consume these — the AI/OCR results ALWAYS require confirmation (ADR 0072).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipExtractionResidual {
    pub report_document_id: String,
    pub company_id: String,
    /// The deterministic parser's terminal non-Found state:
    /// `section_missing | table_unparsable | glyph_encoded`.
    pub parse_state: String,
    pub detected_as_of: Option<String>,
    pub matched_heading: Option<String>,
    /// The tier-4 OCR lifecycle marker (v0.57 T8): `None` = never attempted
    /// (eligible for a bulk OCR pass); `proposed` = a pending OCR proposal
    /// awaits review; `rejected` = the user rejected it (never re-proposed);
    /// `no_table` = OCR yielded no shareholders table (bulk skips, manual
    /// per-company re-arms). See `OCR_STATE_*`.
    pub ocr_state: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A residual awaiting an OCR pass — the residual plus the fields the tier-4
/// job needs to OCR the stored document (from `report_documents`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualNeedingOcr {
    pub report_document_id: String,
    pub company_id: String,
    pub parse_state: String,
    pub detected_as_of: Option<String>,
    pub matched_heading: Option<String>,
}

/// `ocr_state` values on `ownership_extraction_residual` (v0.57 T8). `None`
/// (NULL) means eligible; these are the terminal/awaiting markers.
pub const OCR_STATE_PROPOSED: &str = "proposed";
pub const OCR_STATE_REJECTED: &str = "rejected";
pub const OCR_STATE_NO_TABLE: &str = "no_table";

/// One proposed shareholder row from the tier-4 OCR pass (v0.57 T8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrHolderRow {
    pub holder_name_raw: String,
    pub capital_pct: Option<String>,
    pub votes_pct: Option<String>,
}

/// A new OCR-extracted shareholders table awaiting confirmation. One per
/// residual document; recording it stamps the residual `ocr_state='proposed'`.
#[derive(Debug, Clone)]
pub struct NewOwnershipOcrProposal {
    /// The residual document this proposal resolves (its coverage gap closes on
    /// confirm; its residual is cleared).
    pub report_document_id: String,
    /// The document actually OCR'd — equals `report_document_id` for a PDF
    /// residual, or the PDF sibling for an xhtml container (v0.57 T8).
    pub source_document_id: String,
    pub company_id: String,
    pub as_of: String,
    pub matched_heading: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub rows: Vec<OcrHolderRow>,
}

/// A stored OCR proposal (header + its rows) awaiting confirm/reject. Confirming
/// writes the rows as `report_document` stakes and clears the residual; rejecting
/// parks the residual `ocr_state='rejected'`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipOcrProposalRecord {
    pub report_document_id: String,
    pub source_document_id: String,
    pub company_id: String,
    pub as_of: String,
    pub matched_heading: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    pub rows: Vec<OcrHolderRow>,
}

/// A fetched periodic report document lacking ownership coverage — neither a
/// `report_document`-sourced stake nor a residual row. The extraction triggers
/// (startup catch-up, on-new-report) enqueue exactly these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentNeedingOwnershipExtraction {
    pub company_id: String,
    pub report_document_id: String,
}

/// Normalize a holder name for grouping and dictionary lookup: trim, collapse
/// internal whitespace, uppercase. Deliberately basic in T2; T5 extends it with
/// legal-form suffix stripping. Kept deterministic so the stake id derives stably.
pub(super) fn normalize_holder_name(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// Deterministic stake id (reconciliation idiom, `slug_part`-based): a given
/// `(company, source, as_of, holder)` maps to exactly one row, so re-ingest
/// upserts in place.
fn stake_id(company_id: &str, source: &str, as_of: &str, holder_normalized: &str) -> String {
    format!(
        "ownstake_{}_{}_{}_{}",
        slug_part(company_id),
        slug_part(source),
        slug_part(as_of),
        slug_part(holder_normalized)
    )
}

const STAKE_COLUMNS: &str = "id, holder_name_raw, holder_name_normalized, holder_type, \
     capital_pct, votes_pct, as_of, source, report_document_id, feed_item_id, created_at";

fn row_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<OwnershipStakeRow> {
    Ok(OwnershipStakeRow {
        id: row.get(0)?,
        holder_name_raw: row.get(1)?,
        holder_name_normalized: row.get(2)?,
        holder_type: row.get(3)?,
        capital_pct: row.get(4)?,
        votes_pct: row.get(5)?,
        as_of: row.get(6)?,
        source: row.get(7)?,
        report_document_id: row.get(8)?,
        feed_item_id: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub(super) fn append_snapshot(
    connection: &Connection,
    stake: &NewOwnershipStake,
) -> StorageResult<OwnershipStakeRow> {
    let normalized = normalize_holder_name(&stake.holder_name_raw);
    let id = stake_id(&stake.company_id, &stake.source, &stake.as_of, &normalized);

    // Append-only: same tuple updates the percentages / provenance in place;
    // `created_at` and the domain key are never rewritten. `holder_type` is
    // COALESCEd — an incoming NULL never wipes an existing classification (manual
    // re-types and confirmed proposals always win; ADR 0072 §3, data-model.md
    // Holder-type classification §4), while an explicit incoming type still
    // updates. This is what lets a daily aggregator/report re-ingest (which carries
    // no holder_type) leave a classified holder untouched.
    connection.execute(
        "
        INSERT INTO ownership_stakes (
            id, company_id, holder_name_raw, holder_name_normalized, holder_type,
            capital_pct, votes_pct, as_of, source, report_document_id, feed_item_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            holder_name_raw = excluded.holder_name_raw,
            holder_type = COALESCE(excluded.holder_type, ownership_stakes.holder_type),
            capital_pct = excluded.capital_pct,
            votes_pct = excluded.votes_pct,
            report_document_id = excluded.report_document_id,
            feed_item_id = excluded.feed_item_id
        ",
        params![
            id,
            stake.company_id,
            stake.holder_name_raw,
            normalized,
            stake.holder_type,
            stake.capital_pct,
            stake.votes_pct,
            stake.as_of,
            stake.source,
            stake.report_document_id,
            stake.feed_item_id,
        ],
    )?;

    let row = connection.query_row(
        &format!("SELECT {STAKE_COLUMNS} FROM ownership_stakes WHERE id = ?1"),
        [&id],
        row_from,
    )?;
    Ok(row)
}

/// Latest stake per holder at/after the newest full-picture basis, selected by
/// `as_of` (tie-break: latest `created_at`, then id) — exactly one row per holder.
pub(super) fn current_state(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<OwnershipStakeRow>> {
    // Disclosure-basis rule (real-data harvest 2026-07-16; ADR 0072 amended): a
    // holder who drops below the 5% threshold simply VANISHES from later
    // disclosures — no "0%" row is ever filed. A naive latest-per-holder over all
    // history therefore resurrects stale holders and pushes the disclosed sum
    // past 100%. Current state is instead scoped to the company's newest
    // FULL-PICTURE basis — the max `as_of` across `report_document` AND
    // `aggregator` snapshots (the amendment promotes the aggregator to a breadth
    // source; the newest full picture wins whichever source it came from; fallback
    // to the newest as_of of any source) — overlaid with later espi_filing/manual
    // snapshots. Pre-baseline holders remain in history only.
    let mut statement = connection.prepare(&format!(
        "
        WITH baseline AS (
            SELECT COALESCE(
                (SELECT max(as_of) FROM ownership_stakes
                 WHERE company_id = ?1 AND source IN ('report_document', 'aggregator')),
                (SELECT max(as_of) FROM ownership_stakes WHERE company_id = ?1)
            ) AS baseline_as_of
        )
        SELECT {STAKE_COLUMNS}
        FROM ownership_stakes o, baseline b
        WHERE o.company_id = ?1
          AND o.as_of >= b.baseline_as_of
          AND NOT EXISTS (
            SELECT 1 FROM ownership_stakes o2
            WHERE o2.company_id = o.company_id
              AND o2.holder_name_normalized = o.holder_name_normalized
              AND o2.as_of >= b.baseline_as_of
              AND (
                o2.as_of > o.as_of
                OR (o2.as_of = o.as_of AND o2.created_at > o.created_at)
                OR (o2.as_of = o.as_of AND o2.created_at = o.created_at AND o2.id > o.id)
              )
          )
        ORDER BY o.holder_name_normalized
        "
    ))?;
    let rows = statement.query_map([company_id], row_from)?;
    let mut rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)?;
    // Founder-insider sticky overlay (F-A1). The
    // disclosure-basis rule above legitimately drops an OFE that vanished below
    // the 5% threshold in a newer report. A `founder_insider` is categorically
    // different: a founder crossing below the disclosure threshold is itself an
    // ESPI-disclosable event, not a silent vanish, AND the founder stake is
    // corroborated by the management/insider substrate (the skin-in-the-game
    // join). When a newer full-picture basis only partially captures the
    // shareholder table (OFE funds parsed, founders missed — the ABE live shape),
    // the founder's most-recent disclosed stake must persist in current state so
    // the holder — and its corroboration badge — is still surfaced. Overlay each
    // founder's latest stake ONLY when that holder identity is absent from the
    // baseline-scoped set (a founder still in the newest basis keeps that row).
    overlay_sticky_founders(connection, company_id, &mut rows)?;
    dedup_stakes_by_identity(connection, rows)
}

/// Append each `founder_insider` holder's most-recent stake to `rows` when that
/// holder's canonical identity is not already present. Founders are natural
/// persons (no dictionary alias), so the parenthetical-stripped canonical key is
/// their identity — the same key the founder-stamping and badge joins use.
fn overlay_sticky_founders(
    connection: &Connection,
    company_id: &str,
    rows: &mut Vec<OwnershipStakeRow>,
) -> StorageResult<()> {
    let present: std::collections::HashSet<String> = rows
        .iter()
        .map(|row| {
            crate::fundamentals::ownership::classify::canonical_holder_identity(
                &row.holder_name_raw,
            )
        })
        .collect();
    // Latest stake per founder holder, across ALL history (newest as_of, then
    // newest disclosure, then id — mirrors the baseline tie-break).
    let mut statement = connection.prepare(&format!(
        "
        SELECT {STAKE_COLUMNS}
        FROM ownership_stakes o
        WHERE o.company_id = ?1
          AND o.holder_type = 'founder_insider'
          AND NOT EXISTS (
            SELECT 1 FROM ownership_stakes o2
            WHERE o2.company_id = o.company_id
              AND o2.holder_name_normalized = o.holder_name_normalized
              AND (
                o2.as_of > o.as_of
                OR (o2.as_of = o.as_of AND o2.created_at > o.created_at)
                OR (o2.as_of = o.as_of AND o2.created_at = o.created_at AND o2.id > o.id)
              )
          )
        ORDER BY o.holder_name_normalized
        "
    ))?;
    let founders = statement
        .query_map([company_id], row_from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)?;
    for founder in founders {
        let identity = crate::fundamentals::ownership::classify::canonical_holder_identity(
            &founder.holder_name_raw,
        );
        if !identity.is_empty() && !present.contains(&identity) {
            rows.push(founder);
        }
    }
    Ok(())
}

/// The DISCLOSED-only reference state — the pre-amendment `current_state`
/// semantics kept for the reversed witness path: baseline = newest
/// `report_document` disclosure (fallback: newest of any source), and the
/// `aggregator` source is excluded on both the main and the tie-break scan so the
/// aggregator is never compared against itself (ADR 0072 §2c amended 2026-07-16).
pub(super) fn disclosed_reference_state(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<OwnershipStakeRow>> {
    let mut statement = connection.prepare(&format!(
        "
        WITH baseline AS (
            SELECT COALESCE(
                (SELECT max(as_of) FROM ownership_stakes
                 WHERE company_id = ?1 AND source = 'report_document'),
                (SELECT max(as_of) FROM ownership_stakes WHERE company_id = ?1)
            ) AS baseline_as_of
        )
        SELECT {STAKE_COLUMNS}
        FROM ownership_stakes o, baseline b
        WHERE o.company_id = ?1
          AND o.as_of >= b.baseline_as_of
          AND o.source != 'aggregator'
          AND NOT EXISTS (
            SELECT 1 FROM ownership_stakes o2
            WHERE o2.company_id = o.company_id
              AND o2.holder_name_normalized = o.holder_name_normalized
              AND o2.as_of >= b.baseline_as_of
              AND o2.source != 'aggregator'
              AND (
                o2.as_of > o.as_of
                OR (o2.as_of = o.as_of AND o2.created_at > o.created_at)
                OR (o2.as_of = o.as_of AND o2.created_at = o.created_at AND o2.id > o.id)
              )
          )
        ORDER BY o.holder_name_normalized
        "
    ))?;
    let rows = statement.query_map([company_id], row_from)?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)?;
    dedup_stakes_by_identity(connection, rows)
}

/// Merge a scoped stake set by holder IDENTITY, keeping one row per identity.
///
/// Same-basis documents (e.g. the quarterly report and its attachment) print the
/// same holder with cosmetic name variants ("PTE Allianz Polska" vs "PTE Allianz
/// Polska S.A."), which are distinct `holder_name_normalized` identities by design
/// (append-only ids stay stable). Merge by IDENTITY:
/// a shared dictionary `display_name` when the name resolves to a seeded alias
/// ("NN PTE" and "Nationale-Nederlanden PTE S.A." are one entity), else the
/// parenthetical-stripped canonical key ("cyber_Folks S.A." vs "cyber_Folks S.A.
/// (akcje własne)"). The most specific (longest) raw name represents the merged
/// holder — it carries the richer type signal.
fn dedup_stakes_by_identity(
    connection: &Connection,
    rows: Vec<OwnershipStakeRow>,
) -> StorageResult<Vec<OwnershipStakeRow>> {
    let identity_map = crate::fundamentals::ownership::classify::HolderIdentityMap::from_pairs(
        load_holder_dictionary(connection)?
            .into_iter()
            // Generic MARKER aliases ("akcje własne") classify a row but are
            // not an entity identity — the issuer's own name is; the
            // parenthetical-strip path merges those. Entity aliases (funds,
            // state bodies) do act as identities.
            .filter(|entry| entry.holder_type != "treasury_shares")
            .map(|entry| {
                let identity = entry
                    .display_name
                    .unwrap_or_else(|| entry.alias_normalized.clone());
                (entry.alias_normalized, identity)
            }),
    );
    let mut by_identity: std::collections::BTreeMap<String, OwnershipStakeRow> =
        std::collections::BTreeMap::new();
    for row in rows {
        let key = identity_map
            .resolve(&row.holder_name_raw)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                crate::fundamentals::ownership::classify::canonical_holder_identity(
                    &row.holder_name_raw,
                )
            });
        match by_identity.get(&key) {
            Some(existing)
                if (existing.holder_name_raw.len(), &existing.created_at)
                    >= (row.holder_name_raw.len(), &row.created_at) => {}
            _ => {
                by_identity.insert(key, row);
            }
        }
    }
    Ok(by_identity.into_values().collect())
}

/// Current state plus derived free float.
pub(super) fn current_state_with_free_float(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<OwnershipCurrentState> {
    let stakes = current_state(connection, company_id)?;
    let mut sum = Decimal::ZERO;
    for stake in &stakes {
        if let Some(value) = &stake.capital_pct {
            if let Ok(parsed) = Decimal::from_str(value.trim()) {
                sum += parsed;
            }
        }
    }
    let hundred = Decimal::from(100);
    let free_float = (hundred - sum).max(Decimal::ZERO);
    Ok(OwnershipCurrentState {
        stakes,
        disclosed_capital_sum: sum.normalize().to_string(),
        free_float_pct: free_float.normalize().to_string(),
    })
}

/// Free float over time, one point per FULL-PICTURE disclosure basis (ADR 0072
/// amended 2026-07-16): for every `as_of` carrying `report_document`- or
/// `aggregator`-sourced snapshots, float = `100 − Σ` of that basis's disclosed
/// capital, deduped by holder identity first so cosmetic variant pairs ("nn pte" /
/// "Nationale-Nederlanden PTE S.A.") don't double-count and understate the float.
/// A report basis and an aggregator basis sharing an `as_of` merge into one point.
/// ESPI single-holder updates are deliberately not bases — they change one holder,
/// not the whole disclosure picture.
pub(super) fn free_float_history(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<(String, String)>> {
    let mut statement = connection.prepare(&format!(
        "
        SELECT {STAKE_COLUMNS}
        FROM ownership_stakes
        WHERE company_id = ?1 AND source IN ('report_document', 'aggregator')
        ORDER BY as_of, holder_name_normalized
        "
    ))?;
    let rows = statement.query_map([company_id], row_from)?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)?;

    let identity_map = crate::fundamentals::ownership::classify::HolderIdentityMap::from_pairs(
        load_holder_dictionary(connection)?
            .into_iter()
            .filter(|entry| entry.holder_type != "treasury_shares")
            .map(|entry| {
                let identity = entry
                    .display_name
                    .unwrap_or_else(|| entry.alias_normalized.clone());
                (entry.alias_normalized, identity)
            }),
    );

    let hundred = Decimal::from(100);
    let mut points: Vec<(String, String)> = Vec::new();
    let mut basis_start = 0usize;
    while basis_start < rows.len() {
        let as_of = rows[basis_start].as_of.clone();
        let basis_end = rows[basis_start..]
            .iter()
            .position(|row| row.as_of != as_of)
            .map(|offset| basis_start + offset)
            .unwrap_or(rows.len());
        let mut by_identity: std::collections::BTreeMap<String, Decimal> =
            std::collections::BTreeMap::new();
        for row in &rows[basis_start..basis_end] {
            let key = identity_map
                .resolve(&row.holder_name_raw)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    crate::fundamentals::ownership::classify::canonical_holder_identity(
                        &row.holder_name_raw,
                    )
                });
            if let Some(value) = row
                .capital_pct
                .as_deref()
                .and_then(|value| Decimal::from_str(value.trim()).ok())
            {
                // One value per identity per basis (variants print the same stake).
                by_identity.entry(key).or_insert(value);
            }
        }
        let sum: Decimal = by_identity.values().copied().sum();
        let float = (hundred - sum).max(Decimal::ZERO);
        points.push((as_of, float.normalize().to_string()));
        basis_start = basis_end;
    }
    Ok(points)
}

/// Write a parsed aggregator table as a full-picture `aggregator` basis at
/// `as_of`, in ONE transaction (ADR 0072 §2c amended 2026-07-16). Every holder is
/// upserted (deterministic id, so an unchanged page upserts in place) and then the
/// SAME-basis row set is reconciled: `aggregator` rows at this `(company, as_of)`
/// whose holder is no longer on the page are deleted (a same-basis correction).
/// Prior bases and every other source (`report_document`/`espi_filing`/`manual`)
/// are never touched. Returns the number of holders written.
pub(super) fn replace_aggregator_basis(
    connection: &Connection,
    company_id: &str,
    as_of: &str,
    holders: &[WitnessHolder],
) -> StorageResult<usize> {
    let transaction =
        rusqlite::Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Immediate)?;

    let mut written_normalized: Vec<String> = Vec::with_capacity(holders.len());
    for holder in holders {
        append_snapshot(
            &transaction,
            &NewOwnershipStake {
                company_id: company_id.to_owned(),
                holder_name_raw: holder.holder_name.clone(),
                holder_type: None,
                capital_pct: holder
                    .capital_pct
                    .map(|value| value.normalize().to_string()),
                votes_pct: holder.votes_pct.map(|value| value.normalize().to_string()),
                as_of: as_of.to_owned(),
                source: "aggregator".to_owned(),
                report_document_id: None,
                feed_item_id: None,
            },
        )?;
        written_normalized.push(normalize_holder_name(&holder.holder_name));
    }

    // Same-basis reconcile: drop aggregator rows at this basis no longer present.
    if written_normalized.is_empty() {
        transaction.execute(
            "DELETE FROM ownership_stakes
             WHERE company_id = ?1 AND source = 'aggregator' AND as_of = ?2",
            params![company_id, as_of],
        )?;
    } else {
        let placeholders = std::iter::repeat_n("?", written_normalized.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM ownership_stakes
             WHERE company_id = ? AND source = 'aggregator' AND as_of = ?
               AND holder_name_normalized NOT IN ({placeholders})"
        );
        let mut sql_params: Vec<&dyn rusqlite::ToSql> =
            Vec::with_capacity(written_normalized.len() + 2);
        sql_params.push(&company_id);
        sql_params.push(&as_of);
        for name in &written_normalized {
            sql_params.push(name);
        }
        transaction.execute(&sql, sql_params.as_slice())?;
    }

    transaction.commit()?;
    Ok(written_normalized.len())
}

/// All snapshots for a company (optionally one holder), newest `as_of` first.
pub(super) fn history(
    connection: &Connection,
    company_id: &str,
    holder_normalized: Option<&str>,
) -> StorageResult<Vec<OwnershipStakeRow>> {
    let mut sql = format!("SELECT {STAKE_COLUMNS} FROM ownership_stakes WHERE company_id = ?1");
    if holder_normalized.is_some() {
        sql.push_str(" AND holder_name_normalized = ?2");
    }
    sql.push_str(" ORDER BY as_of DESC, created_at DESC, id DESC");

    let mut statement = connection.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| row_from(row);
    let rows = match holder_normalized {
        Some(holder) => statement.query_map(params![company_id, holder], map)?,
        None => statement.query_map(params![company_id], map)?,
    };
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Manual re-type: correct a holder's classification across all their snapshot
/// rows. A classification label, not a financial fact — it never creates a new
/// snapshot and never touches pct/as_of. Returns the number of rows updated.
pub(super) fn set_holder_type(
    connection: &Connection,
    company_id: &str,
    holder_normalized: &str,
    holder_type: Option<&str>,
) -> StorageResult<usize> {
    let affected = connection.execute(
        "
        UPDATE ownership_stakes
        SET holder_type = ?3
        WHERE company_id = ?1 AND holder_name_normalized = ?2
        ",
        params![company_id, holder_normalized, holder_type],
    )?;
    Ok(affected)
}

/// The seeded holder dictionary for the deterministic classifier.
pub(super) fn load_holder_dictionary(
    connection: &Connection,
) -> StorageResult<Vec<HolderDictionaryEntry>> {
    let mut statement = connection.prepare(
        "SELECT alias_normalized, holder_type, display_name
         FROM ownership_holder_dictionary
         ORDER BY alias_normalized",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(HolderDictionaryEntry {
            alias_normalized: row.get(0)?,
            holder_type: row.get(1)?,
            display_name: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

// ---- Holder-type classification (T5, ADR 0072 §3) ----
//
// Order: dictionary (exact + containment) → heuristic name markers → AI
// classify-with-confirm (proposals below, never auto-applied) → manual re-type
// (`set_holder_type`) always wins. The deterministic pass only ever stamps rows
// whose `holder_type` is NULL, so a manual or previously-confirmed label survives
// a re-classification untouched.

/// Build the deterministic matcher from the seeded dictionary rows.
fn holder_dictionary(connection: &Connection) -> StorageResult<HolderDictionary> {
    Ok(HolderDictionary::from_aliases(
        load_holder_dictionary(connection)?
            .into_iter()
            .map(|entry| (entry.alias_normalized, entry.holder_type)),
    ))
}

/// Distinct still-unclassified (NULL `holder_type`) holders for a company.
pub(super) fn unclassified_holders(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT holder_name_normalized FROM ownership_stakes
         WHERE company_id = ?1 AND holder_type IS NULL
         ORDER BY holder_name_normalized",
    )?;
    let rows = statement.query_map([company_id], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Companies that still have at least one unclassified holder (the AI job's work
/// list). Ordered for determinism.
pub(super) fn companies_with_unclassified_holders(
    connection: &Connection,
) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT company_id FROM ownership_stakes
         WHERE holder_type IS NULL
         ORDER BY company_id",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Stamp `holder_type` on this company's NULL rows via the dictionary + heuristic
/// markers. Only NULL rows are touched (manual/confirmed labels are preserved) and
/// no snapshot is created — a classification label, never a financial fact.
/// Returns the number of stake rows stamped.
pub(super) fn classify_unclassified_for_company(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<usize> {
    let dictionary = holder_dictionary(connection)?;
    let mut stamped = 0usize;
    for holder in unclassified_holders(connection, company_id)? {
        if let Some(holder_type) = classify_holder(&dictionary, &holder) {
            stamped += connection.execute(
                "UPDATE ownership_stakes SET holder_type = ?3
                 WHERE company_id = ?1 AND holder_name_normalized = ?2 AND holder_type IS NULL",
                params![company_id, holder, holder_type],
            )?;
        }
    }
    Ok(stamped)
}

/// A new AI-proposed holder-type classification awaiting confirmation (T5).
#[derive(Debug, Clone)]
pub struct NewHolderTypeProposal {
    pub company_id: String,
    pub holder_name_normalized: String,
    pub proposed_type: String,
    pub confidence: Option<f64>,
    pub rationale: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

/// A stored holder-type proposal row. No `ts_rs` export yet: the review commands
/// (and their DTO) land in T6.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HolderTypeProposalRow {
    pub id: String,
    pub company_id: String,
    pub holder_name_normalized: String,
    pub proposed_type: String,
    pub confidence: Option<f64>,
    pub rationale: Option<String>,
    pub status: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Confirm a proposal: apply the proposed type across the holder's snapshot rows
/// via [`set_holder_type`] (a user confirmation overrides any earlier stamp), then
/// mark the proposal confirmed.
/// Reject a proposal: mark it rejected, leaving `holder_type` NULL. A confirmed
/// proposal is never re-opened.
pub(super) const RESIDUAL_COLUMNS: &str = "report_document_id, company_id, parse_state, \
     detected_as_of, matched_heading, ocr_state, created_at, updated_at";

fn residual_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<OwnershipExtractionResidual> {
    Ok(OwnershipExtractionResidual {
        report_document_id: row.get(0)?,
        company_id: row.get(1)?,
        parse_state: row.get(2)?,
        detected_as_of: row.get(3)?,
        matched_heading: row.get(4)?,
        ocr_state: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// Upsert the residual for one document (idempotent per `report_document_id`):
/// a re-run of the deterministic parse on the same still-unparsable document
/// refreshes `parse_state` / `detected_as_of` / `matched_heading` in place
/// without duplicating. `created_at` is never rewritten.
pub(super) fn record_extraction_residual(
    connection: &Connection,
    residual: &OwnershipExtractionResidual,
) -> StorageResult<OwnershipExtractionResidual> {
    connection.execute(
        "
        INSERT INTO ownership_extraction_residual (
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
                "SELECT {RESIDUAL_COLUMNS} FROM ownership_extraction_residual \
             WHERE report_document_id = ?1"
            ),
            [&residual.report_document_id],
            residual_from,
        )
        .map_err(StorageError::from)
}

/// The residual for one document, if any.
pub(super) fn get_extraction_residual(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Option<OwnershipExtractionResidual>> {
    connection
        .query_row(
            &format!(
                "SELECT {RESIDUAL_COLUMNS} FROM ownership_extraction_residual \
                 WHERE report_document_id = ?1"
            ),
            [report_document_id],
            residual_from,
        )
        .optional()
        .map_err(StorageError::from)
}

/// Pending residuals for a company, newest first.
pub(super) fn list_extraction_residuals(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<OwnershipExtractionResidual>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {RESIDUAL_COLUMNS} FROM ownership_extraction_residual \
         WHERE company_id = ?1 ORDER BY created_at DESC, report_document_id DESC"
    ))?;
    let rows = statement.query_map([company_id], residual_from)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Remove a document's residual (self-heal): the extraction job calls this the
/// moment a parse succeeds, so a document is never both parsed and residual.
pub(super) fn clear_extraction_residual(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<usize> {
    let affected = connection.execute(
        "DELETE FROM ownership_extraction_residual WHERE report_document_id = ?1",
        [report_document_id],
    )?;
    Ok(affected)
}

/// Fetched periodic report documents lacking ownership coverage — neither a
/// `report_document`-sourced stake nor a residual row. When `company_id` is
/// `Some`, restricted to that company. This is the exact selection the startup
/// catch-up and on-new-report triggers enqueue (a projection, so the triggers
/// and the coverage view never disagree about what still needs extracting).
pub(super) fn documents_needing_ownership_extraction(
    connection: &Connection,
    company_id: Option<&str>,
) -> StorageResult<Vec<DocumentNeedingOwnershipExtraction>> {
    let mut sql = String::from(
        "
        SELECT rd.company_id, rd.id
        FROM report_documents rd
        WHERE rd.fetch_status = 'fetched'
          AND rd.local_path IS NOT NULL
          AND rd.doc_kind IN ('periodic_ssf', 'periodic_jsf')
          AND NOT EXISTS (
            SELECT 1 FROM ownership_stakes os
            WHERE os.report_document_id = rd.id AND os.source = 'report_document'
          )
          AND NOT EXISTS (
            SELECT 1 FROM ownership_extraction_residual r
            WHERE r.report_document_id = rd.id
          )
        ",
    );
    if company_id.is_some() {
        sql.push_str(" AND rd.company_id = ?1");
    }
    sql.push_str(" ORDER BY rd.created_at DESC");

    let mut statement = connection.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| {
        Ok(DocumentNeedingOwnershipExtraction {
            company_id: row.get(0)?,
            report_document_id: row.get(1)?,
        })
    };
    let rows = match company_id {
        Some(id) => statement.query_map([id], map)?,
        None => statement.query_map([], map)?,
    };
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

// ---------------------------------------------------------------------------
// Tier-4 OCR proposals (v0.57 T8, ADR 0077 over ADR 0072 decision 2a)
// ---------------------------------------------------------------------------

/// Residuals eligible for a tier-4 OCR pass. `company_id = None` scans every
/// company (the bulk pass); `Some` narrows to one. `include_no_table` re-arms
/// the `no_table` markers too — the manual per-company retry uses it, the bulk
/// pass does not (so a doc OCR already found unreadable is not re-spent). A
/// `proposed` or `rejected` residual is NEVER selected (awaiting review / the
/// user said no). Joined to `report_documents` so the caller can OCR the file.
pub(super) fn residuals_needing_ocr(
    connection: &Connection,
    company_id: Option<&str>,
    include_no_table: bool,
    include_section_missing: bool,
) -> StorageResult<Vec<ResidualNeedingOcr>> {
    // NULL is always eligible; `no_table` only when re-arming. `proposed`/
    // `rejected` are always excluded.
    let state_clause = if include_no_table {
        "(r.ocr_state IS NULL OR r.ocr_state = 'no_table')"
    } else {
        "r.ocr_state IS NULL"
    };
    // Parse-state scope (orchestrator gate, T8 real-data): the bulk pass targets
    // the card's population — `table_unparsable` + `glyph_encoded` (unreadable
    // tables that OCR can defeat). `section_missing` (a document genuinely
    // lacking the shareholders section) is EXCLUDED from bulk to avoid burning
    // provider calls on nothing; the manual per-company retry includes it.
    let parse_clause = if include_section_missing {
        "r.parse_state IN ('table_unparsable', 'glyph_encoded', 'section_missing')"
    } else {
        "r.parse_state IN ('table_unparsable', 'glyph_encoded')"
    };
    let mut sql = format!(
        "SELECT r.report_document_id, r.company_id, r.parse_state, \
                r.detected_as_of, r.matched_heading \
         FROM ownership_extraction_residual r \
         JOIN report_documents rd ON rd.id = r.report_document_id \
         WHERE {state_clause} AND {parse_clause} \
           AND rd.fetch_status = 'fetched' AND rd.local_path IS NOT NULL"
    );
    if company_id.is_some() {
        sql.push_str(" AND r.company_id = ?1");
    }
    sql.push_str(" ORDER BY r.created_at DESC, r.report_document_id DESC");
    let mut statement = connection.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| {
        Ok(ResidualNeedingOcr {
            report_document_id: row.get(0)?,
            company_id: row.get(1)?,
            parse_state: row.get(2)?,
            detected_as_of: row.get(3)?,
            matched_heading: row.get(4)?,
        })
    };
    let rows = match company_id {
        Some(id) => statement.query_map([id], map)?,
        None => statement.query_map([], map)?,
    };
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Stamp a residual's `ocr_state` (a no-op if the residual is gone). Used to
/// mark `no_table` (OCR yielded nothing) and by confirm/reject indirectly.
pub(super) fn set_residual_ocr_state(
    connection: &Connection,
    report_document_id: &str,
    ocr_state: Option<&str>,
) -> StorageResult<()> {
    connection.execute(
        "UPDATE ownership_extraction_residual \
         SET ocr_state = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE report_document_id = ?1",
        params![report_document_id, ocr_state],
    )?;
    Ok(())
}

/// Record (upsert) an OCR shareholders-table proposal for one residual document
/// and mark the residual `ocr_state='proposed'`. Idempotent per document: the
/// header upserts and the rows are fully replaced, so a re-OCR reconciles.
/// **Writes no stakes** — confirmation is the only door.
/// The OCR proposal for one document, if any (header + rows).
/// All pending OCR proposals for a company (header + rows), newest first.
/// Confirm an OCR proposal: write each proposed row as a `report_document`
/// stake at the proposal's `as_of` (the standard extraction write path), stamp
/// the deterministic holder-type classification, CLEAR the residual entirely,
/// and delete the proposal. Idempotent per confirm. Returns rows written.
/// Reject an OCR proposal: delete it and park the residual `ocr_state='rejected'`
/// so the bulk/manual pass never re-proposes it. Writes no stakes.
/// Ownership domain store (ADR 0072). `AppState::ownership()`.
#[derive(Clone)]
pub struct OwnershipStore {
    db: Database,
}

impl OwnershipStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Append a stake snapshot (upsert by deterministic id; append-only history).
    pub fn append_snapshot(&self, stake: NewOwnershipStake) -> StorageResult<OwnershipStakeRow> {
        let connection = self.db.checkout()?;
        append_snapshot(&connection, &stake)
    }

    /// Latest stake per holder at/after the newest full-picture basis, by `as_of`.
    pub fn current_state(&self, company_id: &str) -> StorageResult<Vec<OwnershipStakeRow>> {
        let connection = self.db.checkout()?;
        current_state(&connection, company_id)
    }

    /// The disclosed-only reference state (excludes `aggregator`) — the reversed
    /// witness compares the aggregator against this, never against `current_state`.
    pub fn disclosed_reference_state(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<OwnershipStakeRow>> {
        let connection = self.db.checkout()?;
        disclosed_reference_state(&connection, company_id)
    }

    /// Write a parsed aggregator table as a full-picture `aggregator` basis at
    /// `as_of` (one transaction; same-basis re-ingest reconciles the row set).
    /// Returns the number of holders written.
    pub fn replace_aggregator_basis(
        &self,
        company_id: &str,
        as_of: &str,
        holders: &[WitnessHolder],
    ) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        replace_aggregator_basis(&connection, company_id, as_of, holders)
    }

    /// Current state plus derived free float and the disclosed-capital sum.
    pub fn current_state_with_free_float(
        &self,
        company_id: &str,
    ) -> StorageResult<OwnershipCurrentState> {
        let connection = self.db.checkout()?;
        current_state_with_free_float(&connection, company_id)
    }

    /// Derived free float per full-picture basis (`report_document` + `aggregator`
    /// `as_of` groups), `(as_of, pct)` ascending.
    pub fn free_float_history(&self, company_id: &str) -> StorageResult<Vec<(String, String)>> {
        let connection = self.db.checkout()?;
        free_float_history(&connection, company_id)
    }

    /// Snapshots over time (optionally one holder), newest `as_of` first.
    pub fn history(
        &self,
        company_id: &str,
        holder_normalized: Option<&str>,
    ) -> StorageResult<Vec<OwnershipStakeRow>> {
        let connection = self.db.checkout()?;
        history(&connection, company_id, holder_normalized)
    }

    /// Manual re-type across the holder's rows (never rewrites history).
    pub fn set_holder_type(
        &self,
        company_id: &str,
        holder_normalized: &str,
        holder_type: Option<&str>,
    ) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        set_holder_type(&connection, company_id, holder_normalized, holder_type)
    }

    /// The seeded holder dictionary for the classifier.
    pub fn load_holder_dictionary(&self) -> StorageResult<Vec<HolderDictionaryEntry>> {
        let connection = self.db.checkout()?;
        load_holder_dictionary(&connection)
    }

    /// Upsert a document's extraction residual (idempotent per document).
    pub fn record_extraction_residual(
        &self,
        residual: OwnershipExtractionResidual,
    ) -> StorageResult<OwnershipExtractionResidual> {
        let connection = self.db.checkout()?;
        record_extraction_residual(&connection, &residual)
    }

    /// The residual for one document, if any.
    pub fn get_extraction_residual(
        &self,
        report_document_id: &str,
    ) -> StorageResult<Option<OwnershipExtractionResidual>> {
        let connection = self.db.checkout()?;
        get_extraction_residual(&connection, report_document_id)
    }

    /// Pending residuals for a company, newest first.
    pub fn list_extraction_residuals(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<OwnershipExtractionResidual>> {
        let connection = self.db.checkout()?;
        list_extraction_residuals(&connection, company_id)
    }

    /// Remove a document's residual (self-heal on a later successful parse).
    pub fn clear_extraction_residual(&self, report_document_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        clear_extraction_residual(&connection, report_document_id)
    }

    /// Fetched periodic documents lacking ownership coverage (no stake, no
    /// residual). `company_id = None` scans every company.
    pub fn documents_needing_ownership_extraction(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<DocumentNeedingOwnershipExtraction>> {
        let connection = self.db.checkout()?;
        documents_needing_ownership_extraction(&connection, company_id)
    }

    // ---- Holder-type classification (T5) ----

    /// Stamp `holder_type` on this company's unclassified rows via the dictionary
    /// and heuristic markers (only NULL rows; manual/confirmed labels preserved).
    /// Returns the number of stake rows stamped.
    pub fn classify_unclassified_for_company(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        classify_unclassified_for_company(&connection, company_id)
    }

    /// Distinct still-unclassified holders for a company (the AI job's residual).
    pub fn unclassified_holders(&self, company_id: &str) -> StorageResult<Vec<String>> {
        let connection = self.db.checkout()?;
        unclassified_holders(&connection, company_id)
    }

    /// Companies that still have at least one unclassified holder.
    pub fn companies_with_unclassified_holders(&self) -> StorageResult<Vec<String>> {
        let connection = self.db.checkout()?;
        companies_with_unclassified_holders(&connection)
    }

    // ---- Tier-4 OCR proposals (T8) ----

    /// Residuals eligible for a tier-4 OCR pass (`None` = all companies).
    /// `include_no_table` re-arms `no_table` markers (the manual per-company
    /// retry); the bulk pass passes `false`.
    pub fn residuals_needing_ocr(
        &self,
        company_id: Option<&str>,
        include_no_table: bool,
        include_section_missing: bool,
    ) -> StorageResult<Vec<ResidualNeedingOcr>> {
        let connection = self.db.checkout()?;
        residuals_needing_ocr(
            &connection,
            company_id,
            include_no_table,
            include_section_missing,
        )
    }

    /// Stamp a residual's `ocr_state` (e.g. `no_table` after an empty OCR run).
    pub fn set_residual_ocr_state(
        &self,
        report_document_id: &str,
        ocr_state: Option<&str>,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        set_residual_ocr_state(&connection, report_document_id, ocr_state)
    }

    // ---- ESPI major-holdings stake update (T4 stream 2) ----

    /// Deterministically update `espi_filing` stakes from newly-confirmed
    /// `major_holdings_change` signals. Clean parse → append snapshot; any
    /// ambiguity → unparsed marker + diagnostic (never guessed). Idempotent:
    /// each filing is attempted exactly once. Returns stakes written.
    pub fn update_stakes_from_major_holdings(&self) -> StorageResult<usize> {
        let mut connection = self.db.checkout()?;
        update_stakes_from_major_holdings(&mut connection)
    }

    // ---- Reversed aggregator witness (T4 stream 3) ----

    /// Persist a batch of aggregator-vs-disclosed comparisons (the aggregator is
    /// witnessed by the disclosed-only reference) and mark the adapter healthy
    /// (`last_success_at`). Records a diagnostic per divergence. This recorder
    /// writes no stakes — the basis write is `replace_aggregator_basis`. Returns
    /// the run's ingestion summary (items = companies compared, unmatched =
    /// total divergences).
    pub fn record_witness_comparisons(
        &self,
        adapter_id: &str,
        comparisons: &[WitnessComparison],
        checked_at: &str,
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;
        let (holders, divergences) =
            record_witness_comparisons(&mut connection, adapter_id, comparisons, checked_at)?;
        Ok(SourceIngestionResult {
            adapter_id: adapter_id.to_owned(),
            items_fetched: comparisons.len(),
            items_created: 0,
            items_matched: holders as usize,
            items_unmatched: divergences as usize,
            detail_items_attempted: 0,
            detail_items_stored: 0,
            detail_items_failed: 0,
            fetched_at: Some(checked_at.to_owned()),
        })
    }

    /// The last stored witness comparison for one (adapter, company), if any.
    pub fn get_witness_result(
        &self,
        adapter_id: &str,
        company_id: &str,
    ) -> StorageResult<Option<OwnershipWitnessResult>> {
        let connection = self.db.checkout()?;
        get_witness_result(&connection, adapter_id, company_id)
    }

    /// Record a diagnostic that a company's parsed aggregator basis was implausible
    /// (disclosed capital > 102%) so nothing was written (developer-mode event).
    pub fn record_aggregator_implausible(
        &self,
        company_id: &str,
        ticker: &str,
        capital_sum: &str,
    ) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        record_diagnostic_event(
            &mut connection,
            NewDiagnosticEvent {
                occurred_at: None,
                module: "ownership".to_owned(),
                scope: Some(DiagnosticScope {
                    scope_type: "company".to_owned(),
                    id: Some(company_id.to_owned()),
                }),
                stage: "ownership_aggregator_implausible".to_owned(),
                severity: "warning".to_owned(),
                message: "Aggregator ownership capital sum implausible (>102%); nothing written"
                    .to_owned(),
                metadata: Some(serde_json::json!({
                    "ticker": ticker,
                    "capitalSum": capital_sum,
                })),
            },
        )?;
        Ok(())
    }
}

// ============================================================================
// ESPI major-holdings stake update (ADR 0072 §2b, plan v0.56 T4 stream 2)
// ============================================================================

/// A confirmed `major_holdings_change` filing awaiting a deterministic stake
/// update: not yet turned into an `espi_filing` stake, nor marked unparsed.
struct PendingMajorHoldings {
    feed_item_id: String,
    company_id: String,
    title: String,
    body_text: String,
    disclosed_at: Option<String>,
}

fn load_pending_major_holdings(
    connection: &Connection,
) -> StorageResult<Vec<PendingMajorHoldings>> {
    let mut statement = connection.prepare(
        "
        SELECT cs.feed_item_id, cs.company_id, fi.title,
               COALESCE(fi.body_text, ''), fi.published_at
        FROM company_signals cs
        JOIN feed_items fi ON fi.id = cs.feed_item_id
        WHERE cs.category = 'major_holdings_change'
          AND cs.status = 'confirmed'
          AND fi.body_text IS NOT NULL AND TRIM(fi.body_text) <> ''
          AND NOT EXISTS (
            SELECT 1 FROM ownership_stakes os
            WHERE os.feed_item_id = cs.feed_item_id AND os.source = 'espi_filing'
          )
          AND NOT EXISTS (
            SELECT 1 FROM ownership_espi_unparsed u WHERE u.feed_item_id = cs.feed_item_id
          )
        ORDER BY cs.created_at DESC
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PendingMajorHoldings {
            feed_item_id: row.get(0)?,
            company_id: row.get(1)?,
            title: row.get(2)?,
            body_text: row.get(3)?,
            disclosed_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Record an ESPI filing the deterministic parser could not safely turn into a
/// stake (idempotent per feed item). NO stake is written — never guess.
fn record_espi_unparsed(
    connection: &Connection,
    feed_item_id: &str,
    company_id: &str,
    reason: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO ownership_espi_unparsed (feed_item_id, company_id, reason)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(feed_item_id) DO UPDATE SET
            company_id = excluded.company_id,
            reason = excluded.reason,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![feed_item_id, company_id, reason],
    )?;
    Ok(())
}

fn emit_unparsed_diagnostic(
    connection: &mut Connection,
    filing: &PendingMajorHoldings,
    reason: &str,
) {
    let _ = record_diagnostic_event(
        connection,
        NewDiagnosticEvent {
            occurred_at: None,
            module: "ownership".to_owned(),
            scope: Some(DiagnosticScope {
                scope_type: "company".to_owned(),
                id: Some(filing.company_id.clone()),
            }),
            stage: "espi_stake_update".to_owned(),
            severity: "warning".to_owned(),
            message: "Major-holdings notification could not be parsed into a stake".to_owned(),
            metadata: Some(serde_json::json!({
                "kind": "ownership_espi_unparsed",
                "feedItemId": filing.feed_item_id,
                "reason": reason,
            })),
        },
    );
}

pub(super) fn update_stakes_from_major_holdings(
    connection: &mut Connection,
) -> StorageResult<usize> {
    let pending = load_pending_major_holdings(connection)?;
    let mut written = 0usize;
    for filing in pending {
        match parse_major_holding_notification(&filing.title, &filing.body_text) {
            EspiHoldingParse::Clean(holding) => {
                let as_of = filing
                    .disclosed_at
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.chars().take(10).collect::<String>());
                let Some(as_of) = as_of else {
                    record_espi_unparsed(
                        connection,
                        &filing.feed_item_id,
                        &filing.company_id,
                        "missing_disclosure_date",
                    )?;
                    emit_unparsed_diagnostic(connection, &filing, "missing_disclosure_date");
                    continue;
                };
                append_snapshot(
                    connection,
                    &NewOwnershipStake {
                        company_id: filing.company_id.clone(),
                        holder_name_raw: holding.holder_name,
                        holder_type: None,
                        capital_pct: holding.capital_pct.map(|d| d.normalize().to_string()),
                        votes_pct: holding.votes_pct.map(|d| d.normalize().to_string()),
                        as_of,
                        source: "espi_filing".to_owned(),
                        report_document_id: None,
                        feed_item_id: Some(filing.feed_item_id.clone()),
                    },
                )?;
                written += 1;
            }
            EspiHoldingParse::Ambiguous(reason) => {
                record_espi_unparsed(connection, &filing.feed_item_id, &filing.company_id, reason)?;
                emit_unparsed_diagnostic(connection, &filing, reason);
            }
            EspiHoldingParse::NotFound => {
                record_espi_unparsed(
                    connection,
                    &filing.feed_item_id,
                    &filing.company_id,
                    "not_found",
                )?;
                emit_unparsed_diagnostic(connection, &filing, "not_found");
            }
        }
    }
    Ok(written)
}

// ============================================================================
// Aggregator ownership breadth source + reversed witness
// (ADR 0072 §2c amended 2026-07-16, plan v0.56 T4 stream 3)
// ============================================================================
//
// The aggregator "Akcjonariat" page is the automatic BREADTH source: each refresh
// writes the parsed table as a full-picture `aggregator` basis (see
// `replace_aggregator_basis`) and `current_state` counts it as a full-picture
// basis. The witness comparison stays but reversed — the aggregator is compared
// against the DISCLOSED-only reference (`disclosed_reference_state`, reports/ESPI),
// so it is never compared against itself; divergences are diagnostics only. The
// comparison recorder below writes no stakes — the basis write is the separate
// `replace_aggregator_basis` path.

/// Above this disclosed/aggregator percentage, a holder present on only one side
/// is a divergence worth flagging (the GPW ≥5% disclosure threshold).
const WITNESS_PRESENCE_THRESHOLD: f64 = 5.0;
/// Absolute percentage-point gap above which a matched holder diverges.
const WITNESS_PCT_TOLERANCE: f64 = 1.0;

/// One holder row parsed from an aggregator "Akcjonariat" table.
#[derive(Debug, Clone, PartialEq)]
pub struct WitnessHolder {
    pub holder_name: String,
    pub capital_pct: Option<Decimal>,
    pub votes_pct: Option<Decimal>,
}

/// One divergence between the aggregator and our disclosed current state.
#[derive(Debug, Clone, PartialEq)]
pub struct WitnessDivergence {
    pub holder_normalized: String,
    /// `missing_in_aggregator | missing_in_disclosed | pct_gap`.
    pub kind: String,
    pub disclosed_pct: Option<String>,
    pub aggregator_pct: Option<String>,
}

/// The comparison of one company's aggregator table to disclosed current state.
#[derive(Debug, Clone, PartialEq)]
pub struct WitnessComparison {
    pub company_id: String,
    /// `agree | diverged | no_reference`.
    pub status: String,
    pub holders_compared: i64,
    pub divergences: Vec<WitnessDivergence>,
}

/// A stored witness-result row (last comparison per adapter+company).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipWitnessResult {
    pub id: String,
    pub adapter_id: String,
    pub company_id: String,
    pub status: String,
    pub holders_compared: i64,
    pub divergence_count: i64,
    pub checked_at: String,
}

fn primary_pct(capital: Option<Decimal>, votes: Option<Decimal>) -> Option<Decimal> {
    capital.or(votes)
}

fn decimal_to_f64(value: Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    value.to_f64().unwrap_or(0.0)
}

/// Pure comparison of an aggregator table against disclosed current state.
/// Capital % is the primary axis (votes % as fallback when capital is absent).
/// Matches holders by the shared normalized key; name-form mismatches surface as
/// one-sided presence (expected witness noise for a coarse health check).
pub fn compare_witness(
    company_id: &str,
    aggregator: &[WitnessHolder],
    disclosed: &[OwnershipStakeRow],
) -> WitnessComparison {
    if disclosed.is_empty() {
        return WitnessComparison {
            company_id: company_id.to_owned(),
            status: "no_reference".to_owned(),
            holders_compared: 0,
            divergences: Vec::new(),
        };
    }

    // Index aggregator rows by normalized holder name.
    let mut agg: std::collections::BTreeMap<String, Option<Decimal>> =
        std::collections::BTreeMap::new();
    for holder in aggregator {
        agg.insert(
            normalize_holder_name(&holder.holder_name),
            primary_pct(holder.capital_pct, holder.votes_pct),
        );
    }

    let mut divergences = Vec::new();
    let mut compared = 0i64;
    let mut matched_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for stake in disclosed {
        let key = stake.holder_name_normalized.clone();
        let disclosed_pct = stake
            .capital_pct
            .as_deref()
            .or(stake.votes_pct.as_deref())
            .and_then(|value| Decimal::from_str(value.trim()).ok());
        match agg.get(&key) {
            Some(agg_pct) => {
                matched_keys.insert(key.clone());
                compared += 1;
                if let (Some(d), Some(a)) = (disclosed_pct, agg_pct) {
                    if (decimal_to_f64(d) - decimal_to_f64(*a)).abs() > WITNESS_PCT_TOLERANCE {
                        divergences.push(WitnessDivergence {
                            holder_normalized: key,
                            kind: "pct_gap".to_owned(),
                            disclosed_pct: Some(d.normalize().to_string()),
                            aggregator_pct: Some(a.normalize().to_string()),
                        });
                    }
                }
            }
            None => {
                if disclosed_pct
                    .map(|d| decimal_to_f64(d) >= WITNESS_PRESENCE_THRESHOLD)
                    .unwrap_or(false)
                {
                    divergences.push(WitnessDivergence {
                        holder_normalized: key,
                        kind: "missing_in_aggregator".to_owned(),
                        disclosed_pct: disclosed_pct.map(|d| d.normalize().to_string()),
                        aggregator_pct: None,
                    });
                }
            }
        }
    }

    // Aggregator holders above threshold that we do not disclose at all.
    for holder in aggregator {
        let key = normalize_holder_name(&holder.holder_name);
        if matched_keys.contains(&key) {
            continue;
        }
        let disclosed_here = disclosed.iter().any(|s| s.holder_name_normalized == key);
        if disclosed_here {
            continue;
        }
        let agg_pct = primary_pct(holder.capital_pct, holder.votes_pct);
        if agg_pct
            .map(|a| decimal_to_f64(a) >= WITNESS_PRESENCE_THRESHOLD)
            .unwrap_or(false)
        {
            divergences.push(WitnessDivergence {
                holder_normalized: key,
                kind: "missing_in_disclosed".to_owned(),
                disclosed_pct: None,
                aggregator_pct: agg_pct.map(|a| a.normalize().to_string()),
            });
        }
    }

    let status = if divergences.is_empty() {
        "agree".to_owned()
    } else {
        "diverged".to_owned()
    };
    WitnessComparison {
        company_id: company_id.to_owned(),
        status,
        holders_compared: compared,
        divergences,
    }
}

fn witness_result_id(adapter_id: &str, company_id: &str) -> String {
    format!("ownwit_{}_{}", slug_part(adapter_id), slug_part(company_id))
}

/// Persist the comparisons + one diagnostic per divergence, then mark the witness
/// adapter healthy. Returns `(holders_compared_total, divergences_total)`.
pub(super) fn record_witness_comparisons(
    connection: &mut Connection,
    adapter_id: &str,
    comparisons: &[WitnessComparison],
    checked_at: &str,
) -> StorageResult<(i64, i64)> {
    let mut holders_total = 0i64;
    let mut divergences_total = 0i64;
    for comparison in comparisons {
        let id = witness_result_id(adapter_id, &comparison.company_id);
        let divergence_count = comparison.divergences.len() as i64;
        holders_total += comparison.holders_compared;
        divergences_total += divergence_count;
        connection.execute(
            "
            INSERT INTO ownership_witness_results (
                id, adapter_id, company_id, status,
                holders_compared, divergence_count, checked_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                holders_compared = excluded.holders_compared,
                divergence_count = excluded.divergence_count,
                checked_at = excluded.checked_at,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                id,
                adapter_id,
                comparison.company_id,
                comparison.status,
                comparison.holders_compared,
                divergence_count,
                checked_at,
            ],
        )?;
        for divergence in &comparison.divergences {
            let _ = record_diagnostic_event(
                connection,
                NewDiagnosticEvent {
                    occurred_at: None,
                    module: "ownership".to_owned(),
                    scope: Some(DiagnosticScope {
                        scope_type: "company".to_owned(),
                        id: Some(comparison.company_id.clone()),
                    }),
                    stage: "witness_divergence".to_owned(),
                    severity: "warning".to_owned(),
                    message: "Aggregator ownership differs from disclosed current state".to_owned(),
                    metadata: Some(serde_json::json!({
                        "adapterId": adapter_id,
                        "holder": divergence.holder_normalized,
                        "kind": divergence.kind,
                        "disclosedPct": divergence.disclosed_pct,
                        "aggregatorPct": divergence.aggregator_pct,
                    })),
                },
            );
        }
    }

    // Mark the witness adapter healthy (`last_success_at`) — DoD §C. Best-effort
    // (no-op when the catalog row is not yet seeded).
    let _ = super::ingestion::record_source_outcome(
        connection,
        adapter_id,
        checked_at,
        comparisons.len(),
        0,
        holders_total as usize,
        divergences_total as usize,
    );

    Ok((holders_total, divergences_total))
}

fn get_witness_result(
    connection: &Connection,
    adapter_id: &str,
    company_id: &str,
) -> StorageResult<Option<OwnershipWitnessResult>> {
    connection
        .query_row(
            "
            SELECT id, adapter_id, company_id, status,
                   holders_compared, divergence_count, checked_at
            FROM ownership_witness_results
            WHERE adapter_id = ?1 AND company_id = ?2
            ",
            params![adapter_id, company_id],
            |row| {
                Ok(OwnershipWitnessResult {
                    id: row.get(0)?,
                    adapter_id: row.get(1)?,
                    company_id: row.get(2)?,
                    status: row.get(3)?,
                    holders_compared: row.get(4)?,
                    divergence_count: row.get(5)?,
                    checked_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}
