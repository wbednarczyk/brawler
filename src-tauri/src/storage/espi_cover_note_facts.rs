//! Ingest-time ESPI cover-note ("WYBRANE DANE FINANSOWE") fact extraction —
//! tier 2a of the deterministic fundamentals pipeline
//! ([ADR 0061](../../../docs/adr/0061-deterministic-fundamentals-data-gathering.md)
//! decision 1).
//!
//! The mandated cover table travels as **plain text in the body of a periodic-
//! report komunikat**, which the Bankier company primary already ingests. This
//! module turns that body into validated `financial_facts` at the moment it
//! lands, and is deliberately **not** a lazy/on-demand reader: the carrier text
//! is prunable (the ADR records a feed prune that deleted 448 of 451 WDF
//! carrier bodies), so a body not parsed at ingest is a body lost.
//!
//! Placement and failure policy: this runs **after** the ingest transaction has
//! committed, alongside the other post-commit enhancers in
//! [`super::sources::ingest_bankier_company_items`] (signal classification,
//! ownership stakes, red flags, insider cover notes). Feed ingestion is the more
//! important guarantee, so an extraction failure can never roll it back — the
//! feed rows are already durable when this is entered, and every per-item error
//! is recorded, never propagated.
//!
//! What it does NOT do: invent anything. The period comes from the feed-title
//! entry point of the shared derivation
//! ([`crate::report_diff::classify::period_from_feed_title`]) — same grammar as
//! the document pipeline, but it strips the `z dnia <date>` publication idiom and
//! lets an explicit period marker win over a lone date (a bare feed-title date is
//! usually the publication date, not the reporting period); an underivable period
//! abstains. The facts go through the **same** `validate_parsed_set`
//! gate as every other tier — no bypass. Tier precedence is enforced against the
//! stored provenance: a fact already produced by ESEF/structured xHTML is never
//! touched, a fact from the PDF/aggregator tiers is upgraded (ADR 0061 dec. 1,
//! "a KPI is taken from the highest available tier").

use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use crate::fundamentals::extraction::espi_cover_note::{
    parse_espi_cover_note, WdfEmptyReason, WdfParseResult,
};
use crate::fundamentals::extraction::pipeline::validate_parsed_set;
use crate::fundamentals::extraction::SourceTier;
use crate::source_adapters::bankier_company::{
    is_periodic_report_item, BankierCompanyItem, ADAPTER_ID,
};

use super::diagnostics::{record_diagnostic_event, DiagnosticScope, NewDiagnosticEvent};
use super::financials::{expected_primary_metric_keys, stored_fact_set_for_cross_check};
use super::kpi_extraction::record_structured_fact;
use super::{StorageResult, StructuredFactCommit, StructuredFactInput};

/// The `financial_fact_provenance.source_tier` this tier writes.
const TIER: SourceTier = SourceTier::EspiCoverNote;
/// Diagnostic `module` for every outcome recorded here.
const DIAGNOSTIC_MODULE: &str = "espi_cover_note";
/// The `financial_facts.extraction_method` marker for this tier's rows — the
/// sub-tier discriminator alongside `source_tier` (never trust-bearing itself).
const EXTRACTION_METHOD: &str = "espi_cover_note";

/// One ingested komunikat that may carry a cover table: the feed item it landed
/// as, paired with the adapter item still holding its body text.
pub(super) struct CoverNoteCarrier<'a> {
    pub feed_item_id: String,
    pub item: &'a BankierCompanyItem,
}

/// What one sweep did, for the structured ingest log and for tests to assert
/// against. Every counter here is also visible per item as a diagnostic event.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct CoverNoteExtractionSummary {
    /// Periodic-report items with a non-empty body that were attempted.
    pub attempted: usize,
    /// Items whose reporting period could not be derived — nothing extracted.
    pub no_period: usize,
    /// Items whose body carried no parseable cover table.
    pub empty: usize,
    /// Items whose parsed set the validation gate refused to emit.
    pub flagged: usize,
    /// Facts newly written.
    pub created: usize,
    /// Facts that upgraded a lower-tier stored value.
    pub upgraded: usize,
    /// Slots left alone because an equal-or-higher tier already owns them.
    pub deferred: usize,
    /// Monetary rows the PLN↔EUR cross-check refused (never guessed).
    pub abstained: usize,
    /// Per-item errors — recorded, never propagated to the feed ingest.
    pub errors: usize,
    /// Periods whose persisted cover-note facts AGREED with a cached aggregator
    /// witness (corroboration recorded — ADR 0085).
    pub witness_agreed: usize,
    /// Periods whose persisted cover-note facts DISAGREED with a cached witness
    /// (a `witness_disagreement` outcome recorded; values left in place).
    pub witness_disagreed: usize,
    /// Periods where the witness page was not cached — the comparison was
    /// DEFERRED (no ingest-time fetch, ADR 0085 decision 3), never a false
    /// agreement.
    pub witness_pending: usize,
}

/// Named zero-effect reasons the cover-note tier can state (epic #40 S5). The
/// SAME vocabulary serves the ingest sweep and the rebuild re-scan, which is
/// why the re-scan carries the per-item counters through instead of reducing
/// them to a bare `facts_written = 0`.
pub(crate) mod cover_note_effect_reason {
    /// The reporting period could not be derived — nothing to attribute facts to.
    pub const NO_PERIOD: &str = "no_period";
    /// The body carried no parseable cover table.
    pub const NO_TABLE: &str = "no_table";
    /// The validation gate refused the parsed set.
    pub const FLAGGED: &str = "flagged";
    /// An equal-or-higher tier already owns every slot.
    pub const HIGHER_TIER_HOLDS_SLOT: &str = "higher_tier_holds_slot";
    /// The PLN↔EUR cross-check refused the monetary rows (never guessed).
    pub const ABSTAINED: &str = "abstained";
    /// Per-item errors — recorded, never propagated; they are the reason.
    pub const ITEM_ERRORS: &str = "item_errors";
    /// The carriers' bodies were pruned — lost by design, unrecoverable.
    pub const BODY_PRUNED: &str = "body_pruned";
}

impl crate::effects_honesty::ExplainsEffect for CoverNoteExtractionSummary {
    fn effect_verdict(&self) -> crate::effects_honesty::EffectVerdict {
        use crate::effects_honesty::{first_named, verdict};
        use cover_note_effect_reason as reason;

        let reason = first_named([
            (reason::ITEM_ERRORS, self.errors > 0),
            (reason::FLAGGED, self.flagged > 0),
            (reason::NO_PERIOD, self.no_period > 0),
            (reason::NO_TABLE, self.empty > 0),
            (reason::HIGHER_TIER_HOLDS_SLOT, self.deferred > 0),
            (reason::ABSTAINED, self.abstained > 0),
        ]);
        verdict(
            self.created > 0 || self.upgraded > 0,
            self.attempted > 0,
            reason,
        )
    }
}

impl crate::effects_honesty::ExplainsEffect for CoverNoteRescanSummary {
    fn effect_verdict(&self) -> crate::effects_honesty::EffectVerdict {
        use crate::effects_honesty::{first_named, verdict};
        use cover_note_effect_reason as reason;

        let reason = first_named([
            (reason::ITEM_ERRORS, self.errors > 0),
            (reason::FLAGGED, self.flagged > 0),
            (reason::NO_PERIOD, self.no_period > 0),
            (reason::NO_TABLE, self.no_table > 0),
            (reason::HIGHER_TIER_HOLDS_SLOT, self.deferred > 0),
            (reason::ABSTAINED, self.abstained > 0),
            (reason::BODY_PRUNED, self.skipped_no_body > 0),
        ]);
        verdict(
            self.facts_written > 0,
            self.carriers_scanned > 0 || self.skipped_no_body > 0,
            reason,
        )
    }
}

/// Runs the cover-note tier over the komunikaty this ingest just committed.
///
/// Never returns `Err` for a per-item problem: an item that cannot be parsed,
/// validated, or persisted is counted and recorded as a diagnostic, and the
/// sweep moves on. The `StorageResult` covers only the caller's own bookkeeping.
pub(super) fn extract_cover_note_facts(
    connection: &mut Connection,
    carriers: &[CoverNoteCarrier<'_>],
) -> StorageResult<CoverNoteExtractionSummary> {
    let mut summary = CoverNoteExtractionSummary::default();

    for carrier in carriers {
        let item = carrier.item;
        // Only periodic-report komunikaty carry the mandated table. Reuses the
        // deterministic ESPI classifier the attachment-fetch gate already uses,
        // rather than a second title heuristic.
        if !is_periodic_report_item(item) {
            continue;
        }
        let Some(body) = item.body_text.as_deref().filter(|b| !b.trim().is_empty()) else {
            continue;
        };
        if item.company_id.trim().is_empty() {
            continue;
        }
        summary.attempted += 1;

        if let Err(error) = extract_one(connection, carrier, body, &mut summary) {
            summary.errors += 1;
            log::warn!(
                "module={DIAGNOSTIC_MODULE} stage=extract feed_item={} error={error}",
                carrier.feed_item_id
            );
            let _ = record(
                connection,
                carrier,
                "error",
                "error",
                "cover-note extraction failed",
                json!({ "error": error.to_string() }),
            );
        }
    }

    Ok(summary)
}

/// What one full re-scan of the stored cover-note carriers did, for the
/// `rebuild fundamentals` verdict (ADR 0086 dec. 6, plan TOR C slice C4).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CoverNoteRescanSummary {
    /// Periodic-report komunikaty whose body is still stored and were attempted.
    pub carriers_scanned: usize,
    /// Facts newly written or tier-upgraded across the whole scan.
    pub facts_written: usize,
    /// Periodic-report komunikaty whose carrier body has been pruned — lost by
    /// design (a measured feed prune removed 448/451 WDF bodies), skipped, counted.
    pub skipped_no_body: usize,
    /// Per-item errors — recorded, never propagated (the scan never aborts on one).
    pub errors: usize,
    // The per-item WHY counters, carried through from the inner
    // [`CoverNoteExtractionSummary`] (epic #40 S5). Without them a re-scan that
    // read 450 stored komunikaty and wrote nothing reported
    // `carriers_scanned = 450, facts_written = 0` and could not say why — the
    // zero-effect-success class ADR 0091 exists to kill. Aggregate counts, so
    // the rebuild verdict names the cause without re-reading the diagnostics.
    /// Items whose reporting period could not be derived.
    pub no_period: usize,
    /// Items whose body carried no parseable cover table.
    pub no_table: usize,
    /// Items whose parsed set the validation gate refused to emit.
    pub flagged: usize,
    /// Slots left alone because an equal-or-higher tier already owns them.
    pub deferred: usize,
    /// Monetary rows the PLN↔EUR cross-check refused (never guessed).
    pub abstained: usize,
}

/// Re-run the cover-note (WDF) tier over every STORED Bankier-company komunikat
/// whose body is still in hand — the one-off repopulation pass of the `rebuild
/// fundamentals` flow (ADR 0086 dec. 6). Unlike the ingest hook, which parses the
/// just-fetched bodies, this reconstructs carriers from persisted feed items and
/// reuses the SAME [`extract_cover_note_facts`] entry (same period derivation,
/// same validation gate, same tier-precedence-aware writes) — so a re-scan adds
/// nothing new to a slot an equal-or-higher tier already holds and re-observes
/// its own values idempotently.
///
/// A periodic-report komunikat whose body was pruned is unrecoverable by design
/// and is counted (`skipped_no_body`), never guessed at. Never returns `Err` for
/// a per-item problem: those are counted inside [`extract_cover_note_facts`].
pub fn rescan_stored_cover_note_facts(
    connection: &mut Connection,
) -> StorageResult<CoverNoteRescanSummary> {
    // Phase 1: collect only the LIGHT row identities (no bodies) — a long-lived
    // install can hold thousands of komunikat bodies, and materializing them all
    // at once is an unbounded memory spike (review 2026-07-22). Bodies stream
    // one at a time in phase 2.
    let mut skipped_no_body = 0usize;
    let mut ids: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    {
        let mut statement = connection.prepare(
            "
            SELECT fi.id
            FROM feed_items fi
            JOIN feed_item_companies fic ON fic.feed_item_id = fi.id
            WHERE fi.source_adapter_id = ?1
            ",
        )?;
        let rows = statement.query_map([ADAPTER_ID], |row| row.get::<_, String>(0))?;
        for row in rows {
            let feed_item_id = row?;
            // One carrier per komunikat even if it maps to several companies
            // (mirrors the ingest hook, which builds one carrier per item).
            if seen.insert(feed_item_id.clone()) {
                ids.push(feed_item_id);
            }
        }
    }

    // Phase 2: one body resident at a time, extracted through the same entry as
    // the ingest hook (per-carrier slice — the entry is per-item tolerant).
    let mut totals = CoverNoteRescanSummary::default();
    for feed_item_id in ids {
        let row = {
            let mut statement = connection.prepare(
                "
                SELECT fi.title, fi.source_url, fi.body_text, fic.company_id
                FROM feed_items fi
                JOIN feed_item_companies fic ON fic.feed_item_id = fi.id
                WHERE fi.id = ?1
                LIMIT 1
                ",
            )?;
            statement
                .query_row([&feed_item_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .optional()?
        };
        let Some((title, source_url, body_text, company_id)) = row else {
            continue;
        };
        let has_body = body_text
            .as_deref()
            .map(|body| !body.trim().is_empty())
            .unwrap_or(false);
        let item = reconstruct_carrier_item(company_id, title, source_url, body_text);
        if !has_body {
            if is_periodic_report_item(&item) {
                // A periodic report whose body was pruned — lost by design.
                skipped_no_body += 1;
            }
            continue;
        }
        let carrier = CoverNoteCarrier {
            feed_item_id: feed_item_id.clone(),
            item: &item,
        };
        let summary = extract_cover_note_facts(connection, std::slice::from_ref(&carrier))?;
        totals.carriers_scanned += summary.attempted;
        totals.facts_written += summary.created + summary.upgraded;
        totals.errors += summary.errors;
        // Carry the per-item WHY through (epic #40 S5) — a re-scan that writes
        // nothing must be able to name the cause, not just report a zero.
        totals.no_period += summary.no_period;
        totals.no_table += summary.empty;
        totals.flagged += summary.flagged;
        totals.deferred += summary.deferred;
        totals.abstained += summary.abstained;
    }

    totals.skipped_no_body = skipped_no_body;
    Ok(totals)
}

/// Build the minimal [`BankierCompanyItem`] the cover-note extract path reads
/// from a stored feed row. Only `company_id`, `title`, `link`, and `body_text`
/// are load-bearing; everything else is an inert default.
fn reconstruct_carrier_item(
    company_id: String,
    title: String,
    link: String,
    body_text: Option<String>,
) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id,
        qualified_ticker: String::new(),
        title,
        link,
        summary: String::new(),
        published_at: None,
        fetched_at: String::new(),
        article_id: String::new(),
        pub_id: 0,
        dedupe_key: String::new(),
        duplicate_signature: String::new(),
        body_text,
        attachments: Vec::new(),
        detail_fetch_attempted: false,
    }
}

fn extract_one(
    connection: &mut Connection,
    carrier: &CoverNoteCarrier<'_>,
    body: &str,
    summary: &mut CoverNoteExtractionSummary,
) -> StorageResult<()> {
    let item = carrier.item;

    // --- Period: derived, never guessed ---------------------------------
    let Some((fiscal_year, period_type, period_end)) =
        crate::report_diff::classify::period_from_feed_title(&item.title, &item.link)
    else {
        summary.no_period += 1;
        record(
            connection,
            carrier,
            "no_period",
            "warning",
            "reporting period could not be derived from the komunikat title/URL — abstained",
            json!({ "title": item.title, "link": item.link }),
        )?;
        return Ok(());
    };

    // --- Parse ----------------------------------------------------------
    let parsed = parse_espi_cover_note(body, &period_end);
    summary.abstained += parsed.abstained;

    if parsed.facts.is_empty() {
        summary.empty += 1;
        record(
            connection,
            carrier,
            "empty",
            "info",
            "no cover-note facts parsed from the komunikat body",
            empty_metadata(&parsed, &period_end, period_type),
        )?;
        return Ok(());
    }

    // --- The same validation gate every tier runs through ----------------
    let prior_end = prior_period_end(&period_end);
    let prior = stored_fact_set_for_cross_check(
        connection,
        &item.company_id,
        fiscal_year - 1,
        period_type,
        TIER,
    )?;
    let expected = expected_primary_metric_keys(connection, &item.company_id)?;
    let (acceptance, _status) = validate_parsed_set(
        &parsed.facts,
        &period_end,
        prior.as_ref(),
        prior_end.as_deref(),
        expected.as_ref(),
    );

    if !acceptance.emits() {
        summary.flagged += 1;
        record(
            connection,
            carrier,
            "flagged",
            "warning",
            "cover-note facts failed the validation gate — nothing emitted",
            json!({
                "periodEnd": period_end,
                "periodType": period_type,
                "acceptance": acceptance.as_str(),
                "facts": parsed.facts.len(),
                "abstained": parsed.abstained,
            }),
        )?;
        return Ok(());
    }

    // --- Persist, tier-precedence aware ----------------------------------
    let validation_status = acceptance.validation_status();
    // Review-free facts (ADR 0086 decision 5): every automatic writer stamps
    // `confirmed` — origin truth lives in source_tier + extraction_method +
    // validation_status + citation, never in a review queue.
    let confirmation_state = "confirmed";
    let mut seen = std::collections::BTreeSet::new();
    let mut created = 0usize;
    let mut upgraded = 0usize;
    let mut deferred = 0usize;
    let mut upgrades: Vec<serde_json::Value> = Vec::new();
    // The metric keys THIS run actually persisted (created or upgraded) — the
    // slots for which the cover-note tier is the effective primary, and therefore
    // the only slots the aggregator witness may corroborate. A slot deferred to an
    // equal-or-higher tier is that tier's to defend, never the witness's here.
    let mut persisted_keys: Vec<String> = Vec::new();

    for fact in &parsed.facts {
        if !seen.insert(fact.metric_key.clone()) {
            continue;
        }
        let value = fact.value.to_string();
        // The citation names the FEED ITEM, not a document: the carrier body can
        // be pruned, so the persisted evidence must identify the komunikat that
        // published the table (ADR 0061 tier 2a).
        let citation = format!("{} | feed_item:{}", fact.citation, carrier.feed_item_id);
        let commit = record_structured_fact(
            connection,
            StructuredFactInput {
                company_id: &item.company_id,
                fiscal_year,
                period_type,
                period_end: Some(&period_end),
                report_document_id: &carrier.feed_item_id,
                metric_key: &fact.metric_key,
                value_numeric: &value,
                currency: fact.currency.as_deref(),
                confirmation_state,
                source_tier: TIER.as_str(),
                extraction_method: EXTRACTION_METHOD,
                validation_status,
                drift_json: None,
                citation: Some(&citation),
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )?;

        match commit {
            StructuredFactCommit::Created(_) => {
                created += 1;
                persisted_keys.push(fact.metric_key.clone());
            }
            // Same slot, same value, same-or-higher stored tier — provenance
            // already records the original tier; a re-observation changes nothing.
            StructuredFactCommit::Reobserved(_) => {}
            StructuredFactCommit::NoDefinition => {}
            // The shared precedence (ADR 0086 dec. 3, `record_structured_fact`)
            // took over a lower-tier slot. A value REPLACEMENT is a real
            // disagreement, not a no-op: record WHICH metric and WHAT changed,
            // so the flagged-review surface can act on it — an upgrade counter
            // alone hides the evidence. A label-only takeover (values agreed)
            // counts as an upgrade without a drift entry.
            StructuredFactCommit::Upgraded {
                previous_value,
                previous_tier,
                ..
            } => {
                if let Some(previous) = previous_value {
                    upgrades.push(json!({
                        "metricKey": fact.metric_key,
                        "previousValue": previous,
                        "previousTier": previous_tier,
                        "newValue": value,
                    }));
                }
                upgraded += 1;
                persisted_keys.push(fact.metric_key.clone());
            }
            // A peer or manual slot holds a different value — never overwritten.
            StructuredFactCommit::Divergent { .. } => {
                deferred += 1;
            }
        }
    }

    summary.created += created;
    summary.upgraded += upgraded;
    summary.deferred += deferred;

    if !upgrades.is_empty() {
        record(
            connection,
            carrier,
            "tier_upgrade",
            "warning",
            "cover-note values replaced lower-tier stored values",
            json!({
                "periodEnd": period_end,
                "periodType": period_type,
                "upgrades": upgrades,
            }),
        )?;
    }

    record(
        connection,
        carrier,
        "emitted",
        if parsed.abstained > 0 {
            "warning"
        } else {
            "info"
        },
        "cover-note facts persisted",
        json!({
            "periodEnd": period_end,
            "periodType": period_type,
            "acceptance": acceptance.as_str(),
            "created": created,
            "upgraded": upgraded,
            "deferred": deferred,
            "abstained": parsed.abstained,
            "fxResolved": parsed.fx_resolved,
            "unitScale": parsed.unit_scale,
        }),
    )?;

    // --- Aggregator witness corroboration (ADR 0085, EspiCoverNote scope) ----
    // The witness runs where the primary tier is `Pdf` OR `EspiCoverNote`; the
    // Pdf route is wired in `run_structured_extraction`, and THIS is the
    // EspiCoverNote seam. It is CACHE-ONLY: the cover-note hook is a post-commit,
    // best-effort step inside feed ingestion, so it must never fetch synchronously
    // (ADR 0085 decision 3 politeness) nor let any witness problem affect the feed
    // — every path here is best-effort and returns `Ok`. Corroboration is
    // attempted only over the slots THIS run persisted.
    if !persisted_keys.is_empty() {
        corroborate_with_witness(
            connection,
            carrier,
            fiscal_year,
            period_type,
            &period_end,
            &parsed.facts,
            &persisted_keys,
            created + upgraded,
            summary,
        );
    }

    Ok(())
}

/// Cross-checks the cover-note facts this run persisted against the aggregator
/// witness — the EspiCoverNote half of [ADR 0085](../../../docs/adr/0085-biznesradar-fundamentals-witness.md)
/// (the Pdf half lives in `jobs::structured_extraction::run_structured_extraction`).
///
/// Reuses the SAME seam A3 built: the shared cadence cache
/// (`resolve_witness_from_cache`, which never fetches), the SAME comparison
/// primitive (`cross_check_prior` over `fact_set_for_period` sets) the pipeline
/// uses, and the SAME typed outcome vocabulary (`record_extraction_outcome`).
///
/// Semantics identical to the Pdf route: the primary (cover-note) value always
/// stays — nothing here overwrites a fact. Agreement records a corroboration
/// outcome (`accepted_via_witness`/`emitted`); disagreement records a
/// user-visible `witness_disagreement` outcome carrying the diff, values
/// unchanged. A cache MISS records nothing to the outcome table and does NOT
/// fetch — the comparison is deferred (a diagnostic "pending" is logged), never
/// resolved into a false agreement.
///
/// Best-effort by construction: any error is logged and swallowed, never
/// propagated, so feed ingestion is never affected by a witness problem.
#[allow(clippy::too_many_arguments)]
fn corroborate_with_witness(
    connection: &mut Connection,
    carrier: &CoverNoteCarrier<'_>,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    cover_facts: &[crate::fundamentals::extraction::ExtractedFact],
    persisted_keys: &[String],
    persisted_count: usize,
    summary: &mut CoverNoteExtractionSummary,
) {
    use crate::fundamentals::extraction::fact_set_for_period;
    use crate::fundamentals::extraction::pipeline::Acceptance;
    use crate::fundamentals::validation::{Outcome, Tolerance};
    use crate::jobs::structured_extraction::reason;
    use crate::source_adapters::biznesradar_fundamentals::{
        resolve_witness_from_cache, WitnessResolution,
    };

    let company_id = &carrier.item.company_id;

    // Cache-only: `None` means "no fresh cached page" → DEFER, never fetch.
    let resolution = match resolve_witness_from_cache(
        connection,
        company_id,
        fiscal_year,
        period_type,
        period_end,
    ) {
        Some(resolution) => resolution,
        None => {
            summary.witness_pending += 1;
            // A pending comparison is a diagnostic, not an outcome row: it is
            // neither agreement nor disagreement, and must never read as either.
            let _ = record(
                connection,
                carrier,
                "witness_pending",
                "info",
                "aggregator witness page not cached — corroboration deferred (no ingest-time fetch)",
                json!({ "periodEnd": period_end, "periodType": period_type }),
            );
            return;
        }
    };

    let (witness_facts, page_url) = match &resolution {
        WitnessResolution::Facts { facts, page_url } => (facts, page_url),
        // A cached no-coverage / failed page, or a page with no row for this
        // period: witness unavailable, never agreement. Logged, no outcome row.
        WitnessResolution::Unavailable(_) | WitnessResolution::Skipped => {
            log::debug!(
                "module=espi_cover_note stage=witness company={company_id} period={period_end} \
                 outcome={}",
                resolution.as_str()
            );
            return;
        }
    };

    // Compare only the slots THIS run persisted, against the witness column for
    // the same period. `cross_check_prior` checks only overlapping metrics — a
    // metric in one set but not the other is neither confirmation nor conflict.
    let persisted: std::collections::BTreeSet<&str> =
        persisted_keys.iter().map(String::as_str).collect();
    let mut primary_set = fact_set_for_period(cover_facts, period_end);
    primary_set.retain(|key, _| persisted.contains(key.as_str()));
    let witness_set = fact_set_for_period(witness_facts, period_end);

    let tol = Tolerance::default();
    // Aggregator-zero guard (ADR 0085 amendment): a witness cell of exactly 0
    // against a non-zero primary value is a scrape artifact, never a
    // disagreement — see `witness_cross_check`.
    let checks = crate::source_adapters::biznesradar_fundamentals::witness_cross_check(
        &primary_set,
        &witness_set,
        &tol,
    );
    if checks.is_empty() {
        // No overlapping metric to compare — honest absence, never agreement.
        log::debug!(
            "module=espi_cover_note stage=witness company={company_id} period={period_end} \
             outcome=no_overlap"
        );
        return;
    }

    // `cross_check_prior(primary, witness)` sets `expected = witness`, `actual =
    // primary` (ADR 0085 decision 2 convention), matching the Pdf route's diff.
    let disagreements: Vec<serde_json::Value> = checks
        .iter()
        .filter_map(|check| match &check.outcome {
            Outcome::Fail {
                expected,
                actual,
                residual,
            } => Some(json!({
                "metricKey": check.metric_key,
                "detail": {
                    "expected": expected.to_string(),
                    "actual": actual.to_string(),
                    "residual": residual.to_string(),
                },
            })),
            _ => None,
        })
        .collect();

    let (acceptance, reason_code, detail) = if disagreements.is_empty() {
        // Agreement: corroboration. The primary value is untouched — this only
        // raises confidence (ADR 0085 decision 2).
        summary.witness_agreed += 1;
        let keys: Vec<&String> = checks.iter().map(|c| &c.metric_key).collect();
        (
            Acceptance::AcceptedViaWitness,
            reason::EMITTED,
            json!({
                "witnessCorroboration": {
                    "metricKeys": keys,
                    "count": keys.len(),
                    "pageUrl": page_url,
                }
            }),
        )
    } else {
        // Disagreement: the primary (cover-note) value STAYS; the diff is raised
        // for review, identical shape to the Pdf route's `witness_disagreement`.
        summary.witness_disagreed += 1;
        (
            Acceptance::Flagged,
            reason::WITNESS_DISAGREEMENT,
            json!({
                "failedIdentities": [],
                "failedCrossChecks": [],
                "witnessDisagreements": disagreements,
            }),
        )
    };

    if let Err(error) = super::record_extraction_outcome(
        connection,
        super::NewExtractionOutcome {
            company_id,
            // The cover-note tier keys its evidence to the FEED ITEM, not a
            // document — the same ref its facts carry, so the outcome and the
            // facts point at the same komunikat.
            report_document_id: &carrier.feed_item_id,
            fiscal_year,
            period_type,
            period_end,
            tier: Some(TIER.as_str()),
            acceptance: acceptance.as_str(),
            reason_code,
            detail_json: serde_json::to_string(&detail).ok().as_deref(),
            drift_json: None,
            structure_changed: false,
            fact_count: persisted_count as i64,
        },
    ) {
        log::warn!(
            "module=espi_cover_note stage=witness_outcome company={company_id} \
             period={period_end} error={error}"
        );
    }
}

/// The immediately-prior period's end date (same fiscal period one year
/// earlier), for the comparative cross-check input.
fn prior_period_end(period_end: &str) -> Option<String> {
    let year: i64 = period_end.get(0..4)?.parse().ok()?;
    Some(format!("{:04}{}", year - 1, period_end.get(4..)?))
}

fn empty_metadata(
    parsed: &WdfParseResult,
    period_end: &str,
    period_type: &str,
) -> serde_json::Value {
    let reason = match parsed.empty_reason {
        Some(WdfEmptyReason::DeferredToAttachment) => "deferred_to_attachment",
        Some(WdfEmptyReason::HeaderOnly) => "header_only",
        None => "no_rows",
    };
    json!({
        "periodEnd": period_end,
        "periodType": period_type,
        "emptyReason": reason,
        "abstained": parsed.abstained,
    })
}

/// Records one per-item outcome as a diagnostic event, so abstentions, empty
/// reasons, and flags are inspectable rather than silent. Best-effort by
/// construction: `record_diagnostic_event` is a no-op unless developer mode is
/// on, and every call site also logs.
fn record(
    connection: &mut Connection,
    carrier: &CoverNoteCarrier<'_>,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) -> StorageResult<()> {
    log::info!(
        "module={DIAGNOSTIC_MODULE} stage={stage} feed_item={} company={} metadata={metadata}",
        carrier.feed_item_id,
        carrier.item.company_id,
    );
    record_diagnostic_event(
        connection,
        NewDiagnosticEvent {
            occurred_at: None,
            module: DIAGNOSTIC_MODULE.to_owned(),
            scope: Some(DiagnosticScope {
                scope_type: "company".to_owned(),
                id: Some(carrier.item.company_id.clone()),
            }),
            stage: stage.to_owned(),
            severity: severity.to_owned(),
            message: message.to_owned(),
            metadata: Some(metadata),
        },
    )?;
    Ok(())
}
