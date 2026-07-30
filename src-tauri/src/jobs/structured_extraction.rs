//! Structured-first fundamentals extraction service (ADR 0061 S5).
//!
//! Loads a stored report document, runs the deterministic tiered pipeline
//! (ESEF → positional xHTML → EspiCoverNote → HTML aggregator), and persists
//! the accepted facts with their provenance (source tier + validation verdict +
//! citation). The pipeline is deterministic end to end.
//!
//! The **PDF fact-extraction arm is retired** (ADR 0086 dec. 1): a real PDF
//! document spawns NO extraction attempt here — its route survives only so
//! [`derive_report_period`] can group the registry. Core KPIs for a PDF-only
//! company arrive from the BiznesRadar-primary daily pull (a separate job), not
//! from this seam. A markup/ESEF document no tier parses is an honest gap; the
//! former tier-4 OCR fallback was already retired with the in-app AI layer
//! (ADR 0084 decision 4).

use std::collections::BTreeSet;

use crate::app_state::AppState;
use crate::fundamentals::extraction::container::{detect_container, Container};
use crate::fundamentals::extraction::pipeline::{
    run_pipeline, validate_parsed_set_report, Acceptance, PipelineInput,
};
use crate::fundamentals::extraction::SourceTier;
use crate::storage::StructuredFactInput;

/// The immediately-prior period's end date for `period_end` (`YYYY-MM-DD`),
/// by decrementing the leading year — the same fiscal period one year
/// earlier. `None` when `period_end` doesn't start with a parseable year.
fn prior_period_end(period_end: &str) -> Option<String> {
    let year: i64 = period_end.get(0..4)?.parse().ok()?;
    Some(format!("{:04}{}", year - 1, period_end.get(4..)?))
}

/// The company's expected primary-KPI `metric_key`s for the completeness gate
/// (ADR 0061 dec. 4d). Delegates to the storage-owned query so this pipeline and
/// the ingest-time ESPI cover-note tier check completeness identically.
fn expected_primary_keys(
    state: &AppState,
    company_id: &str,
) -> Result<Option<BTreeSet<String>>, String> {
    state
        .financials()
        .expected_primary_metric_keys(company_id)
        .map_err(|e| e.to_string())
}

/// A re-extraction that observed an already-stored slot with a *different* value
/// than the committed one (owner T7): never silently overwritten — surfaced so
/// the divergence can be ratified. `existing` is the stored value, `incoming` the
/// freshly-extracted one.
#[derive(Debug, Clone)]
pub struct FactDivergence {
    pub fact_id: String,
    pub metric_key: String,
    pub existing: String,
    pub incoming: String,
}

/// The typed reason an extraction attempt landed where it did — never English
/// prose (ADR 0084 decision 6). These strings are the `reason_code` vocabulary
/// the `fundamentals_extraction_outcomes` CHECK constraint enforces and the
/// frontend renders through the translation layer.
pub(crate) mod reason {
    /// The run emitted facts — the non-failure value.
    pub const EMITTED: &str = "emitted";
    /// The validation gate found a contradiction (identity or comparative).
    pub const VALIDATION_FAILED: &str = "validation_failed";
    // The `structure_drift` reason (PDF profile-drift arm) is retired with the
    // PDF fact arm (ADR 0086 dec. 1): no new row carries it. Already-stored rows
    // keep the literal string; there is no producing constant anymore.
    /// The aggregator disagreed with an issuer-held value — recorded by the
    /// reversed-witnessing paths (the BR-primary pull and the WDF ingest seam),
    /// never by this pipeline (ADR 0086 dec. 4).
    pub const WITNESS_DISAGREEMENT: &str = "witness_disagreement";
    // The `witness_fallback` reason (ADR 0085 aggregator gap-fill) is retired
    // with ADR 0086: BiznesRadar sources core KPIs through its own primary
    // pull. Already-stored rows keep the literal string; readers stay tolerant.
    /// No deterministic tier could read the document (post-ADR-0084 there is no
    /// AI fallback: this is an honest, explicit gap, never a guess).
    pub const NO_DETERMINISTIC_TIER: &str = "no_deterministic_tier";
    /// The document's stored file is missing or unreadable.
    pub const DOCUMENT_UNREADABLE: &str = "document_unreadable";
}

/// The typed reason + structured detail for one pipeline verdict.
///
/// `Flagged` is deliberately disambiguated: "the numbers contradict each other"
/// (`validation_failed`) and "the witness disagrees" (`witness_disagreement`)
/// are different problems with different fixes, and a single `flagged` label
/// would hide that. (The `structure_drift` reason is retired with the PDF fact
/// arm, ADR 0086 dec. 1.)
fn reason_for(
    outcome: &crate::fundamentals::extraction::pipeline::PipelineOutcome,
) -> &'static str {
    if outcome.acceptance.emits() {
        return reason::EMITTED;
    }
    match outcome.acceptance {
        Acceptance::Empty => reason::NO_DETERMINISTIC_TIER,
        // A non-emitting non-empty outcome is a failed gate. (`structure_drift`
        // died with the PDF fact arm and `witness_disagreement` is recorded by
        // the aggregator pull's reversed witnessing, never by this pipeline —
        // ADR 0086 dec. 1/4.)
        _ => reason::VALIDATION_FAILED,
    }
}

/// The failing-check detail behind a verdict, as JSON — which identities and
/// which comparative cross-checks objected, with their expected/actual/residual.
///
/// Only *failures* are recorded: a `NotApplicable` check (inputs absent) is not
/// a contradiction and listing it would bury the signal. `None` when nothing
/// failed, so a detail payload always means "here is what objected".
fn failing_check_detail(
    outcome: &crate::fundamentals::extraction::pipeline::PipelineOutcome,
) -> Option<String> {
    use crate::fundamentals::validation::Outcome;

    let describe = |o: &Outcome| match o {
        Outcome::Fail {
            expected,
            actual,
            residual,
        } => Some(serde_json::json!({
            "expected": expected.to_string(),
            "actual": actual.to_string(),
            "residual": residual.to_string(),
        })),
        _ => None,
    };

    let report = outcome.validation.as_ref()?;

    let identities: Vec<serde_json::Value> = report
        .identities
        .iter()
        .filter_map(|check| {
            describe(&check.outcome).map(|detail| {
                serde_json::json!({ "id": check.id, "label": check.label, "detail": detail })
            })
        })
        .collect();
    let cross_checks: Vec<serde_json::Value> = report
        .cross_checks
        .iter()
        .filter_map(|check| {
            describe(&check.outcome).map(
                |detail| serde_json::json!({ "metricKey": check.metric_key, "detail": detail }),
            )
        })
        .collect();

    if identities.is_empty() && cross_checks.is_empty() {
        return None;
    }
    serde_json::to_string(&serde_json::json!({
        "failedIdentities": identities,
        "failedCrossChecks": cross_checks,
    }))
    .ok()
}

/// Persists one attempt's outcome. Best-effort by design: the extraction result
/// the caller already holds is the more important guarantee, so a bookkeeping
/// failure is logged, never propagated — the same policy the ingest-time
/// cover-note tier applies to its own recording.
#[allow(clippy::too_many_arguments)]
fn record_outcome(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    tier: Option<&str>,
    acceptance: Acceptance,
    reason_code: &str,
    detail_json: Option<&str>,
    drift_json: Option<&str>,
    fact_count: i64,
) {
    // Always-on structured log line (the cover-note tier's precedent): the
    // durable row is the record, this is the trail even before anyone looks.
    log::info!(
        "module=structured_extraction stage=outcome company={company_id} document={report_document_id} \
         period={period_end} acceptance={} reason={reason_code} tier={} facts={fact_count}",
        acceptance.as_str(),
        tier.unwrap_or("none"),
    );
    if let Err(error) = state.fundamentals_provenance().record_extraction_outcome(
        crate::storage::NewExtractionOutcome {
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
            tier,
            acceptance: acceptance.as_str(),
            reason_code,
            detail_json,
            drift_json,
            structure_changed: drift_json.is_some(),
            fact_count,
        },
    ) {
        log::warn!(
            "module=structured_extraction stage=outcome_record_failed \
             company={company_id} document={report_document_id} error={error}"
        );
    }
}

/// The `fact_count` an outcome row records: the facts this run ESTABLISHED at
/// the slot — newly produced **plus** re-observed.
///
/// Recording only the produced count was the zero-effect-success defect epic #40
/// S5 hunts (ADR 0091): the outcome row upserts in place, so re-running a landed
/// period (every slot re-observed, nothing new) overwrote a healthy
/// `fact_count = 12` with `0` while keeping `reason_code = "emitted"`. The row
/// then claimed an emission it could not evidence, and the #155 report-documents
/// indicator rendered `has_data` with `0 facts` — a success that produced
/// nothing and could not say why. Counting re-observations makes the row state
/// what is actually AT the slot, which is also what that indicator asks.
fn slot_fact_count(produced_fact_ids: &[String], skipped_fact_ids: &[String]) -> i64 {
    (produced_fact_ids.len() + skipped_fact_ids.len()) as i64
}

/// Record a `tier_upgrade` diagnostic when an issuer tier OVERWROTE a lower-tier
/// slot's VALUE (ADR 0086 dec. 3) — the same upgrade evidence the WDF cover-note
/// seam records ([`crate::storage::espi_cover_note_facts`]). A label-only upgrade
/// (`previous_value` is `None`: the tiers agreed and only the label/evidence
/// moved) records nothing. Best-effort + developer-mode gated (the diagnostic
/// sink is a no-op otherwise) — never fails the extraction.
fn record_tier_upgrade_diagnostic(
    state: &AppState,
    company_id: &str,
    metric_key: &str,
    previous_value: Option<&str>,
    previous_tier: &str,
    new_value: &str,
) {
    let Some(previous_value) = previous_value else {
        return;
    };
    log::info!(
        "module=structured_extraction stage=tier_upgrade company={company_id} \
         metric={metric_key} previous_tier={previous_tier} previous={previous_value} new={new_value}"
    );
    let _ = state.record_diagnostic_event(crate::storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "structured_extraction".to_owned(),
        scope: Some(crate::storage::DiagnosticScope {
            scope_type: "company".to_owned(),
            id: Some(company_id.to_owned()),
        }),
        stage: "tier_upgrade".to_owned(),
        severity: "warning".to_owned(),
        message: "issuer tier overwrote a lower-tier stored value".to_owned(),
        metadata: Some(serde_json::json!({
            "metricKey": metric_key,
            "previousValue": previous_value,
            "previousTier": previous_tier,
            "newValue": new_value,
        })),
    });
}

/// The outcome of a structured extraction attempt.
#[derive(Debug, Clone)]
pub struct StructuredExtractionResult {
    pub acceptance: Acceptance,
    /// Which tier produced the accepted (or attempted) facts.
    pub tier: Option<SourceTier>,
    /// Ids of the `financial_facts` this run created (genuinely new values).
    pub produced_fact_ids: Vec<String>,
    /// Ids of facts already present at their slot (re-observations — same value,
    /// or a divergence). A re-extraction of a landed period skips, never dupes.
    pub skipped_fact_ids: Vec<String>,
    /// Slots re-observed with a value that disagrees with the stored fact.
    pub divergences: Vec<FactDivergence>,
    /// Whether a deterministic **issuer** tier emitted facts this run.
    /// (The ADR 0085 aggregator-fallback flag is retired with ADR 0086 —
    /// BiznesRadar sources facts through its own primary pull; stored
    /// `witness_fallback` outcome rows remain readable as legacy.)
    pub emitted: bool,
    /// The typed `reason_code` this run recorded on its outcome row (the
    /// `reason` vocabulary; issue #244) — `None` only for the benign PDF route,
    /// which records no outcome (ADR 0086 dec. 1).
    pub reason_code: Option<&'static str>,
}

/// The per-fact `confirmation_state` for an accepted outcome. Facts are
/// review-free (ADR 0086 dec. 5, amending ADR 0061 dec. 3/8/9): every accepted
/// set lands `confirmed` in **both** modes — there is no `pending`/`auto_unreviewed`
/// awaiting-confirmation grace period. Whether the "good gate" proved a value
/// (`Accepted`/`AcceptedViaWitness`) or it was merely uncontradicted
/// (`AcceptedUnreviewed`) is provenance, not a review to-do: it lives in the
/// fact's `validation_status` + `source_tier` + citation, surfaced as labels.
/// `confirmation_state` is a frozen compatibility column. `mode` no longer
/// affects it — the parameter stays so callers pass it uniformly. `Flagged`/
/// `Empty` never reach here (`Acceptance::emits()` is `false`).
fn confirmation_state_for(_acceptance: Acceptance, _mode: &str) -> &'static str {
    "confirmed"
}

/// One fact held back by the runtime history-plausibility gate — its magnitude
/// was ≥100× off its own stored history (a dropped `w tys.` multiplier or a note
/// reference read as the value). Carries the figures the flagged-outcome detail
/// cites so a reviewer sees *why* it was quarantined.
struct QuarantinedFact {
    metric_key: String,
    value: rust_decimal::Decimal,
    history_median: rust_decimal::Decimal,
}

/// The `validation_failed` detail for a set with one or more facts quarantined by
/// the history-plausibility gate: each quarantined metric, its rejected value and
/// the history median it defied, folded onto any identity/cross-check failures
/// the same set already carried (so a set that both self-contradicted AND had a
/// scale outlier records both). Object-merged, never nested, so the review
/// surface reads one flat payload.
fn quarantine_detail(quarantined: &[QuarantinedFact], base: Option<String>) -> Option<String> {
    let facts: Vec<serde_json::Value> = quarantined
        .iter()
        .map(|q| {
            serde_json::json!({
                "metricKey": q.metric_key,
                "value": q.value.to_string(),
                "historyMedian": q.history_median.to_string(),
            })
        })
        .collect();
    let mut payload = base
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    payload.insert(
        "quarantinedFacts".to_owned(),
        serde_json::Value::Array(facts),
    );
    serde_json::to_string(&serde_json::Value::Object(payload)).ok()
}

/// The derivation-grammar version stamped on every persisted period (migration
/// 0109). A document's bytes are immutable once ingested, so the ONLY reason to
/// re-derive a cached period is a change to the derivation grammar itself
/// ([`derive_report_period_uncached`] — the ESEF/title/cover-page rules). Bump
/// this when that grammar changes: any cached row stamped with an older version
/// is re-derived and overwritten on next read (self-healing).
pub const DERIVATION_VERSION: i64 = 1;

/// The extraction-pipeline capability version stamped into an autopilot run's
/// `kpi_delta_json` on the `extractionAvailable:false` path. A document's bytes
/// are immutable, so a couldn't-extract verdict only becomes stale when the
/// pipeline itself gains the ability to read a document it previously could not
/// — a new/changed tier, parser, or derivation capability. This is the single
/// knob that makes flagged periods retry: [`crate::jobs::autopilot`] re-arms a
/// terminal couldn't-extract run exactly when the running build's version is
/// newer than the one the run recorded, so each capability upgrade retries a
/// flagged period **once**, then it settles (dedup) until the next bump.
///
/// Bump this whenever a tier/parser/derivation change alters what documents can
/// be read. **Do not** bump for changes that cannot affect readability.
///
/// `2` is the first stamped era; `1` is the implicit pre-versioning era, so a
/// legacy stored run (delta with no `pipelineVersion`, read as `0`) re-arms
/// once under this build and then settles. `3` versions the ADR 0086 dec. 1
/// ladder change (PDF fact arm retired) — it re-arms nothing PDF-wise, since a
/// PDF document no longer arms any extraction attempt.
pub const EXTRACTION_PIPELINE_VERSION: u32 = 3;

/// Re-intern a cached `period_type` string back to the `&'static str` the
/// derivation returns. [`crate::report_diff::classify`] only ever yields these
/// four labels (`to_period`), plus ESEF's `FY`. An unrecognised cached label
/// (never written by the current code) yields `None`, forcing a safe re-derive.
fn intern_period_type(period_type: &str) -> Option<&'static str> {
    Some(match period_type {
        "Q1" => "Q1",
        "H1" => "H1",
        "Q3" => "Q3",
        "FY" => "FY",
        _ => return None,
    })
}

/// Derives the reporting period `(fiscal_year, period_type, period_end)` for a
/// stored report document, reading a persisted derivation (migration 0109) when
/// one exists so the file read + text extraction the cover-page tier costs is
/// paid at most once per document — the Coverage panel and every re-extraction
/// then read the index instead of recomputing the corpus (CLAUDE.md). Cache
/// misses (and stale-version rows) fall through to [`derive_report_period_uncached`]
/// and persist its result — a period OR the explicit none-marker, so an
/// abstention is not re-parsed either. A not-yet-ingested document (no stored
/// file) is never cached: its `None` is transient, not a property of its bytes.
pub fn derive_report_period(
    state: &AppState,
    document: &crate::storage::ReportDocument,
) -> Option<(i64, &'static str, String)> {
    // Read the persisted derivation first — a fresh-enough hit avoids ALL file IO.
    if let Ok(Some(cached)) = state.financials().cached_derived_period(&document.id) {
        if cached.derivation_version >= DERIVATION_VERSION {
            if !cached.has_period {
                return None; // persisted none-marker: never re-parse
            }
            if let (Some(fiscal_year), Some(period_type), Some(period_end)) = (
                cached.fiscal_year,
                cached.period_type.as_deref().and_then(intern_period_type),
                cached.period_end.clone(),
            ) {
                return Some((fiscal_year, period_type, period_end));
            }
            // Unexpected shape (e.g. an unknown interned label) — fall through and
            // re-derive rather than trust a row this code could not have written.
        }
        // Older version → re-derive and overwrite below.
    }

    let derived = derive_report_period_uncached(state, document);

    // Persist ONLY once the document is ingested (fetched, with a stored file):
    // its bytes — hence its derived period — are then stable. Caching a
    // pre-fetch `None` would poison a document into never being re-derived after
    // it is fetched.
    if document.fetch_status == "fetched" && document.local_path.is_some() {
        let _ = state.financials().store_derived_period(
            &document.id,
            derived.as_ref().map(|(fy, pt, pe)| (*fy, *pt, pe.as_str())),
            DERIVATION_VERSION,
        );
    }

    derived
}

/// The uncached derivation — the SAME grammar the autopilot pipeline uses (ADR
/// 0061 dec. 3/8/9), the single source of truth shared by the autopilot stage
/// and the on-demand "Extract data" command. `None` when the document has no
/// stored file, or its period can't be classified from the iXBRL contexts NOR
/// its title/URL NOR its cover page.
///
/// - **Xhtml/ESEF**: the period is self-derived from the iXBRL contexts (ESEF is
///   an annual filing → `FY` at the latest context date). A file on the ESEF
///   route that is NOT valid iXBRL (an interim XHTML with no `ix:` tags — a
///   pdf2htmlEX render) falls through to the title/URL derivation below (T-A1).
/// - **Pdf**: the period is derived from the document's title/URL via
///   [`crate::report_diff::classify::period_sort_key`], assuming a calendar
///   fiscal year (index 1→`Q1`/`-03-31`, 2→`H1`/`-06-30`, 3→`Q3`/`-09-30`,
///   4→`FY`/`-12-31`). An unparseable or ambiguous intra-year period (index `0`)
///   is not guessed.
fn derive_report_period_uncached(
    state: &AppState,
    document: &crate::storage::ReportDocument,
) -> Option<(i64, &'static str, String)> {
    use crate::fundamentals::extraction::{esef::parse_esef, primary_period_end};
    use crate::report_diff::classify::period_from_title_url;

    let local_path = document.local_path.as_deref()?;
    // ESEF tier — a markup instance OR a ZIP report package (ADR 0061 dec. 1),
    // decided from the sniffed container (epic #229 T2). Read the file only for
    // the formats that self-derive their period from the iXBRL contexts; a PDF
    // derives it from its title/URL without a read.
    if is_esef_route(document) {
        // ESEF self-derives its period from the iXBRL contexts. A file that is the
        // ESEF route by extension/content-type but is NOT valid iXBRL — an interim
        // XHTML with no `ix:` tags (a pdf2htmlEX render) — yields no period here;
        // fall THROUGH to the title/URL derivation below rather than returning
        // None (T-A1), the same fallback the coverage read model already applies.
        let esef_period = (|| {
            let raw = std::fs::read(state.data_dir().join(local_path)).ok()?;
            let instance = esef_instance_bytes(&raw)?;
            let facts = parse_esef(&instance).ok()?;
            let period_end = primary_period_end(&facts)?;
            let fiscal_year = period_end.get(0..4).and_then(|y| y.parse::<i64>().ok())?;
            Some((fiscal_year, "FY", period_end))
        })();
        if let Some(period) = esef_period {
            return Some(period);
        }
    }

    // PDF (or a non-iXBRL XHTML that could not self-derive): period from the
    // document's title/URL — the SAME derivation the ingest-time cover-note tier
    // uses, so the two can never drift.
    let title = document.title.as_deref().unwrap_or("");
    if let Some(period) = period_from_title_url(title, &document.url) {
        return Some(period);
    }

    // Last resort: the document's own cover page. A Polish periodic report states
    // its reporting period in the title block ("za okres 6 miesięcy zakończony
    // 30.06.2025"), and on the maintainer's database a run of real statements is
    // stored under a bare `SSF.pdf` whose title/URL name nothing (card fc692da).
    // Same grammar as above — one derivation, two carriers — so a form added for
    // titles works here too, and an unstated period still abstains.
    period_from_cover_page(state, document)
}

/// How much of a document's text counts as its cover page. The title block is at
/// the very top; reading further would drag in comparative-column dates and the
/// notes, where a period reference no longer describes *this* report.
const COVER_PAGE_CHARS: usize = 1_500;

fn period_from_cover_page(
    state: &AppState,
    document: &crate::storage::ReportDocument,
) -> Option<(i64, &'static str, String)> {
    use crate::report_diff::classify::period_from_text;
    use crate::report_diff::extraction::{extract_report, ExtractionState};

    // Only for a periodic statement. Reading a period out of a document costs a
    // full text extraction, and the documents that legitimately have no period —
    // governance filings, auditor work products, announcements — are the large
    // majority of the corpus (~3 000 of 3 790 stored files on the maintainer's
    // database). Spending an extraction on them to confirm an expected `None`
    // would make every sweep an overnight run for no coverage.
    if !matches!(
        crate::fundamentals::extraction::classify::classify_doc_kind(
            document.title.as_deref().unwrap_or(""),
            &document.url,
        ),
        crate::fundamentals::extraction::classify::DocKind::PeriodicSsf
            | crate::fundamentals::extraction::classify::DocKind::PeriodicJsf
    ) {
        return None;
    }
    let local_path = document.local_path.as_deref()?;
    // Container truth (epic #229 T2): a `.pdf` holding markup is read as markup,
    // where the PDF reader used to return nothing. A ZIP package or an
    // unrecognised container yields no cover text at all — the ESEF route above
    // already self-derives a package's period from its iXBRL contexts — so it
    // stays a measured `no_period_derived` gap rather than a garbage parse.
    let format = crate::report_documents_container::resolved_source_format(document)?;
    let bytes = std::fs::read(state.data_dir().join(local_path)).ok()?;
    let outcome = extract_report(&bytes, format);
    if outcome.state != ExtractionState::Extracted {
        // A scanned or unreadable document yields no text to read a period from;
        // it stays a measured `no_period_derived` gap rather than a guess.
        return None;
    }
    let mut cover = String::new();
    let mut taken = 0usize;
    'sections: for section in &outcome.sections {
        for part in [section.heading.as_str(), section.body.as_str()] {
            for ch in part.chars() {
                if taken >= COVER_PAGE_CHARS {
                    break 'sections;
                }
                cover.push(ch);
                taken += 1;
            }
            cover.push('\n');
        }
    }
    period_from_text(&cover)
}

/// Whether a stored document should be resolved through the ESEF/iXBRL tier
/// rather than the PDF tier: a markup instance, or an ESEF report *package* (a
/// ZIP, ADR 0061 dec. 1).
///
/// Decided from the stored `detected_container` (epic #229 T2) with the old
/// extension/content-type rule as the fallback for a never-sniffed row — still
/// **no byte read**, so callers can decide whether to load the file at all. This
/// is what catches the corpus's mislabeled packages (generic
/// `application/octet-stream` under a `.pdf` name) up front instead of relying on
/// the later ZIP-magic sniff in [`esef_instance_bytes`].
pub(crate) fn is_esef_route(document: &crate::storage::ReportDocument) -> bool {
    use crate::report_documents_container::{is_markup, is_package};

    is_markup(document) || is_package(document)
}

/// A fetched periodic (ssf/jsf) document with a stored file — the extractability
/// gate shared by the ownership + management-holdings extraction jobs.
pub(crate) fn is_fetched_periodic(document: &crate::storage::ReportDocument) -> bool {
    document.fetch_status == "fetched"
        && document.local_path.is_some()
        && matches!(
            document.doc_kind.as_deref(),
            Some("periodic_ssf") | Some("periodic_jsf")
        )
}

/// Find a fetched **PDF** sibling of an xhtml/html residual document: a periodic
/// PDF of the SAME company and SAME derived report period. `None` when the
/// document is not xhtml/html, has no derivable period, or no matching PDF
/// exists. Shared by the management-holdings glyph path (T5) and the ownership
/// OCR path (T8) so the sibling rule lives in one place — a pdf2htmlEX container
/// (unreadable text layer) is exactly why such documents are residual, and their
/// real content is in the companion PDF.
pub(crate) fn find_pdf_sibling(
    state: &AppState,
    document: &crate::storage::ReportDocument,
) -> Option<crate::storage::ReportDocument> {
    use crate::report_documents_container::{is_markup, is_real_pdf};

    // Container truth on both ends (epic #229 T2): the residual must really be
    // markup, and the sibling must really be a PDF — a ZIP package under a `.pdf`
    // name has no text layer to fall back to, so choosing it would swap one
    // unreadable document for another.
    if !is_markup(document) {
        return None;
    }
    let target_period = derive_report_period(state, document).map(|(_, _, end)| end)?;
    let siblings = state
        .list_report_documents_by_company(&document.company_id)
        .ok()?;
    siblings.into_iter().find(|sibling| {
        sibling.id != document.id
            && is_fetched_periodic(sibling)
            && is_real_pdf(sibling)
            && derive_report_period(state, sibling).map(|(_, _, end)| end)
                == Some(target_period.clone())
    })
}

/// The inline-XBRL **instance** bytes for a stored document, if it is (or
/// contains) one — the single seam shared by [`derive_report_period`] and
/// [`run_structured_extraction`] so the on-demand button and autopilot resolve
/// the ESEF tier identically (ADR 0061 dec. 1). Markup returns its own bytes; an
/// ESEF report package (a ZIP) is unpacked to its inner `reports/` instance.
/// `None` for a PDF, a package with no readable instance, or an unrecognised
/// container.
///
/// The bytes are already in hand here, so this decides from
/// [`detect_container`] directly — the same sniffer that fills the stored
/// `detected_container` column (epic #229 T2), never the filename.
fn esef_instance_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    use crate::fundamentals::extraction::esef_package;
    match detect_container(bytes) {
        Container::Zip => esef_package::extract_instance(bytes),
        Container::Xml | Container::Html => Some(bytes.to_vec()),
        Container::Pdf | Container::Unknown => None,
    }
}

/// How a stored document should be parsed, decided from its **magic bytes** and
/// not its filename (card `eb71488`). A measured 4.4% of the maintainer's stored
/// `.pdf` documents are XML/ZIP/HTML under a `.pdf` name; trusting the extension
/// hands those to the PDF reader, where they fail 100%. The container is ground
/// truth — the extension and content-type are hints the corpus disproves (every
/// mislabeled file was `application/octet-stream`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocumentRoute {
    /// A real PDF → the text/PDF tier (unchanged from before this card).
    Pdf,
    /// Markup that is inline-XBRL → the ESEF tier reads it as its own instance.
    IxbrlInstance,
    /// Bare markup that is NOT inline-XBRL (a pdf2htmlEX render, an HTML export)
    /// → the deterministic positional parser (ADR 0077 T-B2).
    Positional,
    /// A ZIP → an ESEF/eSprawozdanie report package; the inner instance is
    /// unpacked before the ESEF tier reads it.
    ZipPackage,
    /// A container the pipeline cannot act on → an honest `document_unreadable`
    /// outcome naming what was detected, never a PDF-reader failure or a crash.
    Unsupported(Container),
}

/// Route a document from its bytes alone — pure and unit-testable, so the
/// routing decision is provable without an [`AppState`]. The `%PDF`/ZIP magic
/// and the markup preamble are read by [`detect_container`]; the iXBRL-vs-
/// positional split reuses [`crate::fundamentals::extraction::esef::is_inline_xbrl`]
/// (the SAME sniff the history sweep and the T-B2 router already share), so
/// routing and extractability can never disagree.
pub(crate) fn route_document(bytes: &[u8]) -> DocumentRoute {
    match detect_container(bytes) {
        Container::Pdf => DocumentRoute::Pdf,
        Container::Zip => DocumentRoute::ZipPackage,
        Container::Xml | Container::Html => {
            let prefix = &bytes[..bytes.len().min(64 * 1024)];
            if crate::fundamentals::extraction::esef::is_inline_xbrl(prefix) {
                DocumentRoute::IxbrlInstance
            } else {
                DocumentRoute::Positional
            }
        }
        unsupported @ Container::Unknown => DocumentRoute::Unsupported(unsupported),
    }
}

/// Runs the structured pipeline for one report document and persists the
/// result. `mode` is the run's mode (`MODE_AUTOPILOT` / `MODE_ASSIST`); it no
/// longer affects the per-fact `confirmation_state` — facts are review-free
/// (ADR 0086 dec. 5), so [`confirmation_state_for`] stamps `confirmed` for every
/// accepted set regardless of mode or caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_structured_extraction(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    mode: &str,
) -> Result<StructuredExtractionResult, String> {
    // --- Load the document bytes + format -------------------------------
    let document = state
        .get_report_document(report_document_id)
        .map_err(|e| e.to_string())?;
    // A document we cannot even open is an outcome, not a silence: record the
    // typed reason before propagating, so the period is visibly attempted-and-
    // unreadable rather than indistinguishable from never attempted.
    let unreadable = |error: String| -> String {
        record_outcome(
            state,
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
            None,
            Acceptance::Empty,
            reason::DOCUMENT_UNREADABLE,
            serde_json::to_string(&serde_json::json!({ "error": error }))
                .ok()
                .as_deref(),
            None,
            0,
        );
        error
    };
    // A container the pipeline cannot parse is the same kind of honest gap as an
    // unreadable file — but recorded with the DETECTED container named, so the
    // review surface shows "this .pdf is actually a <zip/xml/html>" rather than a
    // mute PDF-reader failure (card `eb71488`).
    let unsupported_container = |container: Container, reason_text: &str| -> String {
        let detail = serde_json::json!({
            "detectedContainer": container.as_str(),
            "reason": reason_text,
        });
        record_outcome(
            state,
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
            None,
            Acceptance::Empty,
            reason::DOCUMENT_UNREADABLE,
            serde_json::to_string(&detail).ok().as_deref(),
            None,
            0,
        );
        format!(
            "unsupported container: {} ({reason_text})",
            container.as_str()
        )
    };
    let Some(local_path) = document.local_path else {
        return Err(unreadable(
            "the report document has no stored file".to_owned(),
        ));
    };
    let path = state.data_dir().join(&local_path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return Err(unreadable(format!("failed to read report file: {error}"))),
    };

    // --- Route on container MAGIC BYTES, not the filename (card eb71488) -----
    // The stored `.pdf` name is not trusted: 4.4% of the maintainer's stored
    // "PDF" documents are XML/ZIP/HTML under a `.pdf` name and fail 100% when
    // handed to the PDF reader (19 of 430 periodic filings, 7 companies). The
    // bytes decide; several of those XMLs carry real financial data, so correct
    // routing ADDS coverage via the ESEF/iXBRL/positional tiers. An unsupported
    // container is an explicit outcome row, never an error that aborts the sweep.
    let esef_opt: Option<Vec<u8>> = match route_document(&bytes) {
        // Bare markup that is NOT iXBRL — a pdf2htmlEX visual render or an HTML
        // export. The deterministic positional parser (ADR 0077 T-B2) persists
        // its own result, so this arm early-returns just as before.
        DocumentRoute::Positional => {
            return run_positional_extraction(
                state,
                company_id,
                report_document_id,
                fiscal_year,
                period_type,
                period_end,
                mode,
                &bytes,
            );
        }
        // Markup that IS inline-XBRL → the ESEF tier reads it as its own instance
        // (a bare `.xhtml` instance stored under any name).
        DocumentRoute::IxbrlInstance => Some(bytes.clone()),
        // A ZIP report package (ESEF/eSprawozdanie): unpack the inner iXBRL
        // instance (ADR 0061 dec. 1). A ZIP with no readable instance is
        // unreadable-by-container, recorded as such — never fed to the PDF reader.
        DocumentRoute::ZipPackage => {
            match crate::fundamentals::extraction::esef_package::extract_instance(&bytes) {
                Some(instance) => Some(instance),
                None => {
                    return Err(unsupported_container(
                        Container::Zip,
                        "the ZIP report package holds no readable iXBRL instance",
                    ));
                }
            }
        }
        // A real PDF → NO extraction attempt (ADR 0086 dec. 1: the PDF fact arm is
        // retired). The route survives so the registry/period derivation still
        // group this document, but no tier reads financial facts out of it — core
        // KPIs for a PDF-only company arrive from the BiznesRadar-primary daily
        // pull. Returns a benign empty result and records NO outcome, so a PDF
        // never generates a `no_deterministic_tier` extraction-outcome row.
        DocumentRoute::Pdf => {
            return Ok(StructuredExtractionResult {
                acceptance: Acceptance::Empty,
                tier: None,
                produced_fact_ids: Vec::new(),
                skipped_fact_ids: Vec::new(),
                divergences: Vec::new(),
                emitted: false,
                reason_code: None,
            });
        }
        // Not a container the pipeline can act on (e.g. garbage bytes under a
        // `.pdf` name): an honest, explicit gap naming what was detected.
        DocumentRoute::Unsupported(container) => {
            return Err(unsupported_container(
                container,
                "the stored file's bytes are not a PDF, ZIP, XML, or HTML container",
            ));
        }
    };

    // --- Comparative cross-check + completeness inputs (ADR 0061 dec. 4b/4d) --
    let prior_end = prior_period_end(period_end);
    // Veto-capable priors only (ADR 0086 dec. 3/4): a lower-tier (aggregator)
    // stored prior must not fail the issuer filing's comparative cross-check.
    let stored_prior = state
        .financials()
        .stored_fact_set_for_cross_check(company_id, fiscal_year - 1, period_type, SourceTier::Esef)
        .map_err(|e| e.to_string())?;
    let expected_keys = expected_primary_keys(state, company_id)?;

    // The ADR 0085 pipeline witness seam is retired with ADR 0086: only ESEF
    // routes reach this point (positional and PDF routes early-return above),
    // and issuer-tagged ESEF was always out of witness scope. BiznesRadar
    // corroboration now runs reversed — the aggregator's own primary pull
    // (`jobs::aggregator_fundamentals_pull`) records `witness_disagreement`
    // against issuer-held slots.
    let input = PipelineInput {
        period_end,
        esef_bytes: esef_opt.as_deref(),
        prior: stored_prior.as_ref(),
        prior_period_end: prior_end.as_deref(),
        expected_keys: expected_keys.as_ref(),
    };
    let mut outcome = run_pipeline(&input);

    // ADR 0085 amendment (2026-07-21) condition 2: an aggregator-SOURCED set is
    // never auto-confirmed. The pipeline's aggregator arm can return `Accepted`
    // for a clean aggregator set, which would make `confirmation_state_for`
    // commit it as `confirmed` in autopilot — claiming issuer-grade trust for a
    // third-party number. Downgrading the acceptance here fixes the whole ladder
    // at once (confirmation state, `validation_status`, and the recorded
    // acceptance), rather than special-casing three call sites that could drift
    // apart. It never *blocks* the fallback — it only refuses to over-trust it.
    if outcome.tier == Some(SourceTier::HtmlAggregator)
        && outcome.acceptance == Acceptance::Accepted
    {
        outcome.acceptance = Acceptance::AcceptedUnreviewed;
    }

    // The PDF profile-drift arm is retired (ADR 0086 dec. 1), so the pipeline no
    // longer produces a drift diff. The provenance `drift_json` column + the
    // extraction-outcome `drift_json` param are kept (append-only) and simply
    // carry `None` — the dead result/summary plumbing is gone (F7).
    let drift_json: Option<String> = None;

    // --- Persist accepted facts + provenance ----------------------------
    let mut produced_fact_ids = Vec::new();
    let mut skipped_fact_ids = Vec::new();
    let mut divergences = Vec::new();
    // Facts held back by the history-plausibility gate (ADR 0061 magnitude guard,
    // card 22ac70c). Per-FACT: the outlier is quarantined while its plausible
    // siblings still emit; any quarantine downgrades the SET's recorded acceptance
    // to Flagged so the period surfaces for review. Migration 0108 runs before any
    // extraction, so the medians this reads are already scale-cleaned on the
    // maintainer's machine — the check never modifies anything stored.
    let mut quarantined: Vec<QuarantinedFact> = Vec::new();
    if outcome.acceptance.emits() {
        let validation_status = outcome.acceptance.validation_status();
        let tier = outcome.tier.map(|t| t.as_str()).unwrap_or("unknown");
        let confirmation_state = confirmation_state_for(outcome.acceptance, mode);
        let store = state.kpi_extraction();
        // Within-batch dedup: the structured commit ignores per-fact `basis` (all
        // facts land on the default slot), so two facts sharing a `metric_key`
        // collapse to one slot. Keep the FIRST occurrence deterministically — a
        // later same-key fact would otherwise re-observe the row this same run
        // just wrote and be mis-counted as a skip.
        // History-plausibility quarantine input, read ONCE for the whole set: a
        // metric's own stored history (excluding this very period) is invariant
        // across the facts of this run, so batch it rather than re-reading the
        // company's periods per fact (the batched read is bit-identical to N
        // single `metric_history` calls — see `metric_histories`).
        let history_keys: BTreeSet<String> =
            outcome.facts.iter().map(|f| f.metric_key.clone()).collect();
        let histories = state
            .financials()
            .metric_histories(company_id, &history_keys, fiscal_year, period_type)
            .map_err(|e| e.to_string())?;
        let mut seen_keys = BTreeSet::new();
        for fact in &outcome.facts {
            if !seen_keys.insert(fact.metric_key.clone()) {
                continue;
            }
            // History-plausibility quarantine: a magnitude ≥100× off this metric's
            // own stored history (excluding this very period) is a uniform-scale
            // error no same-period identity or comparative column can see. Hold the
            // outlier back — never persist it — and record it so the set flags; its
            // plausible siblings continue to emit below. History INCLUDES confirmed
            // values (the trust anchor); the gate abstains with <2 history periods.
            let empty_history = Vec::new();
            let history = histories.get(&fact.metric_key).unwrap_or(&empty_history);
            if crate::fundamentals::validation::implausible_against_history(
                &fact.metric_key,
                fact.value,
                history,
            ) {
                if let Some(history_median) =
                    crate::fundamentals::validation::history_median(history)
                {
                    quarantined.push(QuarantinedFact {
                        metric_key: fact.metric_key.clone(),
                        value: fact.value,
                        history_median,
                    });
                }
                continue;
            }
            let value = fact.value.to_string();
            let commit = store
                .record_structured_fact(StructuredFactInput {
                    company_id,
                    fiscal_year,
                    period_type,
                    period_end: Some(period_end),
                    report_document_id,
                    metric_key: &fact.metric_key,
                    value_numeric: &value,
                    currency: fact.currency.as_deref(),
                    confirmation_state,
                    source_tier: tier,
                    extraction_method: "api",
                    validation_status,
                    drift_json: drift_json.as_deref(),
                    citation: Some(&fact.citation),
                })
                .map_err(|e| e.to_string())?;
            match commit {
                crate::storage::StructuredFactCommit::Created(id) => produced_fact_ids.push(id),
                // A higher tier took over a lower-tier slot (ADR 0086 dec. 3):
                // the fact now carries this run's value/evidence — an emit. A VALUE
                // overwrite (previous_value Some) is a real disagreement — leave the
                // upgrade evidence as a diagnostic (F6), mirroring the WDF seam.
                crate::storage::StructuredFactCommit::Upgraded {
                    fact_id,
                    previous_value,
                    previous_tier,
                } => {
                    record_tier_upgrade_diagnostic(
                        state,
                        company_id,
                        &fact.metric_key,
                        previous_value.as_deref(),
                        &previous_tier,
                        &value,
                    );
                    produced_fact_ids.push(fact_id);
                }
                crate::storage::StructuredFactCommit::Reobserved(id) => skipped_fact_ids.push(id),
                crate::storage::StructuredFactCommit::Divergent {
                    fact_id,
                    metric_key,
                    existing,
                    incoming,
                } => {
                    skipped_fact_ids.push(fact_id.clone());
                    divergences.push(FactDivergence {
                        fact_id,
                        metric_key,
                        existing,
                        incoming,
                    });
                }
                // Non-catalog key — the pipeline should not emit it; not counted.
                crate::storage::StructuredFactCommit::NoDefinition => {}
            }
        }
    }

    // The ADR 0085 aggregator gap-fill fallback is retired with ADR 0086:
    // BiznesRadar sources core KPIs through its own primary pull, under the tier
    // precedence, never through this extraction run.
    let issuer_emitted = !produced_fact_ids.is_empty();

    // --- Persist the OUTCOME, emitting or not (ADR 0061 dec. 2 guardrail) ---
    // The emit branch above is unchanged; this records what the run concluded
    // for every attempt, so a Flagged/Empty period leaves a durable, reviewable
    // trace instead of evaporating with the in-memory result.
    //
    // Precedence of the set-level reason: a quarantine downgrades the acceptance
    // to Flagged and forces a `validation_failed` reason naming the quarantined
    // metric(s) — a persisted scale outlier is the most actionable signal.
    // Otherwise the normal outcome reason (an honest gap, a drift, …).
    let recorded_acceptance = if quarantined.is_empty() {
        outcome.acceptance
    } else {
        Acceptance::Flagged
    };
    let (reason_code, detail_json) = if !quarantined.is_empty() {
        (
            reason::VALIDATION_FAILED,
            quarantine_detail(&quarantined, failing_check_detail(&outcome)),
        )
    } else {
        (reason_for(&outcome), failing_check_detail(&outcome))
    };
    record_outcome(
        state,
        company_id,
        report_document_id,
        fiscal_year,
        period_type,
        period_end,
        outcome.tier.map(|t| t.as_str()),
        recorded_acceptance,
        reason_code,
        detail_json.as_deref(),
        drift_json.as_deref(),
        slot_fact_count(&produced_fact_ids, &skipped_fact_ids),
    );

    Ok(StructuredExtractionResult {
        acceptance: recorded_acceptance,
        tier: outcome.tier,
        emitted: issuer_emitted,
        produced_fact_ids,
        skipped_fact_ids,
        divergences,
        reason_code: Some(reason_code),
    })
}

/// Re-runs the extraction for a **recorded outcome slot**, by its id.
///
/// The review surface's retry action. It re-uses the company/document/period the
/// outcome row already carries instead of asking the UI to re-derive a period —
/// the same rule as everywhere else in this module: the period is derived once,
/// server-side, and never invented downstream. Because the slot id is
/// deterministic, the re-run updates the same row: a repaired period stops being
/// flagged rather than leaving a stale flag next to a fresh success.
pub(crate) fn rerun_extraction_outcome(
    state: &AppState,
    outcome_id: &str,
    mode: &str,
) -> Result<StructuredExtractionResult, String> {
    let outcome = state
        .fundamentals_provenance()
        .get_extraction_outcome(outcome_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no recorded extraction outcome '{outcome_id}'"))?;

    run_structured_extraction(
        state,
        &outcome.company_id,
        &outcome.report_document_id,
        outcome.fiscal_year,
        &outcome.period_type,
        &outcome.period_end,
        mode,
    )
}

/// Tier-3b positional route (ADR 0077 T-B2): parse a non-iXBRL pdf2htmlEX XHTML
/// render with the deterministic positional parser, run its facts through the
/// **same** [`validate_parsed_set`] identity gate every tier uses, and persist a
/// clean set under `source_tier='pdf'` with the identifiable
/// `extraction_method='html_positional'` provenance marker. A flagged/empty set
/// emits nothing (like any deterministic tier) — never `validation_status='none'`
/// (G-1).
#[allow(clippy::too_many_arguments)]
fn run_positional_extraction(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    mode: &str,
    html_bytes: &[u8],
) -> Result<StructuredExtractionResult, String> {
    use crate::fundamentals::extraction::html_positional::{
        parse_html_positional, PositionalColumn,
    };

    let html = String::from_utf8_lossy(html_bytes);
    let facts = parse_html_positional(&html, &PositionalColumn::new(period_end));

    // The SAME comparative cross-check + completeness inputs and gate the
    // structured pipeline and tier-4 assemble — the balance-sheet identity and the
    // prior-period magnitude cross-check catch a mis-read or mis-scale here too.
    let prior_end = prior_period_end(period_end);
    let stored_prior = state
        .financials()
        .stored_fact_set_for_cross_check(company_id, fiscal_year - 1, period_type, SourceTier::Pdf)
        .map_err(|e| e.to_string())?;
    let expected_keys = expected_primary_keys(state, company_id)?;
    let (acceptance, validation) = validate_parsed_set_report(
        &facts,
        period_end,
        stored_prior.as_ref(),
        prior_end.as_deref(),
        expected_keys.as_ref(),
    );

    let mut produced_fact_ids = Vec::new();
    let mut skipped_fact_ids = Vec::new();
    let mut divergences = Vec::new();
    // Same per-fact history-plausibility quarantine as the tiered path: a
    // positional-tier fact grossly off its own history is held back while its
    // siblings emit, and any quarantine flags the set.
    let mut quarantined: Vec<QuarantinedFact> = Vec::new();
    if acceptance.emits() {
        let validation_status = acceptance.validation_status();
        let confirmation_state = confirmation_state_for(acceptance, mode);
        let store = state.kpi_extraction();
        // Same batched history read as the tiered path: one company-wide read, not
        // one per positional fact (bit-identical to N single `metric_history`s).
        let history_keys: BTreeSet<String> = facts.iter().map(|f| f.metric_key.clone()).collect();
        let histories = state
            .financials()
            .metric_histories(company_id, &history_keys, fiscal_year, period_type)
            .map_err(|e| e.to_string())?;
        let mut seen_keys = BTreeSet::new();
        for fact in &facts {
            if !seen_keys.insert(fact.metric_key.clone()) {
                continue;
            }
            let empty_history = Vec::new();
            let history = histories.get(&fact.metric_key).unwrap_or(&empty_history);
            if crate::fundamentals::validation::implausible_against_history(
                &fact.metric_key,
                fact.value,
                history,
            ) {
                if let Some(history_median) =
                    crate::fundamentals::validation::history_median(history)
                {
                    quarantined.push(QuarantinedFact {
                        metric_key: fact.metric_key.clone(),
                        value: fact.value,
                        history_median,
                    });
                }
                continue;
            }
            let value = fact.value.to_string();
            let commit = store
                .record_structured_fact(StructuredFactInput {
                    company_id,
                    fiscal_year,
                    period_type,
                    period_end: Some(period_end),
                    report_document_id,
                    metric_key: &fact.metric_key,
                    value_numeric: &value,
                    currency: fact.currency.as_deref(),
                    confirmation_state,
                    // Reuse the deterministic PDF tier (no new trust-order variant);
                    // the positional sub-tier is identified by `extraction_method`.
                    source_tier: SourceTier::Pdf.as_str(),
                    extraction_method: "html_positional",
                    validation_status,
                    drift_json: None,
                    citation: Some(&fact.citation),
                })
                .map_err(|e| e.to_string())?;
            match commit {
                crate::storage::StructuredFactCommit::Created(id) => produced_fact_ids.push(id),
                // The positional tier outranks html_aggregator (ADR 0086 dec. 3)
                // — a takeover of an aggregator-held slot is this run's emit. A
                // VALUE overwrite leaves upgrade evidence as a diagnostic (F6).
                crate::storage::StructuredFactCommit::Upgraded {
                    fact_id,
                    previous_value,
                    previous_tier,
                } => {
                    record_tier_upgrade_diagnostic(
                        state,
                        company_id,
                        &fact.metric_key,
                        previous_value.as_deref(),
                        &previous_tier,
                        &value,
                    );
                    produced_fact_ids.push(fact_id);
                }
                crate::storage::StructuredFactCommit::Reobserved(id) => skipped_fact_ids.push(id),
                crate::storage::StructuredFactCommit::Divergent {
                    fact_id,
                    metric_key,
                    existing,
                    incoming,
                } => {
                    skipped_fact_ids.push(fact_id.clone());
                    divergences.push(FactDivergence {
                        fact_id,
                        metric_key,
                        existing,
                        incoming,
                    });
                }
                crate::storage::StructuredFactCommit::NoDefinition => {}
            }
        }
    }

    let tier = (!facts.is_empty()).then_some(SourceTier::Pdf);
    // The positional route is a full deterministic tier, so it records its
    // outcome on the same terms as the tiered pipeline — a non-emitting
    // positional parse must not be the one silent path left.
    let positional_outcome = crate::fundamentals::extraction::pipeline::PipelineOutcome {
        acceptance,
        tier,
        facts: Vec::new(),
        status: validation
            .as_ref()
            .map(|r| r.status)
            .unwrap_or(crate::fundamentals::validation::Status::Inconclusive),
        validation,
    };
    let recorded_acceptance = if quarantined.is_empty() {
        acceptance
    } else {
        Acceptance::Flagged
    };
    let (reason_code, detail_json) = if quarantined.is_empty() {
        (
            reason_for(&positional_outcome),
            failing_check_detail(&positional_outcome),
        )
    } else {
        (
            reason::VALIDATION_FAILED,
            quarantine_detail(&quarantined, failing_check_detail(&positional_outcome)),
        )
    };
    record_outcome(
        state,
        company_id,
        report_document_id,
        fiscal_year,
        period_type,
        period_end,
        tier.map(|t| t.as_str()),
        recorded_acceptance,
        reason_code,
        detail_json.as_deref(),
        None,
        slot_fact_count(&produced_fact_ids, &skipped_fact_ids),
    );

    Ok(StructuredExtractionResult {
        acceptance: recorded_acceptance,
        tier,
        emitted: !produced_fact_ids.is_empty(),
        produced_fact_ids,
        skipped_fact_ids,
        divergences,
        reason_code: Some(reason_code),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    // MODE_AUTOPILOT is now test-only (production no longer branches on mode for
    // confirmation state — facts are review-free, ADR 0086 dec. 5).
    use crate::storage::{
        open_in_memory_database, CaptureReportDocumentInput, ListKpiDefinitionsInput, NewCompany,
        NewFinancialFact, NewFinancialPeriod, MODE_ASSIST, MODE_AUTOPILOT,
    };

    /// Zero-effects honesty (epic #40 S5, ADR 0091): the outcome row records the
    /// facts AT the slot, so a re-run that re-observed everything cannot
    /// overwrite a healthy count with `0` while still claiming `emitted`. That
    /// row is what the #155 report-documents indicator reads — it used to render
    /// "contains extractable data" beside "0 facts".
    #[test]
    fn slot_fact_count_counts_reobservations_not_only_new_facts() {
        let produced = vec!["fact_1".to_owned(), "fact_2".to_owned()];
        let reobserved = vec![
            "fact_3".to_owned(),
            "fact_4".to_owned(),
            "fact_5".to_owned(),
        ];

        assert_eq!(slot_fact_count(&produced, &[]), 2);
        // The re-run of a landed period: nothing new, five facts at the slot.
        assert_eq!(slot_fact_count(&[], &reobserved), 3);
        assert_eq!(slot_fact_count(&produced, &reobserved), 5);
        // A genuinely empty slot still records zero — the honest zero.
        assert_eq!(slot_fact_count(&[], &[]), 0);
    }

    /// A minimal ZIP archive holding one named entry — enough for
    /// [`detect_container`] to see the `PK\x03\x04` magic and for
    /// `esef_package::extract_instance` to unpack it.
    fn minimal_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, body) in entries {
                writer
                    .start_file(*name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(body).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    // --- route_document: magic-byte container routing (card eb71488) --------
    // The routing decision is a pure function of the bytes, so it is provable
    // without an AppState — these pin every arm the router relies on.

    #[test]
    fn route_pdf_bytes_keep_the_pdf_tier() {
        // A real `%PDF` document routes exactly as before this card — no regression.
        let pdf = minimal_text_pdf(&["Przychody 100"]);
        assert_eq!(route_document(&pdf), DocumentRoute::Pdf);
    }

    #[test]
    fn route_non_ixbrl_markup_under_pdf_name_goes_positional() {
        // The maintainer's pdf2htmlEX render: XML preamble, an `<html>` root, no
        // `ix:` tags. It must reach the positional parser, not the PDF reader.
        let render = b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
            <!-- Created by pdf2htmlEX -->\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\
            <body><div>Przychody 100</div></body></html>";
        assert_eq!(route_document(render), DocumentRoute::Positional);
    }

    #[test]
    fn route_ixbrl_markup_goes_to_the_esef_instance() {
        let ixbrl = br#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"><body>
            <ix:nonFraction name="ifrs-full:Revenue">100</ix:nonFraction></body></html>"#;
        assert_eq!(route_document(ixbrl), DocumentRoute::IxbrlInstance);
    }

    #[test]
    fn route_zip_under_pdf_name_goes_to_the_package_path() {
        let zip = minimal_zip(&[("reports/instance.xhtml", b"<html></html>")]);
        assert_eq!(route_document(&zip), DocumentRoute::ZipPackage);
    }

    #[test]
    fn route_garbage_bytes_are_unsupported() {
        assert_eq!(
            route_document(b"\x00\x01\x02 definitely not a document"),
            DocumentRoute::Unsupported(Container::Unknown)
        );
    }

    /// Seeds one fetched document whose stored file is `filename` holding exactly
    /// `bytes` — used to prove the extraction entry routes on the BYTES, not the
    /// `.pdf` in the name.
    fn seed_document_with_bytes(
        label: &str,
        ticker: &str,
        title: &str,
        filename: &str,
        bytes: &[u8],
    ) -> (AppState, String, String) {
        let dir = unique_temp_dir(label);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: format!("https://example.com/{filename}"),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join(filename), bytes).expect("write bytes");
        state
            .mark_report_document_fetched(
                &document.id,
                Some(filename),
                // The maintainer's real mislabeled files are all octet-stream.
                Some("application/octet-stream"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    #[test]
    fn xml_content_under_pdf_name_routes_to_structured_tier_and_extracts() {
        // The maintainer's real failure: a pdf2htmlEX render stored as `*.pdf`.
        // Before this card the `.pdf` name sent it to the PDF reader, where it
        // produced nothing; now the XML magic routes it to the positional tier and
        // it EXTRACTS. Coverage the extension was silently throwing away.
        let (state, company_id, document_id) = seed_document_with_bytes(
            "xml-under-pdf",
            "CDR",
            "Interim condensed consolidated statement Q3 2024",
            "raport_q3_2024_signed.pdf",
            POSITIONAL_XHTML.as_bytes(),
        );
        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2024,
            "Q3",
            "2024-09-30",
            MODE_AUTOPILOT,
        )
        .expect("routes to the structured tier despite the .pdf name");
        assert!(
            result.emitted && !result.produced_fact_ids.is_empty(),
            "an XML-content file named .pdf must extract via the positional tier, not fail on the PDF reader"
        );
    }

    #[test]
    fn garbage_under_pdf_name_records_document_unreadable_with_container() {
        // Genuine garbage under a `.pdf` name lands an explicit, typed outcome
        // naming the detected container — never a mute PDF-reader failure, and
        // never an error that aborts the sweep (the caller catches the Err).
        let (state, company_id, document_id) = seed_document_with_bytes(
            "garbage-under-pdf",
            "ATR",
            "Zawiadomienie",
            "zawiadomienie.pdf",
            b"\x00\x01\x02\x03 not a document at all",
        );
        let err = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "Q1",
            "2025-03-31",
            MODE_AUTOPILOT,
        )
        .expect_err("an unsupported container is a recorded gap, surfaced as Err");
        assert!(
            err.contains("unsupported container"),
            "err names the gap: {err}"
        );

        let outcome = all_outcomes(&state, &company_id)
            .into_iter()
            .find(|o| o.report_document_id == document_id)
            .expect("an outcome row is recorded, not silence");
        assert_eq!(outcome.reason_code, reason::DOCUMENT_UNREADABLE);
        let detail = outcome.detail_json.expect("detail names the container");
        assert!(
            detail.contains("\"detectedContainer\":\"unknown\""),
            "detailJson must name the detected container: {detail}"
        );
    }

    /// Builds a minimal, valid single-page PDF whose extracted text reproduces
    /// `lines` (one label+value statement line per output line) — just enough
    /// PDF structure for `pdf-extract` to recover plain ASCII content, without a
    /// PDF-writing dependency. Byte offsets in the xref table are computed as
    /// the buffer is assembled, so it stays valid however `lines` changes.
    fn minimal_text_pdf(lines: &[&str]) -> Vec<u8> {
        // `extract_pdf` treats anything under 200 chars/page as a scanned
        // no-text-layer document (`MIN_CHARS_PER_PAGE`); pad with boilerplate
        // filler lines so a short statement excerpt still clears that density
        // floor, the way a real report page (full of surrounding prose) would.
        let filler = "Nota objasniajaca do sprawozdania finansowego za okres sprawozdawczy.";
        let mut all_lines: Vec<&str> = lines.to_vec();
        while all_lines.iter().map(|l| l.len() + 1).sum::<usize>() < 220 {
            all_lines.push(filler);
        }

        let mut content = String::from("BT /F1 12 Tf 40 750 Td 16 TL\n");
        for (i, line) in all_lines.iter().enumerate() {
            if i > 0 {
                content.push_str("T*\n");
            }
            let escaped = line
                .replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)");
            content.push_str(&format!("({escaped}) Tj\n"));
        }
        content.push_str("ET");

        let objects = [
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_owned(),
            "<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/MediaBox[0 0 612 792]/Contents 5 0 R>>"
                .to_owned(),
            "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_owned(),
            format!(
                "<</Length {}>>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
        ];

        let mut buf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(buf.len());
            buf.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
        }
        let xref_offset = buf.len();
        buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        buf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in &offsets {
            buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        buf.extend_from_slice(
            format!(
                "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF",
                objects.len() + 1,
                xref_offset
            )
            .as_bytes(),
        );
        buf
    }

    /// A per-call-unique scratch dir: `std::process::id()` alone collides
    /// across parallel `#[test]` threads (and across loop iterations within
    /// one test) sharing this file's data dir, which is a real flakiness class
    /// — two tests racing to write/read the same `report.xhtml`/`annual.pdf`
    /// path. A monotonic counter makes every call's dir distinct.
    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "brawler-structured-{}-{label}-{n}",
            std::process::id()
        ))
    }

    const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

    fn seed_esef() -> (AppState, String, String) {
        let dir = unique_temp_dir("esef");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/annual-2026.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Annual 2026 ESEF".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("report.xhtml"), ESEF.as_bytes()).expect("write esef");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(ESEF.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    /// A minimal ESEF report package (ZIP) whose inner `reports/` instance is a
    /// balanced iXBRL statement at `2025-12-31` — the shape of a real GPW `.xbri`
    /// annual filing, without shipping a real one. Includes a dimensional
    /// (`explicitMember`) Equity component that must be filtered out.
    fn esef_package_bytes() -> Vec<u8> {
        use std::io::Write;
        let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="i"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:context id="nci"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period>
        <xbrli:scenario><xbrldi:explicitMember dimension="ifrs-full:ComponentsOfEquityAxis">ifrs-full:NoncontrollingInterestsMember</xbrldi:explicitMember></xbrli:scenario>
      </xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="i" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="i" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="i" unitRef="pln" scale="3">25 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="nci" unitRef="pln" scale="3">3 000</ix:nonFraction>
    </html>"#;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(
                "CBF-2025-12-31-1-pl/reports/CBF-2025-12-31-1-pl.xhtml",
                opts,
            )
            .unwrap();
            zip.write_all(instance.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    fn seed_esef_package() -> (AppState, String, String) {
        let dir = unique_temp_dir("esef-pkg");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CBF".to_owned(),
                display_name: "Cyber_Folks S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "espi_attachment".to_owned(),
                url: "https://example.com/CBF-2025-12-31-1-pl.xbri".to_owned(),
                period_id: None,
                origin_ref: None,
                // Title carries no parseable period on purpose — the period MUST
                // come from the iXBRL contexts, not the filename.
                title: Some("CBF-2025-12-31-1-pl.xbri".to_owned()),
                attribution: None,
            })
            .expect("document");
        let bytes = esef_package_bytes();
        std::fs::write(dir.join("report.xbri"), &bytes).expect("write package");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xbri"),
                // A real `.xbri` is stored with a generic content type.
                Some("application/octet-stream"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    /// A stored PDF whose title/URL name no period at all — the real
    /// `SSF.pdf` / `Benefit_Systems_SSF_Raport_signed.pdf` attachment shape —
    /// with `cover` as its first page text.
    fn seed_untitled_pdf(cover: &[&str]) -> (AppState, String) {
        let dir = unique_temp_dir("cover-period");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "ABC".to_owned(),
                display_name: "ABC S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "espi_attachment".to_owned(),
                url: "https://example.com/SSF.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("SSF.pdf".to_owned()),
                attribution: None,
            })
            .expect("document");
        let bytes = minimal_text_pdf(cover);
        std::fs::write(dir.join("ssf.pdf"), &bytes).expect("write pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("ssf.pdf"),
                Some("application/pdf"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        (state, document.id)
    }

    #[test]
    fn period_falls_back_to_the_documents_own_cover_page() {
        // Card fc692da: on the maintainer's database a run of periodic statements
        // is stored as a bare `SSF.pdf` — nothing in the title or URL names a
        // period, so the document never reached extraction. Its cover page states
        // the period, and the SAME grammar reads it.
        let (state, document_id) = seed_untitled_pdf(&[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
            "za okres 6 miesiecy zakonczony 30.06.2025",
        ]);
        let document = state.get_report_document(&document_id).expect("document");
        assert_eq!(
            derive_report_period(&state, &document),
            Some((2025, "H1", "2025-06-30".to_owned()))
        );
    }

    /// Epic #229 T2: `is_esef_route` decides — without reading the file — whether
    /// a document goes down the ESEF/iXBRL path. It used to answer from the
    /// extension alone, so the corpus's markup and ZIP packages stored under a
    /// `.pdf` name were routed to the PDF tier and never reached the structured
    /// path at all.
    #[test]
    fn esef_route_follows_the_sniffed_container_not_the_pdf_name() {
        let (state, _company_id, document_id) = seed_document_with_bytes(
            "esef-route-liar",
            "PKN",
            "Skonsolidowane sprawozdanie finansowe 2024",
            "ssf_2024_signed.pdf",
            POSITIONAL_XHTML.as_bytes(),
        );
        // Never sniffed: the `.pdf` name still decides (the pre-T2 fallback), so a
        // legacy row routes exactly as it did before.
        let unsniffed = state.get_report_document(&document_id).expect("document");
        assert!(!is_esef_route(&unsniffed));

        for (container, expected) in [
            ("xml", true),
            ("html", true),
            ("zip", true),
            ("pdf", false),
            ("unknown", false),
        ] {
            state
                .set_report_document_detected_container(&document_id, container)
                .expect("stamp container");
            let document = state.get_report_document(&document_id).expect("document");
            assert_eq!(
                is_esef_route(&document),
                expected,
                "a document sniffed as `{container}` under a .pdf name"
            );
        }
    }

    /// Epic #229 T2: the residual→PDF-sibling fallback exists because a pdf2htmlEX
    /// container has no usable text layer and its real content sits in the
    /// companion PDF. Both ends must be container truth: a sibling that is a ZIP
    /// package wearing a `.pdf` name has no text layer either, so choosing it
    /// swaps one unreadable document for another.
    #[test]
    fn pdf_sibling_selection_uses_container_truth_on_both_ends() {
        let dir = unique_temp_dir("sibling-container");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "SIB".to_owned(),
                display_name: "Sibling S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        let seed = |file: &str, title: &str, bytes: &[u8], container: &str| -> String {
            let document = state
                .create_or_find_pending_report_document(CaptureReportDocumentInput {
                    company_id: company.id.clone(),
                    source_type: "espi_attachment".to_owned(),
                    url: format!("https://example.com/{file}"),
                    period_id: None,
                    origin_ref: None,
                    title: Some(title.to_owned()),
                    attribution: None,
                })
                .expect("document");
            assert!(
                matches!(
                    document.doc_kind.as_deref(),
                    Some("periodic_ssf") | Some("periodic_jsf")
                ),
                "sample title must classify as periodic, got {:?}",
                document.doc_kind
            );
            std::fs::write(dir.join(file), bytes).expect("write bytes");
            state
                .mark_report_document_fetched(
                    &document.id,
                    Some(file),
                    Some("application/octet-stream"),
                    None,
                    Some(bytes.len() as i64),
                )
                .expect("mark fetched");
            state
                .set_report_document_detected_container(&document.id, container)
                .expect("stamp container");
            document.id
        };

        // The residual: markup, stored under a `.pdf` name.
        let residual_id = seed(
            "raport_q3_2024_render.pdf",
            "Skonsolidowany raport okresowy Q3 2024 SSF",
            POSITIONAL_XHTML.as_bytes(),
            "html",
        );
        // The only same-period candidate is an ESEF package wearing `.pdf`.
        let package_id = seed(
            "raport_q3_2024_pakiet.pdf",
            "Skonsolidowany raport okresowy Q3 2024 SSF pakiet",
            &minimal_zip(&[("reports/instance.xhtml", b"<html></html>")]),
            "zip",
        );
        let residual = state.get_report_document(&residual_id).expect("residual");
        assert_eq!(
            find_pdf_sibling(&state, &residual).map(|d| d.id),
            None,
            "a ZIP package is not a readable PDF sibling, whatever its name says"
        );

        // Add a genuine PDF for the same period: now the fallback has a real target.
        let pdf_id = seed(
            "raport_q3_2024_ssf.pdf",
            "Skonsolidowany raport okresowy Q3 2024 SSF podpisany",
            &minimal_text_pdf(&["Przychody 100"]),
            "pdf",
        );
        assert_eq!(
            find_pdf_sibling(&state, &residual).map(|d| d.id),
            Some(pdf_id),
            "the genuine PDF is the sibling — selected over the same-period package"
        );
        assert_ne!(residual_id, package_id);
    }

    /// Epic #229 T2: the cover-page tier is the last resort for a bare `SSF.pdf`
    /// whose title and URL name no period. Reading that cover with the PDF reader
    /// because the name ends `.pdf` returns nothing when the bytes are markup — the
    /// document then has no period, so it never reaches extraction at all. Container
    /// truth reads the same cover as markup and the period lands.
    #[test]
    fn cover_page_period_reads_markup_stored_under_a_pdf_name() {
        let dir = unique_temp_dir("cover-container");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CVR".to_owned(),
                display_name: "Cover S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "espi_attachment".to_owned(),
                url: "https://example.com/SSF.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("SSF.pdf".to_owned()),
                attribution: None,
            })
            .expect("document");
        // A pdf2htmlEX render: the cover text is markup, the name says PDF.
        let body = format!(
            "<html><body><h1>SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ CVR</h1>\
             <p>za okres 6 miesiecy zakonczony 30.06.2025</p><p>{}</p></body></html>",
            "dane porownawcze oraz komentarz zarzadu. ".repeat(120)
        );
        std::fs::write(dir.join("ssf.pdf"), body.as_bytes()).expect("write render");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("ssf.pdf"),
                Some("application/pdf"),
                None,
                Some(body.len() as i64),
            )
            .expect("mark fetched");
        state
            .set_report_document_detected_container(&document.id, "html")
            .expect("stamp container");

        let stored = state.get_report_document(&document.id).expect("document");
        assert_eq!(
            derive_report_period(&state, &stored),
            Some((2025, "H1", "2025-06-30".to_owned())),
            "the cover page must be read with the reader the BYTES call for"
        );
    }

    #[test]
    fn period_abstains_when_neither_title_nor_cover_page_names_one() {
        // The abstention contract survives the new fallback: a cover page that
        // states no period persists nothing and records `no_period_derived`
        // (ADR 0061 decision 1) — widening the parse must never turn "I don't
        // know" into a guess.
        let (state, document_id) = seed_untitled_pdf(&[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
            "Nota informacyjna do sprawozdania.",
        ]);
        let document = state.get_report_document(&document_id).expect("document");
        assert_eq!(derive_report_period(&state, &document), None);
    }

    #[test]
    fn cover_page_period_is_derived_once_then_served_from_cache() {
        // E2/C4: the bare-SSF cover-page tier costs a full PDF text extraction.
        // The first derivation persists the period (migration 0109); a second one
        // reads the cache — proven by DELETING the file between the two calls, so a
        // second call that still returns the period cannot have re-read/extracted.
        let (state, document_id) = seed_untitled_pdf(&[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
            "za okres 6 miesiecy zakonczony 30.06.2025",
        ]);
        let document = state.get_report_document(&document_id).expect("document");

        let first = derive_report_period(&state, &document);
        assert_eq!(first, Some((2025, "H1", "2025-06-30".to_owned())));

        let cached = state
            .financials()
            .cached_derived_period(&document_id)
            .expect("cache read")
            .expect("the first derivation persisted a row");
        assert!(cached.has_period);

        // Any re-extraction now fails, so an identical result proves the cache.
        let local_path = document.local_path.clone().expect("local path");
        std::fs::remove_file(state.data_dir().join(&local_path)).expect("remove pdf");

        assert_eq!(
            derive_report_period(&state, &document),
            first,
            "second derivation must be served from the cache, not re-extracted"
        );
    }

    #[test]
    fn cover_page_abstention_is_cached_as_a_none_marker() {
        // An abstention (cover page names no period) is recorded too — has_period
        // = 0 — so the next load does not re-extract a document that once again
        // yields nothing.
        let (state, document_id) = seed_untitled_pdf(&[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
            "Nota informacyjna do sprawozdania.",
        ]);
        let document = state.get_report_document(&document_id).expect("document");

        assert_eq!(derive_report_period(&state, &document), None);
        let cached = state
            .financials()
            .cached_derived_period(&document_id)
            .expect("cache read")
            .expect("the abstention is persisted");
        assert!(
            !cached.has_period,
            "an abstention is an explicit none-marker"
        );

        // Delete the file: the second call must still return None from the marker
        // without touching the (now absent) file.
        let local_path = document.local_path.clone().expect("local path");
        std::fs::remove_file(state.data_dir().join(&local_path)).expect("remove pdf");
        assert_eq!(derive_report_period(&state, &document), None);
    }

    #[test]
    fn a_stale_derivation_version_is_re_derived_not_served() {
        // Self-healing invalidation: a row stamped with an older DERIVATION_VERSION
        // is ignored and re-derived. Proven by planting a stale row with a BOGUS
        // period, deleting the file, and asserting the derive returns None (a
        // re-derivation of a now-fileless document) rather than the stale period.
        let (state, document_id) = seed_untitled_pdf(&[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
            "za okres 6 miesiecy zakonczony 30.06.2025",
        ]);
        let document = state.get_report_document(&document_id).expect("document");

        state
            .financials()
            .store_derived_period(
                &document_id,
                Some((1999, "FY", "1999-12-31")),
                DERIVATION_VERSION - 1,
            )
            .expect("plant a stale-version row");

        let local_path = document.local_path.clone().expect("local path");
        std::fs::remove_file(state.data_dir().join(&local_path)).expect("remove pdf");

        assert_eq!(
            derive_report_period(&state, &document),
            None,
            "a stale-version row must be re-derived, never served"
        );
    }

    #[test]
    fn esef_package_derives_fy_period_from_ixbrl_not_the_filename() {
        // T7-C: a `.xbri` ZIP package resolves to the ESEF tier; the period is
        // self-derived from the unpacked instance's contexts (FY 2025-12-31),
        // even though the filename carries no parseable period.
        let (state, _company_id, document_id) = seed_esef_package();
        let document = state.get_report_document(&document_id).expect("document");
        assert_eq!(
            derive_report_period(&state, &document),
            Some((2025, "FY", "2025-12-31".to_owned()))
        );
    }

    #[test]
    fn esef_package_extraction_emits_dimensionless_totals() {
        // T7-C end to end: the button path (derive + run) over a `.xbri` package
        // emits the three balance-sheet totals from the unpacked instance, with
        // the dimensional NCI-component Equity filtered out (total_equity = 25m,
        // not 25m+3m), so the identity validates and the set is Accepted.
        let (state, company_id, document_id) = seed_esef_package();
        let document = state.get_report_document(&document_id).expect("document");
        let (fiscal_year, period_type, period_end) =
            derive_report_period(&state, &document).expect("period derives");
        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            fiscal_year,
            period_type,
            &period_end,
            MODE_AUTOPILOT,
        )
        .expect("structured extraction runs");

        assert_eq!(result.tier, Some(SourceTier::Esef));
        assert_eq!(result.acceptance, Acceptance::Accepted);
        assert!(result.emitted, "the ESEF package should emit facts");
        assert_eq!(result.produced_fact_ids.len(), 3);
    }

    /// A non-iXBRL pdf2htmlEX render (an XHTML with no `ix:` tags): the visual
    /// geometry with CSS coordinate maps and shredded numbers no ESEF/PDF tier can
    /// read. `derive_report_period` falls through to the title (T-A1).
    const POSITIONAL_XHTML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head>
<style type="text/css">
.x0{left:56.000000px;}
.y0{bottom:700.000000px;}
.y1{bottom:680.000000px;}
.y2{bottom:660.000000px;}
.y3{bottom:640.000000px;}
</style></head><body>
<div id="pf1" class="pf w0 h0">
<div class="t m0 x0 h4 y0 ff2">(all amounts in PLN thousand, unless stated otherwise) </div>
<div class="t m0 x0 hb y1 ff2">Sales revenue  227 555  442 682  652 375  767 692</div>
<div class="t m0 x0 hb y2 ff2">Total assets  2 755 416  2 613 500</div>
<div class="t m0 x0 hb y3 ff2">Equity  2 570 916  2 403 223</div>
</div></body></html>"#;

    fn seed_positional() -> (AppState, String, String) {
        let dir = unique_temp_dir("positional");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/interim-q3-2024.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Interim condensed consolidated statement Q3 2024".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("report.xhtml"), POSITIONAL_XHTML.as_bytes()).expect("write");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(POSITIONAL_XHTML.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    #[test]
    fn non_ixbrl_xhtml_routes_to_the_positional_tier_and_persists_pdf_provenance() {
        // ADR 0077 T-B2: a non-iXBRL XHTML no longer falls through as
        // not-extractable — it routes to the tier-3b positional parser, whose facts
        // clear the SAME validate gate and persist under `source_tier='pdf'` with an
        // identifiable `extraction_method='html_positional'` (never validation_status
        // 'none', G-1). Revenue reads the YTD (652 375) via automatic column
        // selection, not the 3M column.
        let (state, company_id, document_id) = seed_positional();
        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2024,
            "Q3",
            "2024-09-30",
            MODE_AUTOPILOT,
        )
        .expect("positional extraction runs");

        assert_eq!(
            result.tier,
            Some(SourceTier::Pdf),
            "reuses the deterministic Pdf tier"
        );
        assert!(result.emitted, "the positional render emits facts");
        assert!(!result.produced_fact_ids.is_empty());

        // Revenue is the current YTD, and provenance is honest + identifiable.
        let facts = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts");
        let revenue = facts
            .iter()
            .find(|f| f.value_numeric.starts_with("652375"))
            .expect("revenue reads the YTD column (652 375 × 1000), not the 3M column");
        assert_eq!(
            revenue.extraction_method, "html_positional",
            "the positional sub-tier is identifiable via extraction_method"
        );
        for id in &result.produced_fact_ids {
            let provenance = state
                .fundamentals_provenance()
                .get_fact_provenance(id)
                .expect("provenance query")
                .expect("every positional fact carries provenance");
            assert_eq!(provenance.source_tier, "pdf");
            assert_ne!(
                provenance.validation_status, "none",
                "the positional path clears the validation gate — never 'none' (G-1)"
            );
        }
    }

    #[test]
    fn re_extracting_the_same_document_is_idempotent_not_a_unique_violation() {
        // Owner T7 bug: clicking "Wyciągnij dane" a second time on a document
        // whose facts already landed must NOT surface a UNIQUE constraint error.
        // Each incoming fact whose full uniqueness slot matches an existing row
        // is a RE-OBSERVATION: same value ⇒ skipped (counted, never produced),
        // the run succeeds cleanly and the DB keeps exactly one row per slot.
        let (state, company_id, document_id) = seed_esef_package();

        let first = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("first extraction runs");
        assert_eq!(first.produced_fact_ids.len(), 3);
        assert!(first.skipped_fact_ids.is_empty());
        assert!(first.divergences.is_empty());

        // Second click over the identical document + period.
        let second = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("re-extraction must not error with a UNIQUE violation");

        assert!(
            second.produced_fact_ids.is_empty(),
            "a re-observation produces no new facts"
        );
        assert_eq!(
            second.skipped_fact_ids.len(),
            3,
            "all three facts already exist at their slot → skipped"
        );
        assert!(!second.emitted, "no new facts emitted on re-extraction");
        assert!(
            second.divergences.is_empty(),
            "identical values → no divergence"
        );

        // The DB still holds exactly one fact per slot — no duplication.
        let facts = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts");
        assert_eq!(facts.len(), 3, "re-extraction must not duplicate rows");
    }

    #[test]
    fn re_extraction_with_a_diverging_value_is_skipped_and_reported_not_overwritten() {
        // A re-observation whose slot matches but whose value differs from the
        // already-committed (confirmed) fact must NOT silently overwrite it: the
        // safe minimal behavior (spec is silent on value conflicts) is skip +
        // record the divergence for ratification. The stored value is unchanged.
        let (state, company_id, document_id) = seed_esef_package();
        let first = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("first extraction runs");
        assert_eq!(first.produced_fact_ids.len(), 3);

        // Mutate one stored fact so the next identical extraction diverges.
        let assets = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts")
            .into_iter()
            .find(|f| f.value_numeric.trim_start_matches('-').starts_with("45"))
            .expect("the 45m total-assets fact should exist");
        state
            .update_financial_fact(crate::storage::UpdateFinancialFact {
                id: assets.id.clone(),
                value_numeric: Some("999000000".to_owned()),
                currency: None,
                data_quality: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: None,
                annotation: None,
            })
            .expect("mutate stored fact");

        let second = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("re-extraction must not error");

        assert!(second.produced_fact_ids.is_empty());
        assert_eq!(
            second.skipped_fact_ids.len(),
            3,
            "every matching slot is skipped, diverging or not"
        );
        assert_eq!(second.divergences.len(), 1, "the one mutated slot diverges");
        let divergence = &second.divergences[0];
        assert_eq!(divergence.existing.trim(), "999000000");

        // The confirmed fact is untouched — never silently overwritten.
        let after = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts")
            .into_iter()
            .find(|f| f.id == assets.id)
            .expect("fact still present");
        assert_eq!(after.value_numeric.trim(), "999000000");
    }

    #[test]
    fn derive_report_period_reads_the_esef_period_from_the_stored_file() {
        // The shared derivation the on-demand "Extract data" command relies on:
        // an ESEF filing self-derives its `FY` period from the iXBRL contexts.
        let (state, _company_id, document_id) = seed_esef();
        let document = state.get_report_document(&document_id).expect("document");
        assert_eq!(
            derive_report_period(&state, &document),
            Some((2026, "FY", "2026-03-31".to_owned()))
        );
    }

    #[test]
    fn derive_report_period_is_none_for_a_document_with_no_stored_file() {
        // A metadata-only (unfetched) document has no local file to parse, so no
        // period can be derived — the "Extract data" command surfaces this as a
        // clear error instead of inventing a period.
        let dir = unique_temp_dir("no-file");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/pending.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Some report".to_owned()),
                attribution: None,
            })
            .expect("document");
        // Never marked fetched → `local_path` is None.
        assert_eq!(derive_report_period(&state, &document), None);
    }

    /// A plain XHTML with NO inline-XBRL (`ix:`) tags — a pdf2htmlEX render of an
    /// interim report. It is on the ESEF route by extension but cannot self-derive
    /// a period from contexts.
    const NON_IXBRL_XHTML: &str = "<html><head><title>Raport</title></head><body>\
<h1>Skonsolidowany raport za III kwartał 2024</h1><p>Treść raportu.</p></body></html>";

    fn seed_non_ixbrl_xhtml(title: &str, url: &str) -> (AppState, String, String) {
        let dir = unique_temp_dir("non-ixbrl");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CBF".to_owned(),
                display_name: "Cyber_Folks S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: url.to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("interim.xhtml"), NON_IXBRL_XHTML.as_bytes()).expect("write xhtml");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("interim.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(NON_IXBRL_XHTML.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    #[test]
    fn derive_report_period_falls_through_to_title_for_non_ixbrl_xhtml() {
        // T-A1: a stored `.xhtml` that is NOT valid iXBRL (a pdf2htmlEX render of
        // an interim report) cannot self-derive a period from contexts. Before the
        // fallthrough `derive_report_period` returned None ("no derivable period");
        // now the title's "Q3_2024" is parsed the same way a PDF's title would be.
        let (state, _company_id, document_id) = seed_non_ixbrl_xhtml(
            "cyber_Folks_SSF_Q3_2024.xhtml",
            "https://example.com/ssf_q3_2024.xhtml",
        );
        let document = state.get_report_document(&document_id).expect("document");
        assert_eq!(
            derive_report_period(&state, &document),
            Some((2024, "Q3", "2024-09-30".to_owned()))
        );
    }

    // A minimal balanced ESEF instance carrying a second, prior-period context
    // (40m = 18m + 22m at 2025-03-31) alongside the current one (45m = 20m +
    // 25m at 2026-03-31) — ESEF tags comparatives natively, so this exercises
    // the comparative cross-check (ADR 0061 dec. 4b) through tier 1.
    const ESEF_WITH_PRIOR: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:context id="p"><xbrli:period><xbrli:instant>2025-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="p" unitRef="pln" scale="3">40 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="p" unitRef="pln" scale="3">18 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="p" unitRef="pln" scale="3">22 000</ix:nonFraction>
    </html>"#;

    fn seed_esef_with_prior() -> (AppState, String, String) {
        let dir = unique_temp_dir("esef-prior");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/annual-2026.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Annual 2026 ESEF".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("report.xhtml"), ESEF_WITH_PRIOR.as_bytes()).expect("write esef");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(ESEF_WITH_PRIOR.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    /// Seeds a prior-period `financial_period` + facts for `company_id`,
    /// bridging each `(metric_key, value)` through the canonical KPI
    /// definition catalog — the "already known" prior period the comparative
    /// cross-check reads back via `stored_fact_set`.
    fn seed_prior_period(
        state: &AppState,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
        facts: &[(&str, &str)],
    ) {
        let period = state
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.to_owned(),
                fiscal_year,
                period_type: period_type.to_owned(),
                period_end_date: None,
                report_evidence_ref: None,
            })
            .expect("prior financial period should create");
        let definitions = state
            .list_kpi_definitions(ListKpiDefinitionsInput {
                scope: Some("canonical".to_owned()),
                sector: None,
                company_id: None,
            })
            .expect("canonical definitions should list");
        for (metric_key, value) in facts {
            let definition = definitions
                .iter()
                .find(|d| d.metric_key == *metric_key)
                .unwrap_or_else(|| panic!("{metric_key} should exist in the canonical catalog"));
            state
                .create_financial_fact(NewFinancialFact {
                    company_id: company_id.to_owned(),
                    period_id: period.id.clone(),
                    definition_id: definition.id.clone(),
                    value_numeric: (*value).to_owned(),
                    currency: Some("PLN".to_owned()),
                    statement_basis: None,
                    attribution: None,
                    variant: None,
                    measure_window: None,
                    data_quality: None,
                    as_reported_value: None,
                    as_reported_scale: None,
                    reporting_standard: None,
                    extraction_method: None,
                    confidence: None,
                    confirmation_state: Some("confirmed".to_owned()),
                    supersedes_id: None,
                    source_document_ref: None,
                    annotation: None,
                })
                .expect("prior financial fact should create");
        }
    }

    /// Fetches the `confirmation_state` of a produced fact via the public
    /// list-facts read model (the only way to read that field from outside
    /// `storage::financials`).
    fn confirmation_states(state: &AppState, company_id: &str, fact_ids: &[String]) -> Vec<String> {
        state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.to_owned()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts")
            .into_iter()
            .filter(|f| fact_ids.contains(&f.id))
            .map(|f| f.confirmation_state)
            .collect()
    }

    #[test]
    fn esef_extraction_confirms_in_both_modes() {
        for mode in [MODE_ASSIST, MODE_AUTOPILOT] {
            let (state, company_id, document_id) = seed_esef();
            let result = run_structured_extraction(
                &state,
                &company_id,
                &document_id,
                2026,
                "FY",
                "2026-03-31",
                mode,
            )
            .expect("structured extraction runs");

            assert!(result.emitted, "ESEF facts should be emitted");
            assert_eq!(result.tier, Some(SourceTier::Esef));
            assert_eq!(result.acceptance, Acceptance::Accepted);
            assert_eq!(result.produced_fact_ids.len(), 3);

            // Every produced fact carries structured provenance: tier + passed status.
            // (The retired drift plumbing is asserted absent at the DB level below.)
            let provenance = state
                .fundamentals_provenance()
                .get_many(&result.produced_fact_ids)
                .expect("provenance");
            assert_eq!(provenance.len(), 3);
            assert!(provenance.iter().all(|p| p.source_tier == "esef"));
            assert!(provenance.iter().all(|p| p.validation_status == "passed"));
            assert!(provenance.iter().all(|p| p.drift_json.is_none()));

            // ADR 0061 dec. 3/8/9: a validation-clean structured set auto-confirms
            // in BOTH modes — no unreviewed grace period for a proven fact.
            let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
            assert!(
                states.iter().all(|s| s == "confirmed"),
                "mode={mode} states={states:?}"
            );
        }
    }

    // -------------------------------------------------------------------
    // Comparative cross-check + completeness, live in the pipeline (ADR
    // 0061 dec. 4b/4d): the DB-level wiring — stored_fact_set lookup,
    // prior_period_end derivation, kpi_relevance → expected_keys bridge.
    // -------------------------------------------------------------------

    #[test]
    fn esef_with_matching_stored_prior_cross_check_stays_confirmed() {
        let (state, company_id, document_id) = seed_esef_with_prior();
        seed_prior_period(
            &state,
            &company_id,
            2025,
            "FY",
            &[
                ("total_assets", "40000000"),
                ("total_liabilities", "18000000"),
                ("total_equity", "22000000"),
            ],
        );

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        )
        .expect("structured extraction runs");

        assert_eq!(result.acceptance, Acceptance::Accepted);
        assert!(result.emitted);
        assert_eq!(result.tier, Some(SourceTier::Esef));
        assert_eq!(result.produced_fact_ids.len(), 3);
        let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
        assert!(states.iter().all(|s| s == "confirmed"), "states={states:?}");
    }

    #[test]
    fn esef_with_mismatching_stored_prior_cross_check_is_not_silently_accepted() {
        // The filing's comparative column (40m) disagrees with what is
        // already stored for that period (999m) — the strongest signal of a
        // misread/wrong-context tagging. No PDF/witness fallback exists for
        // this report, so the tier-1 failure yields no accepted tier — the
        // key guardrail is that it is never silently emitted as if proven.
        let (state, company_id, document_id) = seed_esef_with_prior();
        seed_prior_period(
            &state,
            &company_id,
            2025,
            "FY",
            &[("total_assets", "999000000")],
        );

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        )
        .expect("structured extraction runs");

        assert_ne!(
            result.acceptance,
            Acceptance::Accepted,
            "a contradicted comparative must never be silently accepted"
        );
        assert!(!result.emitted);
        assert!(result.produced_fact_ids.is_empty());
    }

    /// ADR 0086 decisions 3/4: a stored prior sourced by a LOWER tier (the daily
    /// BiznesRadar pull) must never veto an ISSUER tier's emission — the issuer
    /// witnesses the aggregator, not the other way around. (Live regression,
    /// 2026-07-22: CBF's whole FY2025 ESEF set was discarded because BR's stored
    /// FY2024 equity disagreed with the filing's own comparative by 68%.)
    #[test]
    fn esef_emits_despite_a_mismatching_aggregator_sourced_prior() {
        let (state, company_id, document_id) = seed_esef_with_prior();
        // Seed the prior period the way the BR-primary pull does: value + an
        // html_aggregator provenance row.
        state
            .kpi_extraction()
            .record_structured_fact(crate::storage::StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: Some("2025-03-31"),
                report_document_id: &document_id,
                metric_key: "total_assets",
                value_numeric: "999000000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "html_aggregator",
                extraction_method: "api",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("https://biznesradar.example | Aktywa razem"),
            })
            .expect("aggregator prior");

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        )
        .expect("structured extraction runs");

        assert_eq!(
            result.acceptance,
            Acceptance::Accepted,
            "an aggregator-sourced prior must not veto the issuer's filing"
        );
        assert!(result.emitted);
        assert_eq!(result.tier, Some(SourceTier::Esef));
    }

    /// The honesty half (ADR 0061 dec. 2): an ESEF set a SAME-or-higher-tier
    /// prior contradicts is a FLAGGED outcome carrying the failing checks —
    /// never a silent `empty` that reads like "nothing to extract".
    #[test]
    fn esef_failed_validation_is_flagged_with_detail_not_silent_empty() {
        let (state, company_id, document_id) = seed_esef_with_prior();
        seed_prior_period(
            &state,
            &company_id,
            2025,
            "FY",
            &[("total_assets", "999000000")],
        );

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        )
        .expect("structured extraction runs");

        assert_eq!(
            result.acceptance,
            Acceptance::Flagged,
            "a contradicted filing is flagged, never a silent empty"
        );
        assert!(!result.emitted);
        let outcome = state
            .fundamentals_provenance()
            .list_flagged_extraction_outcomes(&company_id)
            .expect("outcomes")
            .into_iter()
            .find(|o| o.report_document_id == document_id)
            .expect("the flagged run must leave a reviewable outcome row");
        assert_eq!(outcome.reason_code, "validation_failed");
        assert!(
            outcome.detail_json.is_some(),
            "the failing checks must be persisted for review"
        );
    }

    #[test]
    fn missing_file_errors_cleanly() {
        let (state, company_id, document_id) = seed_esef();
        // Remove the file to simulate a broken fetch.
        std::fs::remove_file(state.data_dir().join("report.xhtml")).ok();
        let err = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        );
        assert!(err.is_err());
    }

    #[test]
    fn minimal_pdf_fixture_extracts_expected_text() {
        // Guards the hand-built PDF test fixture itself: if this regresses, every
        // PDF-tier test above would fail for the wrong reason (a broken fixture,
        // not a pipeline bug).
        let bytes = minimal_text_pdf(&["Zysk netto 12 000", "Aktywa razem 45 000"]);
        let outcome = crate::report_diff::extraction::extract_report(
            &bytes,
            crate::report_diff::extraction::SourceFormat::Pdf,
        );
        let text = outcome
            .sections
            .iter()
            .map(|s| s.body.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("Zysk netto") && text.contains("45 000"),
            "state={:?} char_count={} text={text:?}",
            outcome.state,
            outcome.char_count
        );
    }

    // -----------------------------------------------------------------------
    // A2 — the persistence half of "never silently wrong" (ADR 0061 dec. 2,
    // ADR 0084 dec. 4). A run that emits nothing must still leave a durable,
    // queryable record of what the pipeline tried and what objected.
    // -----------------------------------------------------------------------

    /// Every outcome the store holds for a company, flagged or not — the
    /// "was this period ever attempted?" question the review read model
    /// deliberately narrows.
    fn all_outcomes(state: &AppState, company_id: &str) -> Vec<crate::storage::ExtractionOutcome> {
        // `list_flagged_extraction_outcomes` is the review surface (non-emitting
        // only); for the never-attempted assertions the tests need the raw set,
        // so read both and merge with the by-id lookup of the emitting slots.
        state
            .fundamentals_provenance()
            .list_flagged_extraction_outcomes(company_id)
            .expect("list outcomes")
    }

    /// F6 (ADR 0086 code-review): when an issuer tier OVERWRITES a lower-tier
    /// slot's VALUE (a real disagreement, not a label-only takeover), the ESEF /
    /// positional emit path records a `tier_upgrade` diagnostic carrying the
    /// previous value + tier — mirroring the WDF cover-note seam. A label-only
    /// upgrade (values agreed) records nothing.
    #[test]
    fn a_value_overwriting_upgrade_records_a_tier_upgrade_diagnostic() {
        let (state, company_id, document_id) = seed_esef();
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode enables the diagnostic sink");

        // Seed a LOWER-tier (aggregator) fact holding a DIFFERENT value in the
        // very slot the ESEF extraction will land total_assets into.
        state
            .kpi_extraction()
            .record_aggregator_fact(crate::storage::StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2026,
                period_type: "FY",
                period_end: Some("2026-03-31"),
                report_document_id: &document_id,
                metric_key: "total_assets",
                value_numeric: "1000000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "html_aggregator",
                extraction_method: "api",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("https://biznesradar.example/page | Aktywa"),
            })
            .expect("seed aggregator slot");

        // ESEF (issuer) re-extracts total_assets = 45,000,000 — a value overwrite
        // of the aggregator-held slot (Upgraded { previous_value: Some }).
        run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        )
        .expect("structured extraction");

        let events = state.list_diagnostic_events(50).expect("list diagnostics");
        let upgrade = events
            .iter()
            .find(|e| e.module == "structured_extraction" && e.stage == "tier_upgrade")
            .expect("a value-overwriting upgrade must leave a tier_upgrade diagnostic");
        assert_eq!(
            upgrade.metadata.get("metricKey").and_then(|v| v.as_str()),
            Some("total_assets")
        );
        assert_eq!(
            upgrade
                .metadata
                .get("previousValue")
                .and_then(|v| v.as_str()),
            Some("1000000")
        );
        assert_eq!(
            upgrade
                .metadata
                .get("previousTier")
                .and_then(|v| v.as_str()),
            Some("html_aggregator")
        );
    }
}
