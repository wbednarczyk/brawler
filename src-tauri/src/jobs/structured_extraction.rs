//! Structured-first fundamentals extraction service (ADR 0061 S5).
//!
//! Loads a stored report document, runs the deterministic tiered pipeline
//! (ESEF → PDF+profile → HTML witness), and persists the accepted facts with
//! their provenance (source tier + validation verdict + citation) and the
//! learned per-company extraction profile. This is the structured-first path
//! that runs *before* the AI proposal job — AI is only the last resort when no
//! structured tier produces a validated set.
//!
//! Live aggregator (BiznesRadar/Bankier) fetch is gated on a source-specific
//! scraping ADR (see ADR 0061 decision 4), so the witness tier runs with no
//! remote fetch here yet; the pipeline degrades cleanly to ESEF + PDF.

use std::collections::BTreeSet;

use crate::app_state::AppState;
use crate::fundamentals::extraction::ocr::{parse_ocr_markdown, OcrExtractionProfile};
use crate::fundamentals::extraction::pdf::parse_pdf_text;
use crate::fundamentals::extraction::pipeline::{
    run_pipeline, validate_parsed_set, Acceptance, PipelineInput,
};
use crate::fundamentals::extraction::profile::ExtractionProfile;
use crate::fundamentals::extraction::{ExtractedFact, FactPeriod, SourceTier};
use crate::providers::analysis::{
    capabilities::{AiCapability, CAPABILITY_ROUTED_PROVIDER_ID},
    ocr_profile_bootstrap_prompt, AiAnalysisProvider, AnalysisDocument, KpiCatalogEntry,
    OCR_PROFILE_BOOTSTRAP_PROMPT_VERSION,
};
use crate::report_diff::extraction::{extract_report, SourceFormat};
use crate::storage::{
    CompletedKpiExtraction, ListKpiDefinitionsInput, NewKpiExtractionJob, NewKpiProposal,
    StructuredFactInput, MODE_AUTOPILOT,
};

/// The tier-4 (OCR) caller gate (ADR 0077 §4 kickoff decision 4). The vision
/// fallback runs **only** when this is [`Tier4Gate::Allowed`]; the two denied
/// variants keep it off with an honest reason so nothing is skipped silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tier4Gate {
    /// Determinism ended Flagged|Empty and the caller (manual/detection) permits
    /// the OCR fallback.
    Allowed,
    /// A history-sweep run: tier-4 stays off until the F3b budget counter exists
    /// (ADR 0077 §3 amendment (b)); passed for defense in depth alongside the
    /// autopilot stage's own pre-hook early-return.
    DeniedSweep,
    /// Reserved for the T5.2 budget counter (`history_sweep_ai_call_limit`): the
    /// budget seam the ADR approved, wired here so plugging it in later needs no
    /// hook-shape change. Unused until T5.2.
    #[allow(dead_code)]
    DeniedBudget,
}

/// The immediately-prior period's end date for `period_end` (`YYYY-MM-DD`),
/// by decrementing the leading year — the same fiscal period one year
/// earlier. `None` when `period_end` doesn't start with a parseable year.
fn prior_period_end(period_end: &str) -> Option<String> {
    let year: i64 = period_end.get(0..4)?.parse().ok()?;
    Some(format!("{:04}{}", year - 1, period_end.get(4..)?))
}

/// The company's expected primary-KPI `metric_key`s (ADR 0061 dec. 4d): every
/// `kpi_relevance` row that is `active` and ranked `primary` (case-
/// insensitively), bridged to its `metric_key` via the definition catalog.
/// `None` when there are none (nothing to check completeness against) —
/// never blocks emission by itself.
fn expected_primary_keys(
    state: &AppState,
    company_id: &str,
) -> Result<Option<BTreeSet<String>>, String> {
    let relevance = state
        .financials()
        .list_kpi_relevance(company_id)
        .map_err(|e| e.to_string())?;
    let primary_definition_ids: BTreeSet<String> = relevance
        .into_iter()
        .filter(|r| {
            r.status == "active"
                && r.rank
                    .as_deref()
                    .is_some_and(|rank| rank.eq_ignore_ascii_case("primary"))
        })
        .map(|r| r.definition_id)
        .collect();
    if primary_definition_ids.is_empty() {
        return Ok(None);
    }

    let definitions = state
        .financials()
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .map_err(|e| e.to_string())?;
    let keys: BTreeSet<String> = definitions
        .into_iter()
        .filter(|d| primary_definition_ids.contains(&d.id))
        .map(|d| d.metric_key)
        .collect();
    Ok(if keys.is_empty() { None } else { Some(keys) })
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
    /// Serialized `DriftReport` when the layout drifted (for the notification).
    pub drift_json: Option<String>,
    /// Whether the pipeline detected a layout drift (`drift_json.is_some()`),
    /// surfaced separately so callers can merge a `structureChanged` flag into
    /// a run's `kpi_delta_json` without re-deriving it from the option.
    pub structure_changed: bool,
    /// Whether the pipeline emitted any facts (structured tiers **or** tier-4).
    pub emitted: bool,
    /// The tier-4 (OCR) fallback's honest outcome when it ran (ADR 0077 §4), else
    /// `None` (determinism emitted, or the gate denied it). One of:
    /// `facts_emitted` · `bootstrap_proposals` · `proposals_flagged` ·
    /// `no_vision_provider` · `not_pdf` · `provider_error:<code>` · `empty`.
    pub tier4: Option<String>,
    /// How many proposals the tier-4 fallback landed for confirmation (bootstrap
    /// or validation-failing values). `0` when tier-4 emitted facts or did not run.
    pub tier4_proposals: usize,
}

/// The per-fact `confirmation_state` for an accepted outcome, given the run
/// mode (ADR 0061 dec. 3/8/9). A structured, validation-clean set — `Accepted`
/// or `AcceptedViaWitness` — auto-confirms in **both** modes: the "good gate"
/// already proved it, so review would add no signal. An `AcceptedUnreviewed`
/// set (nothing proven, nothing contradicted) keeps the pre-existing trust
/// ladder: `auto_unreviewed` in autopilot, `pending` in assist. `Flagged`/
/// `Empty` never reach here (`Acceptance::emits()` is `false`), so their arm is
/// unreachable in practice but still total.
fn confirmation_state_for(acceptance: Acceptance, mode: &str) -> &'static str {
    match acceptance {
        Acceptance::Accepted | Acceptance::AcceptedViaWitness => "confirmed",
        Acceptance::AcceptedUnreviewed => {
            if mode == MODE_AUTOPILOT {
                "auto_unreviewed"
            } else {
                "pending"
            }
        }
        Acceptance::Flagged | Acceptance::Empty => "pending",
    }
}

/// Derives the reporting period `(fiscal_year, period_type, period_end)` for a
/// stored report document the SAME way the autopilot pipeline does (ADR 0061
/// dec. 3/8/9) — the single source of truth shared by the autopilot stage and
/// the on-demand "Extract data" command, so the two never drift. `None` when the
/// document has no stored file, or its period can't be classified from the iXBRL
/// contexts NOR its title/URL (the caller then falls back to AI, or surfaces an
/// error).
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
pub fn derive_report_period(
    state: &AppState,
    document: &crate::storage::ReportDocument,
) -> Option<(i64, &'static str, String)> {
    use crate::fundamentals::extraction::{esef::parse_esef, primary_period_end};
    use crate::report_diff::classify::period_sort_key;

    let local_path = document.local_path.as_deref()?;
    let content_type = document.content_type.as_deref();
    // ESEF tier — a bare `.xhtml` instance OR a `.xbri`/`.zip` report package
    // (ADR 0061 dec. 1). Read the file only for the formats that self-derive
    // their period from the iXBRL contexts; a PDF derives it from its title/URL
    // without a read.
    if is_esef_route(content_type, local_path) {
        // ESEF self-derives its period from the iXBRL contexts. A file that is the
        // ESEF route by extension/content-type but is NOT valid iXBRL — an interim
        // XHTML with no `ix:` tags (a pdf2htmlEX render) — yields no period here;
        // fall THROUGH to the title/URL derivation below rather than returning
        // None (T-A1), the same fallback the coverage read model already applies.
        let esef_period = (|| {
            let raw = std::fs::read(state.data_dir().join(local_path)).ok()?;
            let instance = esef_instance_bytes(content_type, local_path, &raw)?;
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
    // document's title/URL.
    let title = document.title.as_deref().unwrap_or("");
    let (year, period_index) = period_sort_key(title, &document.url)?;
    let period_type = match period_index {
        1 => "Q1",
        2 => "H1",
        3 => "Q3",
        4 => "FY",
        // Unknown intra-year period (0) — never guess.
        _ => return None,
    };
    let period_end = match period_type {
        "Q1" => format!("{year}-03-31"),
        "H1" => format!("{year}-06-30"),
        "Q3" => format!("{year}-09-30"),
        _ => format!("{year}-12-31"),
    };
    Some((i64::from(year), period_type, period_end))
}

/// Whether a stored document should be resolved through the ESEF/iXBRL tier
/// rather than the PDF tier: a bare `.xhtml`/`.html` instance, or an ESEF report
/// *package* (`.xbri`/`.zip`, ADR 0061 dec. 1). Extension/content-type only —
/// no byte read — so callers can decide whether to load the file at all. A
/// mislabeled package (generic `application/octet-stream`, no telltale
/// extension) is still caught later by the ZIP-magic sniff in
/// [`esef_instance_bytes`].
pub(crate) fn is_esef_route(content_type: Option<&str>, local_path: &str) -> bool {
    if SourceFormat::resolve(content_type, local_path) == SourceFormat::Xhtml {
        return true;
    }
    let lower = local_path.to_ascii_lowercase();
    lower.ends_with(".xbri") || lower.ends_with(".zip")
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
    let format = SourceFormat::resolve(
        document.content_type.as_deref(),
        document.local_path.as_deref().unwrap_or(""),
    );
    if format != SourceFormat::Xhtml {
        return None;
    }
    let target_period = derive_report_period(state, document).map(|(_, _, end)| end)?;
    let siblings = state
        .list_report_documents_by_company(&document.company_id)
        .ok()?;
    siblings.into_iter().find(|sibling| {
        sibling.id != document.id
            && is_fetched_periodic(sibling)
            && SourceFormat::resolve(
                sibling.content_type.as_deref(),
                sibling.local_path.as_deref().unwrap_or(""),
            ) == SourceFormat::Pdf
            && derive_report_period(state, sibling).map(|(_, _, end)| end)
                == Some(target_period.clone())
    })
}

/// The inline-XBRL **instance** bytes for a stored document, if it is (or
/// contains) one — the single seam shared by [`derive_report_period`] and
/// [`run_structured_extraction`] so the on-demand button and autopilot resolve
/// the ESEF tier identically (ADR 0061 dec. 1). A bare `.xhtml`/`.html` returns
/// its own bytes; an ESEF report package (`.xbri`/`.zip`, or any ZIP by magic)
/// is unpacked to its inner `reports/` instance. `None` for a PDF, or a package
/// with no readable instance.
fn esef_instance_bytes(
    content_type: Option<&str>,
    local_path: &str,
    bytes: &[u8],
) -> Option<Vec<u8>> {
    use crate::fundamentals::extraction::esef_package;
    if esef_package::is_report_package(local_path, bytes) {
        return esef_package::extract_instance(bytes);
    }
    match SourceFormat::resolve(content_type, local_path) {
        SourceFormat::Xhtml => Some(bytes.to_vec()),
        SourceFormat::Pdf => None,
    }
}

/// Runs the structured pipeline for one report document and persists the
/// result. `mode` is the run's trust-ladder mode (`MODE_AUTOPILOT` /
/// `MODE_ASSIST`) — it drives the per-fact `confirmation_state` via
/// [`confirmation_state_for`], not a caller-chosen literal, so a validated set
/// auto-confirms consistently regardless of who calls this.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_structured_extraction(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    mode: &str,
    tier4: Tier4Gate,
) -> Result<StructuredExtractionResult, String> {
    // --- Load the document bytes + format -------------------------------
    let document = state
        .get_report_document(report_document_id)
        .map_err(|e| e.to_string())?;
    let local_path = document
        .local_path
        .ok_or_else(|| "the report document has no stored file".to_owned())?;
    let path = state.data_dir().join(&local_path);
    let bytes = std::fs::read(&path).map_err(|e| format!("failed to read report file: {e}"))?;

    // --- Tier-3b positional route (ADR 0077 T-B2) -----------------------
    // A bare XHTML that is NOT valid iXBRL is a pdf2htmlEX visual render: no ESEF
    // tier can read it (no `ix:` tags) and it is not a PDF for the text/vision
    // tiers. Route it to the deterministic positional parser instead of letting it
    // fall through the ESEF tier as not-extractable. `.xbri`/`.zip` packages
    // (which `is_esef_route` also covers) are NOT bare XHTML and stay on the ESEF
    // package path. The sniff shares `esef::is_inline_xbrl` with the history
    // sweep, so routing and extractability can never disagree.
    if SourceFormat::resolve(document.content_type.as_deref(), &local_path) == SourceFormat::Xhtml {
        let prefix = &bytes[..bytes.len().min(64 * 1024)];
        if !crate::fundamentals::extraction::esef::is_inline_xbrl(prefix) {
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
    }

    // --- Build the pipeline input ---------------------------------------
    let profile = state
        .fundamentals_provenance()
        .get_profile(company_id)
        .map_err(|e| e.to_string())?;

    // ESEF tier gets the inline-XBRL instance bytes — the document's own bytes
    // for a bare `.xhtml`, or the inner instance unpacked from a `.xbri`/`.zip`
    // report package (ADR 0061 dec. 1). Everything else is the PDF tier.
    let (esef_opt, pdf_opt): (Option<Vec<u8>>, Option<String>) =
        match esef_instance_bytes(document.content_type.as_deref(), &local_path, &bytes) {
            Some(instance) => (Some(instance), None),
            None => {
                let extracted = extract_report(&bytes, SourceFormat::Pdf);
                let text = extracted
                    .sections
                    .iter()
                    .map(|s| s.body.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                (None, Some(text))
            }
        };

    // --- Comparative cross-check + completeness inputs (ADR 0061 dec. 4b/4d) --
    let prior_end = prior_period_end(period_end);
    let stored_prior = state
        .financials()
        .stored_fact_set(company_id, fiscal_year - 1, period_type)
        .map_err(|e| e.to_string())?;
    let expected_keys = expected_primary_keys(state, company_id)?;

    let input = PipelineInput {
        period_end,
        esef_bytes: esef_opt.as_deref(),
        pdf_text: pdf_opt.as_deref(),
        profile: profile.as_ref(),
        prior: stored_prior.as_ref(),
        prior_period_end: prior_end.as_deref(),
        expected_keys: expected_keys.as_ref(),
        witness: None,
    };
    let outcome = run_pipeline(&input);

    let drift_json = outcome
        .drift
        .as_ref()
        .and_then(|d| serde_json::to_string(d).ok());
    let structure_changed = drift_json.is_some();

    // --- Persist accepted facts + provenance ----------------------------
    let mut produced_fact_ids = Vec::new();
    let mut skipped_fact_ids = Vec::new();
    let mut divergences = Vec::new();
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
        let mut seen_keys = BTreeSet::new();
        for fact in &outcome.facts {
            if !seen_keys.insert(fact.metric_key.clone()) {
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

        // Learn the PDF layout on a clean accept: bootstrap or merge the
        // per-company profile so the next period parses zero-touch.
        if outcome.tier == Some(SourceTier::Pdf) && outcome.acceptance == Acceptance::Accepted {
            if let Some(text) = pdf_opt.as_deref() {
                let parse = parse_pdf_text(text, period_end, profile.as_ref());
                let learned = match &profile {
                    Some(existing) => existing.merge_confirmed(&parse),
                    None => ExtractionProfile::bootstrap(company_id, &parse),
                };
                let _ = state.fundamentals_provenance().upsert_profile(&learned);
            }
        }
    }

    // --- Tier-4 (OCR) fallback (ADR 0077 §4) ----------------------------
    // Only when determinism ended Flagged|Empty (`!emits()`) AND the caller gate
    // permits it. Lives here in the job layer, never in pure `pipeline.rs`. Its
    // facts fold into `produced_fact_ids` (they are persisted through the same
    // slot-aware `record_structured_fact`); its proposals land in the existing
    // confirm substrate via a synthetic completed job. A transient provider
    // failure propagates as `Err` so the queue's backoff retry engages; a
    // terminal one degrades with an honest reason.
    let mut tier4_reason: Option<String> = None;
    let mut tier4_proposals = 0usize;
    let mut tier4_effective: Option<(Acceptance, SourceTier)> = None;
    if !outcome.acceptance.emits() && tier4 == Tier4Gate::Allowed {
        match run_tier4_extraction(
            state,
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
            mode,
        ) {
            Ok(t4) => {
                tier4_reason = Some(t4.reason.clone());
                if let Some(acceptance) = t4.acceptance {
                    tier4_effective = Some((acceptance, SourceTier::AiText));
                }
                // Facts and proposals are mutually exclusive (an emitting profile
                // parse yields facts; a bootstrap / validation-failing parse yields
                // proposals) — so branch rather than partially move `t4`.
                if t4.proposals.is_empty() {
                    produced_fact_ids.extend(t4.produced_fact_ids);
                } else {
                    tier4_proposals = t4.proposals.len();
                    persist_tier4_proposals(state, company_id, report_document_id, t4)?;
                }
            }
            Err((code, message)) => {
                if is_transient_provider_code(code) {
                    return Err(format!(
                        "tier-4 extraction transient failure ({code}): {message}"
                    ));
                }
                let reason = format!("provider_error:{code}");
                warn_tier4_degrade(&reason, company_id, report_document_id, None, None);
                tier4_reason = Some(reason);
            }
        }
    }

    // When tier-4 emitted facts, the honest final outcome is its `ai_text` verdict
    // (the structured tiers only flagged/emptied); otherwise keep the structured
    // outcome so a flagged/empty run still reports its tier + acceptance.
    let (final_acceptance, final_tier) = match tier4_effective {
        Some((acceptance, tier)) => (acceptance, Some(tier)),
        None => (outcome.acceptance, outcome.tier),
    };

    Ok(StructuredExtractionResult {
        acceptance: final_acceptance,
        tier: final_tier,
        emitted: !produced_fact_ids.is_empty(),
        produced_fact_ids,
        skipped_fact_ids,
        divergences,
        drift_json,
        structure_changed,
        tier4: tier4_reason,
        tier4_proposals,
    })
}

/// Tier-3b positional route (ADR 0077 T-B2): parse a non-iXBRL pdf2htmlEX XHTML
/// render with the deterministic positional parser, run its facts through the
/// **same** [`validate_parsed_set`] identity gate every tier uses, and persist a
/// clean set under `source_tier='pdf'` with the identifiable
/// `extraction_method='html_positional'` provenance marker. A flagged/empty set
/// emits nothing (like any deterministic tier) — never `validation_status='none'`
/// (G-1). Tier-4 is skipped for this path (the render is not a PDF; a sweep run
/// degrades `not_pdf` upstream, spending zero budget), so this returns directly.
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
        .stored_fact_set(company_id, fiscal_year - 1, period_type)
        .map_err(|e| e.to_string())?;
    let expected_keys = expected_primary_keys(state, company_id)?;
    let (acceptance, _status) = validate_parsed_set(
        &facts,
        period_end,
        stored_prior.as_ref(),
        prior_end.as_deref(),
        expected_keys.as_ref(),
    );

    let mut produced_fact_ids = Vec::new();
    let mut skipped_fact_ids = Vec::new();
    let mut divergences = Vec::new();
    if acceptance.emits() {
        let validation_status = acceptance.validation_status();
        let confirmation_state = confirmation_state_for(acceptance, mode);
        let store = state.kpi_extraction();
        let mut seen_keys = BTreeSet::new();
        for fact in &facts {
            if !seen_keys.insert(fact.metric_key.clone()) {
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

    Ok(StructuredExtractionResult {
        acceptance,
        tier: (!facts.is_empty()).then_some(SourceTier::Pdf),
        emitted: !produced_fact_ids.is_empty(),
        produced_fact_ids,
        skipped_fact_ids,
        divergences,
        drift_json: None,
        structure_changed: false,
        tier4: None,
        tier4_proposals: 0,
    })
}

/// Transient (queue-retryable) provider error codes — availability failures only,
/// mirroring [`crate::providers::analysis::AnalysisProviderError::is_availability_error`]
/// (the same split `jobs::kpi_extraction` pins). A transient tier-4 failure must
/// propagate so the queue's capped backoff retries; every other code degrades.
fn is_transient_provider_code(code: &str) -> bool {
    matches!(
        code,
        "provider_limit" | "provider_unavailable" | "network_error"
    )
}

/// The tier-4 (OCR) attempt's outcome, folded into a [`StructuredExtractionResult`]
/// by [`run_structured_extraction`] and translated into a [`CompletedKpiExtraction`]
/// by the rewired manual job (T4.5). Either facts were emitted (validated OCR
/// parse through a confirmed profile) or proposals were produced (bootstrap, or a
/// validation-failing parse) — never both.
#[derive(Debug)]
pub(crate) struct Tier4Outcome {
    /// Honest state string (see [`StructuredExtractionResult::tier4`]).
    pub reason: String,
    /// The acceptance verdict when facts were emitted (drives the confirmation
    /// state + the caller's honest auto-confirm count); `None` for proposals or a
    /// degradation.
    pub acceptance: Option<Acceptance>,
    /// Ids of the facts this run created (empty for bootstrap/proposals/degrade).
    pub produced_fact_ids: Vec<String>,
    /// Proposals to land in the confirm substrate (empty when facts were emitted).
    pub proposals: Vec<NewKpiProposal>,
    pub detected_fiscal_year: Option<i64>,
    pub detected_period_type: Option<String>,
    pub detected_period_end_date: Option<String>,
    pub detected_currency: Option<String>,
    pub detected_language: Option<String>,
}

impl Tier4Outcome {
    /// A degradation with no facts/proposals — tier-4 could not run for a benign,
    /// terminal reason (no vision provider, a non-PDF document, an unbuildable
    /// pool). The run still succeeds deterministically-quiet.
    fn degraded(reason: &str) -> Self {
        Self {
            reason: reason.to_owned(),
            acceptance: None,
            produced_fact_ids: Vec::new(),
            proposals: Vec::new(),
            detected_fiscal_year: None,
            detected_period_type: None,
            detected_period_end_date: None,
            detected_currency: None,
            detected_language: None,
        }
    }
}

/// Emit one production-visible warning per tier-4 degradation (ADR 0077 §4
/// annotation 2026-07-10) — the trail an operator reads in Diagnostics → Logs when
/// a period comes back empty. The logger redacts every message string via
/// `redact_text` (logging.rs), which only masks sensitive `key=value` tokens, so a
/// bootstrap response prefix is safe to include. `ocr_len` is the OCR markdown
/// length when an OCR response already exists at the degrade point (`None` for a
/// degrade before OCR runs); `response_prefix` carries a ~200-char LLM-response
/// prefix, passed only for `bootstrap_failed`.
fn warn_tier4_degrade(
    reason: &str,
    company_id: &str,
    report_document_id: &str,
    ocr_len: Option<usize>,
    response_prefix: Option<&str>,
) {
    let ocr = ocr_len
        .map(|len| format!(" ocr_markdown_len={len}"))
        .unwrap_or_default();
    let response = response_prefix
        .map(|text| {
            let prefix: String = text.chars().take(200).collect();
            format!(" llm_response_prefix={prefix:?}")
        })
        .unwrap_or_default();
    log::warn!(
        "tier-4 degraded: reason={reason} company_id={company_id} report_document_id={report_document_id}{ocr}{response}"
    );
}

/// Runs the tier-4 OCR fallback for one report document (ADR 0077 §4). Resolves
/// the `VisionExtraction` pool, OCRs the stored PDF, and either parses it
/// deterministically through a confirmed [`OcrExtractionProfile`] (→ validated
/// facts, or proposals when validation fails) or, for a never-bootstrapped
/// company, bootstraps the profile with the text-LLM (labels only) and lands ALL
/// output as proposals (a bootstrap run **never** writes facts). `Err` carries a
/// provider error `(code, message)`; the caller propagates transient codes and
/// degrades terminal ones.
pub(crate) fn run_tier4_extraction(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    mode: &str,
) -> Result<Tier4Outcome, (&'static str, String)> {
    let document = state
        .get_report_document(report_document_id)
        .map_err(|e| ("provider_error", e.to_string()))?;
    let Some(local_path) = document.local_path.clone() else {
        warn_tier4_degrade("no_stored_file", company_id, report_document_id, None, None);
        return Ok(Tier4Outcome::degraded("no_stored_file"));
    };
    // Tier-4 is PDF-only: an ESEF/iXBRL instance or report package must never
    // reach the OCR path (the ESEF tier owns those). Degrade honestly.
    if is_esef_route(document.content_type.as_deref(), &local_path) {
        warn_tier4_degrade("not_pdf", company_id, report_document_id, None, None);
        return Ok(Tier4Outcome::degraded("not_pdf"));
    }

    // The `VisionExtraction` pool routes ONLY to an explicitly-configured
    // document-native provider (Mistral OCR) — never the general-analysis
    // fallback (Gemini was rejected for tier-4, ADR 0077 verdict; it cannot OCR).
    let settings = state
        .get_settings()
        .map_err(|e| ("provider_error", e.to_string()))?;
    let has_vision = settings
        .capability_providers
        .get(AiCapability::VisionExtraction.key())
        .is_some_and(|entries| !entries.is_empty());
    if !has_vision {
        warn_tier4_degrade(
            "no_vision_provider",
            company_id,
            report_document_id,
            None,
            None,
        );
        return Ok(Tier4Outcome::degraded("no_vision_provider"));
    }
    let timeout = settings.ai_providers.general_analysis_timeout_seconds;
    let provider = match crate::jobs::build_capability_provider(
        state,
        AiCapability::VisionExtraction,
        timeout,
    ) {
        Ok(provider) => provider,
        Err(error) => {
            log::warn!("tier-4: vision provider unbuildable for {company_id}: {error}");
            return Ok(Tier4Outcome::degraded("no_vision_provider"));
        }
    };

    let bytes = std::fs::read(state.data_dir().join(&local_path))
        .map_err(|e| ("provider_error", format!("failed to read report file: {e}")))?;
    // Tier-4 only reaches here for a PDF (the ESEF route degraded above), so the
    // OCR data-URL MIME is `application/pdf` regardless of a generic stored
    // content type (servers often deliver report PDFs as octet-stream).
    let mime_type = "application/pdf";

    tier4_with_provider(
        state,
        provider.as_ref(),
        company_id,
        report_document_id,
        &bytes,
        mime_type,
        fiscal_year,
        period_type,
        period_end,
        mode,
    )
}

/// The provider-driven core of tier-4 (OCR → profile parse / bootstrap → persist),
/// split out so tests inject a scripted provider without a live pool. See
/// [`run_tier4_extraction`] for the contract.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tier4_with_provider(
    state: &AppState,
    provider: &dyn AiAnalysisProvider,
    company_id: &str,
    report_document_id: &str,
    bytes: &[u8],
    mime_type: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    mode: &str,
) -> Result<Tier4Outcome, (&'static str, String)> {
    let markdown = tauri::async_runtime::block_on(provider.ocr_document(bytes, mime_type))
        .map_err(|error| (error.code(), error.to_string()))?;
    let period = FactPeriod::Instant(period_end.to_owned());

    let profile = state
        .fundamentals_provenance()
        .get_ocr_profile(company_id)
        .map_err(|e| ("provider_error", e.to_string()))?;

    match profile {
        // ADR 0077 §4 kickoff decision 3: only a CONFIRMED profile (version ≥ 2 —
        // the version bumps when the user confirms the bootstrap's first
        // proposals) may commit facts. A version-1 profile is a stored bootstrap
        // PROPOSAL: parse with it, but keep landing proposals — a plausible-but-
        // wrong bootstrap map that happens to satisfy the identities must never
        // commit facts no one reviewed.
        Some(profile) if profile.version >= 2 => tier4_profile_path(
            state,
            &markdown,
            &profile,
            &period,
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
            mode,
        ),
        Some(profile) => {
            let facts = parse_ocr_markdown(&markdown, &profile, &period, SourceTier::AiText);
            let proposals = facts
                .iter()
                .map(|fact| tier4_proposal(fact, "ocr_pending_profile"))
                .collect();
            Ok(Tier4Outcome {
                reason: "proposals_pending_profile".to_owned(),
                acceptance: None,
                produced_fact_ids: Vec::new(),
                proposals,
                detected_fiscal_year: Some(fiscal_year),
                detected_period_type: Some(period_type.to_owned()),
                detected_period_end_date: Some(period_end.to_owned()),
                detected_currency: None,
                detected_language: None,
            })
        }
        None => tier4_bootstrap_path(
            state,
            provider,
            &markdown,
            &period,
            company_id,
            report_document_id,
            fiscal_year,
            period_type,
            period_end,
        ),
    }
}

/// Tier-4 profile path: parse the OCR markdown through the confirmed profile,
/// validate through the same gate every tier uses, and either persist validated
/// facts (`source_tier='ai'`, confirmation state per `mode`) or route the
/// validation-failing values to proposals — never a silent emit.
#[allow(clippy::too_many_arguments)]
fn tier4_profile_path(
    state: &AppState,
    markdown: &str,
    profile: &OcrExtractionProfile,
    period: &FactPeriod,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    mode: &str,
) -> Result<Tier4Outcome, (&'static str, String)> {
    let facts = parse_ocr_markdown(markdown, profile, period, SourceTier::AiText);
    if facts.is_empty() {
        warn_tier4_degrade(
            "empty",
            company_id,
            report_document_id,
            Some(markdown.len()),
            None,
        );
        return Ok(Tier4Outcome::degraded("empty"));
    }

    // Comparative cross-check + completeness inputs — the SAME the structured
    // pipeline assembles (magnitude/prior cross-check catches a 1000× mis-scale).
    let prior_end = prior_period_end(period_end);
    let stored_prior = state
        .financials()
        .stored_fact_set(company_id, fiscal_year - 1, period_type)
        .map_err(|e| ("provider_error", e.to_string()))?;
    let expected_keys =
        expected_primary_keys(state, company_id).map_err(|e| ("provider_error", e))?;

    let (acceptance, _status) = validate_parsed_set(
        &facts,
        period_end,
        stored_prior.as_ref(),
        prior_end.as_deref(),
        expected_keys.as_ref(),
    );

    if acceptance.emits() {
        let validation_status = acceptance.validation_status();
        let confirmation_state = confirmation_state_for(acceptance, mode);
        let store = state.kpi_extraction();
        let mut produced_fact_ids = Vec::new();
        for fact in &facts {
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
                    // Tier-4 facts persist under the honest `ai` tier; the OCR
                    // label read is the provenance citation (ADR 0077 dec. 2). The
                    // `source_tier`/`validation_status` carry the trust; the
                    // `extraction_method` marker stays `api` (unchanged) — the
                    // positional path is the only one that overrides it (T-B2).
                    source_tier: "ai",
                    extraction_method: "api",
                    validation_status,
                    drift_json: None,
                    citation: Some(&fact.citation),
                })
                .map_err(|e| ("provider_error", e.to_string()))?;
            if let crate::storage::StructuredFactCommit::Created(id) = commit {
                produced_fact_ids.push(id);
            }
        }
        Ok(Tier4Outcome {
            reason: "facts_emitted".to_owned(),
            acceptance: Some(acceptance),
            produced_fact_ids,
            proposals: Vec::new(),
            detected_fiscal_year: Some(fiscal_year),
            detected_period_type: Some(period_type.to_owned()),
            detected_period_end_date: Some(period_end.to_owned()),
            detected_currency: None,
            detected_language: None,
        })
    } else {
        // Validation failed (a contradicted identity / mis-scale): never a fact —
        // route the values to the confirm flow as proposals.
        let proposals = facts
            .iter()
            .map(|fact| tier4_proposal(fact, "ocr_flagged"))
            .collect();
        Ok(Tier4Outcome {
            reason: "proposals_flagged".to_owned(),
            acceptance: None,
            produced_fact_ids: Vec::new(),
            proposals,
            detected_fiscal_year: Some(fiscal_year),
            detected_period_type: Some(period_type.to_owned()),
            detected_period_end_date: Some(period_end.to_owned()),
            detected_currency: None,
            detected_language: None,
        })
    }
}

/// Tier-4 bootstrap path (ADR 0077 §4 kickoff decision 3): the text-LLM proposes
/// the profile (labels only), which is stored as version 1, then the markdown is
/// parsed through it and ALL output lands as proposals. A bootstrap run NEVER
/// writes facts — the LLM proposes the MAP, never reads numbers into facts.
#[allow(clippy::too_many_arguments)]
fn tier4_bootstrap_path(
    state: &AppState,
    provider: &dyn AiAnalysisProvider,
    markdown: &str,
    period: &FactPeriod,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
) -> Result<Tier4Outcome, (&'static str, String)> {
    let known_kpis = known_kpi_catalog(state, company_id).map_err(|e| ("provider_error", e))?;
    let prompt = ocr_profile_bootstrap_prompt(&known_kpis);
    let response = tauri::async_runtime::block_on(provider.complete_document(
        &prompt,
        &AnalysisDocument::Text {
            text: markdown.to_owned(),
        },
    ))
    .map_err(|error| (error.code(), error.to_string()))?;

    let Some(profile) = OcrExtractionProfile::from_bootstrap_json(company_id, &response) else {
        // The one degrade that carries the LLM response prefix: bootstrap failed
        // because the text-LLM's profile JSON did not parse — the prefix is what an
        // operator needs to see why (ADR 0077 §4 annotation).
        warn_tier4_degrade(
            "bootstrap_failed",
            company_id,
            report_document_id,
            Some(markdown.len()),
            Some(&response),
        );
        return Ok(Tier4Outcome::degraded("bootstrap_failed"));
    };
    state
        .fundamentals_provenance()
        .upsert_ocr_profile(&profile)
        .map_err(|e| ("provider_error", e.to_string()))?;

    let facts = parse_ocr_markdown(markdown, &profile, period, SourceTier::AiText);
    let proposals = facts
        .iter()
        .map(|fact| tier4_proposal(fact, "ocr_bootstrap"))
        .collect();
    Ok(Tier4Outcome {
        reason: "bootstrap_proposals".to_owned(),
        acceptance: None,
        produced_fact_ids: Vec::new(),
        proposals,
        detected_fiscal_year: Some(fiscal_year),
        detected_period_type: Some(period_type.to_owned()),
        detected_period_end_date: Some(period_end.to_owned()),
        detected_currency: None,
        detected_language: None,
    })
}

/// One tier-4 proposal from a parsed OCR fact. `citation` marks its origin
/// (`ocr_bootstrap` / `ocr_flagged`) in the source-snippet trail.
fn tier4_proposal(fact: &ExtractedFact, citation: &str) -> NewKpiProposal {
    NewKpiProposal {
        metric_key: fact.metric_key.clone(),
        label: fact.citation.clone(),
        value_numeric: fact.value.to_string(),
        unit: None,
        currency: fact.currency.clone(),
        as_reported_value: None,
        as_reported_scale: None,
        measure_window: None,
        confidence: None,
        source_snippet: Some(format!("{citation}: {}", fact.citation)),
        is_proposed_kpi: false,
    }
}

/// The company's applicable KPI catalog (canonical + company-scoped, no derived
/// metrics) as bootstrap-prompt grounding — the same filter the text-AI KPI job
/// used, so the label map targets real catalog keys.
fn known_kpi_catalog(state: &AppState, company_id: &str) -> Result<Vec<KpiCatalogEntry>, String> {
    Ok(state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|d| d.computation != "derived")
        .filter(|d| {
            d.scope == "canonical"
                || (d.scope == "company" && d.company_id.as_deref() == Some(company_id))
        })
        .map(|d| KpiCatalogEntry {
            metric_key: d.metric_key,
            label: d.label,
            value_kind: d.value_kind,
            unit: d.unit,
        })
        .collect())
}

/// Lands tier-4 proposals in the existing confirm substrate via a synthetic
/// completed KPI-extraction job (the substrate keys proposals by a job row). Used
/// by the structured hook, which has no pre-existing job — the rewired manual job
/// (T4.5) completes its own job instead.
fn persist_tier4_proposals(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    outcome: Tier4Outcome,
) -> Result<(), String> {
    let job = state
        .create_kpi_extraction_job(NewKpiExtractionJob {
            company_id: company_id.to_owned(),
            report_document_id: report_document_id.to_owned(),
            provider_id: CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
            model: CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
            prompt_version: OCR_PROFILE_BOOTSTRAP_PROMPT_VERSION.to_owned(),
            period_hint: None,
        })
        .map_err(|e| e.to_string())?;
    state
        .complete_kpi_extraction_job(CompletedKpiExtraction {
            job_id: job.id,
            detected_fiscal_year: outcome.detected_fiscal_year,
            detected_period_type: outcome.detected_period_type,
            detected_period_end_date: outcome.detected_period_end_date,
            detected_currency: outcome.detected_currency,
            detected_language: outcome.detected_language,
            proposals: outcome.proposals,
            committed_fact_count: 0,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{
        open_in_memory_database, CaptureReportDocumentInput, NewCompany, NewFinancialFact,
        NewFinancialPeriod, NewKpiRelevance, MODE_ASSIST,
    };

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
            Tier4Gate::DeniedSweep,
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
            Tier4Gate::DeniedSweep,
        )
        .expect("positional extraction runs");

        assert_eq!(
            result.tier,
            Some(SourceTier::Pdf),
            "reuses the deterministic Pdf tier"
        );
        assert!(result.emitted, "the positional render emits facts");
        assert_eq!(
            result.tier4, None,
            "tier-4 is never invoked for the XHTML render"
        );
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
            Tier4Gate::DeniedSweep,
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
            Tier4Gate::DeniedSweep,
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
            Tier4Gate::DeniedSweep,
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
            Tier4Gate::DeniedSweep,
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
                Tier4Gate::DeniedSweep,
            )
            .expect("structured extraction runs");

            assert!(result.emitted, "ESEF facts should be emitted");
            assert_eq!(result.tier, Some(SourceTier::Esef));
            assert_eq!(result.acceptance, Acceptance::Accepted);
            assert_eq!(result.produced_fact_ids.len(), 3);
            assert!(!result.structure_changed);
            assert_eq!(result.drift_json, None);

            // Every produced fact carries structured provenance: tier + passed status.
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
            Tier4Gate::DeniedSweep,
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
            Tier4Gate::DeniedSweep,
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

    #[test]
    fn pdf_with_mismatching_stored_prior_cross_check_is_flagged() {
        let (state, company_id, document_id) = seed_pdf(&[
            "Total Assets Line 45 000  40 000",
            "Total Liabilities Line 20 000  18 000",
            "Total Equity Line 25 000  22 000",
        ]);
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");
        // Stored prior disagrees with the report's own comparative column
        // (40m) — a column-misalignment signal — and no witness is wired
        // (ADR 0061 decision 4), so it must flag, never emit.
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
            "2026-12-31",
            MODE_AUTOPILOT,
            Tier4Gate::DeniedSweep,
        )
        .expect("structured extraction runs");

        assert_eq!(result.acceptance, Acceptance::Flagged);
        assert!(!result.emitted, "a flagged cross-check must not emit facts");
        assert!(result.produced_fact_ids.is_empty());
    }

    #[test]
    fn pdf_zero_overlap_expected_keys_downgrades_to_accepted_unreviewed() {
        let (state, company_id, document_id) = seed_pdf(&[
            "Total Assets Line 45 000",
            "Total Liabilities Line 20 000",
            "Total Equity Line 25 000",
        ]);
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");

        // Mark "revenue" as the company's primary KPI — absent from this
        // report, so completeness is zero-overlap despite the balance sheet
        // validating cleanly.
        let definitions = state
            .list_kpi_definitions(ListKpiDefinitionsInput {
                scope: Some("canonical".to_owned()),
                sector: None,
                company_id: None,
            })
            .expect("canonical definitions should list");
        let revenue_def = definitions
            .iter()
            .find(|d| d.metric_key == "revenue")
            .expect("revenue should exist in the canonical catalog");
        state
            .create_kpi_relevance(NewKpiRelevance {
                company_id: company_id.clone(),
                definition_id: revenue_def.id.clone(),
                source: "user".to_owned(),
                rank: Some("primary".to_owned()),
                first_seen_period: None,
                last_seen_period: None,
            })
            .expect("kpi relevance should create");

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-12-31",
            MODE_AUTOPILOT,
            Tier4Gate::DeniedSweep,
        )
        .expect("structured extraction runs");

        assert_eq!(result.acceptance, Acceptance::AcceptedUnreviewed);
        assert!(result.emitted, "a downgrade must never block emission");
        assert_eq!(result.produced_fact_ids.len(), 3);
        let provenance = state
            .fundamentals_provenance()
            .get_many(&result.produced_fact_ids)
            .expect("provenance");
        assert!(provenance
            .iter()
            .all(|p| p.validation_status == "unreviewed"));
        let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
        assert!(
            states.iter().all(|s| s == "auto_unreviewed"),
            "states={states:?}"
        );
    }

    #[test]
    fn pdf_overlapping_expected_keys_keeps_full_accepted() {
        let (state, company_id, document_id) = seed_pdf(&[
            "Total Assets Line 45 000",
            "Total Liabilities Line 20 000",
            "Total Equity Line 25 000",
        ]);
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");

        let definitions = state
            .list_kpi_definitions(ListKpiDefinitionsInput {
                scope: Some("canonical".to_owned()),
                sector: None,
                company_id: None,
            })
            .expect("canonical definitions should list");
        let total_assets_def = definitions
            .iter()
            .find(|d| d.metric_key == "total_assets")
            .expect("total_assets should exist in the canonical catalog");
        state
            .create_kpi_relevance(NewKpiRelevance {
                company_id: company_id.clone(),
                definition_id: total_assets_def.id.clone(),
                source: "user".to_owned(),
                rank: Some("primary".to_owned()),
                first_seen_period: None,
                last_seen_period: None,
            })
            .expect("kpi relevance should create");

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-12-31",
            MODE_AUTOPILOT,
            Tier4Gate::DeniedSweep,
        )
        .expect("structured extraction runs");

        assert_eq!(result.acceptance, Acceptance::Accepted);
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
            Tier4Gate::DeniedSweep,
        );
        assert!(err.is_err());
    }

    fn seed_pdf(lines: &[&str]) -> (AppState, String, String) {
        let dir = unique_temp_dir("pdf");
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
                url: "https://example.com/annual-2026.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Annual 2026 PDF".to_owned()),
                attribution: None,
            })
            .expect("document");
        let bytes = minimal_text_pdf(lines);
        std::fs::write(dir.join("annual.pdf"), &bytes).expect("write pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("annual.pdf"),
                Some("application/pdf"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    /// A profile with ASCII-only labels so the fixture never has to round-trip
    /// Polish diacritics through the hand-built PDF (a real profile's labels
    /// are read out of the actual PDF; this test cares only about the
    /// pipeline's acceptance/confirmation/drift plumbing, not label matching).
    fn ascii_profile(company_id: &str, labels: &[(&str, &str)]) -> ExtractionProfile {
        let label_map = labels
            .iter()
            .map(|(label, metric)| (label.to_string(), metric.to_string()))
            .collect();
        ExtractionProfile {
            company_id: company_id.to_owned(),
            template_hash: "test-template".to_owned(),
            unit_scale: crate::fundamentals::extraction::pdf::UnitScale::Thousands,
            label_map,
            version: 1,
        }
    }

    #[test]
    fn pdf_extraction_with_clean_profile_match_confirms_in_both_modes() {
        for mode in [MODE_ASSIST, MODE_AUTOPILOT] {
            let (state, company_id, document_id) = seed_pdf(&[
                "Total Assets Line 45 000",
                "Total Liabilities Line 20 000",
                "Total Equity Line 25 000",
            ]);
            let profile = ascii_profile(
                &company_id,
                &[
                    ("total assets line", "total_assets"),
                    ("total liabilities line", "total_liabilities"),
                    ("total equity line", "total_equity"),
                ],
            );
            state
                .fundamentals_provenance()
                .upsert_profile(&profile)
                .expect("seed profile");

            let result = run_structured_extraction(
                &state,
                &company_id,
                &document_id,
                2026,
                "FY",
                "2026-12-31",
                mode,
                Tier4Gate::DeniedSweep,
            )
            .expect("structured extraction runs");

            assert!(result.emitted, "a balanced PDF parse should emit facts");
            assert_eq!(result.tier, Some(SourceTier::Pdf));
            assert_eq!(result.acceptance, Acceptance::Accepted);
            assert_eq!(result.produced_fact_ids.len(), 3);
            assert!(!result.structure_changed, "a matching profile has no drift");

            let provenance = state
                .fundamentals_provenance()
                .get_many(&result.produced_fact_ids)
                .expect("provenance");
            assert!(provenance.iter().all(|p| p.source_tier == "pdf"));
            assert!(provenance.iter().all(|p| p.validation_status == "passed"));

            let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
            assert!(
                states.iter().all(|s| s == "confirmed"),
                "mode={mode} states={states:?}"
            );
        }
    }

    #[test]
    fn pdf_extraction_with_no_identity_evidence_follows_trust_ladder_by_mode() {
        // Only a P&L line is present (no balance-sheet triple) — every identity
        // is not-applicable, so validation is Inconclusive → AcceptedUnreviewed.
        // No profile is needed: "zysk netto" is an ASCII-safe default-dictionary
        // label, so this also covers the no-profile PDF path.
        let cases = [
            (MODE_AUTOPILOT, "auto_unreviewed"),
            (MODE_ASSIST, "pending"),
        ];
        for (mode, expected_state) in cases {
            let (state, company_id, document_id) = seed_pdf(&["Zysk netto 12 000"]);
            let result = run_structured_extraction(
                &state,
                &company_id,
                &document_id,
                2026,
                "FY",
                "2026-12-31",
                mode,
                Tier4Gate::DeniedSweep,
            )
            .expect("structured extraction runs");

            assert!(result.emitted, "an uncontradicted parse should still emit");
            assert_eq!(result.acceptance, Acceptance::AcceptedUnreviewed);
            assert_eq!(result.produced_fact_ids.len(), 1);
            assert!(!result.structure_changed);

            let provenance = state
                .fundamentals_provenance()
                .get_many(&result.produced_fact_ids)
                .expect("provenance");
            assert!(provenance
                .iter()
                .all(|p| p.validation_status == "unreviewed"));

            let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
            assert_eq!(states, vec![expected_state.to_owned()], "mode={mode}");
        }
    }

    #[test]
    fn pdf_profile_drift_flags_emits_nothing_but_reports_structure_changed() {
        // The confirmed profile expects three lines (assets/liabilities/equity);
        // the new report drops the equity line entirely — a real "the company
        // restructured its statement layout" scenario. No witness is wired
        // (ADR 0061 decision 4), so a drifted PDF is flagged, never silently
        // emitted or silently dropped: the caller gets the drift back to surface.
        let (state, company_id, document_id) =
            seed_pdf(&["Total Assets Line 45 000", "Total Liabilities Line 20 000"]);
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");

        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-12-31",
            MODE_AUTOPILOT,
            Tier4Gate::DeniedSweep,
        )
        .expect("structured extraction runs");

        assert!(!result.emitted, "a flagged drift must not emit facts");
        assert_eq!(result.acceptance, Acceptance::Flagged);
        assert!(result.produced_fact_ids.is_empty());
        assert!(
            result.structure_changed,
            "a dropped confirmed label is a structure change"
        );
        let drift_json = result.drift_json.expect("drift diff must be reported");
        assert!(
            drift_json.contains("total equity line"),
            "drift: {drift_json}"
        );
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

    // -------------------------------------------------------------------
    // Tier-4 (OCR) fallback (ADR 0077 §4)
    // -------------------------------------------------------------------

    use crate::fundamentals::extraction::ocr::{OcrExtractionProfile, ValueColumnLayout};
    use crate::fundamentals::extraction::pdf::UnitScale;
    use crate::providers::analysis::{
        AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest, DocumentSupport,
        ResearchBriefProviderOutput, ResearchBriefRequest, ResearchDigestRequest,
    };
    use async_trait::async_trait;

    /// A scripted vision provider (follows the pool.rs `ScriptedProvider` pattern):
    /// `ocr_document` returns a canned markdown or a scripted error, and
    /// `complete_document` returns a canned bootstrap JSON.
    struct MockVisionProvider {
        ocr_markdown: String,
        ocr_error: Option<&'static str>,
        bootstrap_json: String,
    }

    impl MockVisionProvider {
        fn ocr(markdown: &str) -> Self {
            Self {
                ocr_markdown: markdown.to_owned(),
                ocr_error: None,
                bootstrap_json: String::new(),
            }
        }
        fn with_bootstrap(markdown: &str, bootstrap_json: &str) -> Self {
            Self {
                ocr_markdown: markdown.to_owned(),
                ocr_error: None,
                bootstrap_json: bootstrap_json.to_owned(),
            }
        }
        fn ocr_failing(kind: &'static str) -> Self {
            Self {
                ocr_markdown: String::new(),
                ocr_error: Some(kind),
                bootstrap_json: String::new(),
            }
        }
    }

    #[async_trait]
    impl AiAnalysisProvider for MockVisionProvider {
        fn provider_id(&self) -> &'static str {
            "provider_mistral"
        }
        fn model(&self) -> &str {
            "mock-vision"
        }
        async fn analyze(
            &self,
            _request: &AnalysisRequest,
        ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
            Err(AnalysisProviderError::ProviderError("unused".to_owned()))
        }
        async fn generate_research_brief(
            &self,
            _request: &ResearchBriefRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            Err(AnalysisProviderError::ProviderError("unused".to_owned()))
        }
        async fn generate_research_digest(
            &self,
            _request: &ResearchDigestRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            Err(AnalysisProviderError::ProviderError("unused".to_owned()))
        }
        fn document_support(&self) -> DocumentSupport {
            DocumentSupport::Native
        }
        async fn ocr_document(
            &self,
            _bytes: &[u8],
            _mime_type: &str,
        ) -> Result<String, AnalysisProviderError> {
            match self.ocr_error {
                Some("limit") => Err(AnalysisProviderError::ProviderLimit),
                Some(_) => Err(AnalysisProviderError::ProviderError("scripted".to_owned())),
                None => Ok(self.ocr_markdown.clone()),
            }
        }
        async fn complete_document(
            &self,
            _prompt: &str,
            _document: &AnalysisDocument,
        ) -> Result<String, AnalysisProviderError> {
            Ok(self.bootstrap_json.clone())
        }
    }

    fn balance_sheet_profile(company_id: &str, equity_key: &str) -> OcrExtractionProfile {
        let label_map = std::collections::BTreeMap::from([
            ("aktywa razem".to_string(), "total_assets".to_string()),
            (
                "zobowiązania razem".to_string(),
                "total_liabilities".to_string(),
            ),
            ("kapitał własny".to_string(), equity_key.to_string()),
        ]);
        OcrExtractionProfile::bootstrap(
            company_id,
            UnitScale::Thousands,
            label_map,
            ValueColumnLayout::CurrentPeriodFirst,
            Vec::new(),
            false,
        )
    }

    /// A CONFIRMED (version-2) profile — the facts-path precondition (ADR 0077
    /// §4 kickoff decision 3: a v1 bootstrap only ever yields proposals).
    fn confirmed_balance_sheet_profile(company_id: &str, equity_key: &str) -> OcrExtractionProfile {
        let p = balance_sheet_profile(company_id, equity_key);
        p.clone().confirm(
            p.scale,
            p.label_map.clone(),
            p.value_column,
            p.skip_columns.clone(),
            p.strip_enumerators,
        )
    }

    const BALANCED_OCR: &str = "\
| Pozycja | 2025 |
| --- | --- |
| Aktywa razem | 45 000 |
| Zobowiązania razem | 20 000 |
| Kapitał własny | 25 000 |
";

    /// ADR 0077 §4 kickoff decision 3: a version-1 profile is a stored bootstrap
    /// PROPOSAL, not a confirmation — a human confirms it by accepting its first
    /// proposals. Until then tier-4 must keep landing proposals, never facts:
    /// a plausible-but-wrong bootstrap map that happens to satisfy the identities
    /// would otherwise commit facts no one ever reviewed.
    #[test]
    fn tier4_unconfirmed_profile_emits_proposals_not_facts() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        // Version 1 = fresh bootstrap, never confirmed.
        state
            .fundamentals_provenance()
            .upsert_ocr_profile(&balance_sheet_profile(&company_id, "total_equity"))
            .expect("seed ocr profile v1");
        let provider = MockVisionProvider::ocr(BALANCED_OCR);

        let outcome = tier4_with_provider(
            &state,
            &provider,
            &company_id,
            &document_id,
            b"%PDF",
            "application/pdf",
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 runs");

        assert_eq!(outcome.reason, "proposals_pending_profile");
        assert!(
            outcome.produced_fact_ids.is_empty(),
            "an unconfirmed profile must never commit facts"
        );
        assert_eq!(outcome.proposals.len(), 3);
        assert!(outcome.proposals.iter().all(|p| p
            .source_snippet
            .as_deref()
            .unwrap_or_default()
            .starts_with("ocr_pending_profile")));
    }

    /// T4.3-1: a confirmed profile parses the OCR markdown to a validation-clean
    /// set → tier-4 emits FACTS with `source_tier='ai'` and a real (`passed`)
    /// validation status.
    #[test]
    fn tier4_profile_path_emits_validated_facts_as_source_tier_ai() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        state
            .fundamentals_provenance()
            .upsert_ocr_profile(&confirmed_balance_sheet_profile(
                &company_id,
                "total_equity",
            ))
            .expect("seed ocr profile");
        let provider = MockVisionProvider::ocr(BALANCED_OCR);

        let outcome = tier4_with_provider(
            &state,
            &provider,
            &company_id,
            &document_id,
            b"%PDF",
            "application/pdf",
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 runs");

        assert_eq!(outcome.reason, "facts_emitted");
        assert_eq!(outcome.produced_fact_ids.len(), 3);
        assert!(outcome.proposals.is_empty());

        let provenance = state
            .fundamentals_provenance()
            .get_many(&outcome.produced_fact_ids)
            .expect("provenance");
        assert!(provenance.iter().all(|p| p.source_tier == "ai"));
        assert!(provenance.iter().all(|p| p.validation_status == "passed"));
    }

    /// T4.3-9: a second tier-4 run over the same document re-observes the slots —
    /// no duplicate facts (`record_structured_fact` idempotency holds).
    #[test]
    fn tier4_re_run_is_idempotent_no_duplicate_facts() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        state
            .fundamentals_provenance()
            .upsert_ocr_profile(&confirmed_balance_sheet_profile(
                &company_id,
                "total_equity",
            ))
            .expect("seed ocr profile");

        let run = || {
            tier4_with_provider(
                &state,
                &MockVisionProvider::ocr(BALANCED_OCR),
                &company_id,
                &document_id,
                b"%PDF",
                "application/pdf",
                2025,
                "FY",
                "2025-12-31",
                MODE_AUTOPILOT,
            )
            .expect("tier-4 runs")
        };
        assert_eq!(run().produced_fact_ids.len(), 3);
        let second = run();
        assert!(
            second.produced_fact_ids.is_empty(),
            "a re-observation produces no new facts"
        );
        let facts = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("facts");
        assert_eq!(facts.len(), 3, "no duplicate facts on re-run");
    }

    /// T4.3-3: a parsed set that violates the balance-sheet identity is NEVER a
    /// fact — the values route to proposals for the confirm flow.
    #[test]
    fn tier4_validation_failing_set_routes_to_proposals_never_facts() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        state
            .fundamentals_provenance()
            .upsert_ocr_profile(&confirmed_balance_sheet_profile(
                &company_id,
                "total_equity",
            ))
            .expect("seed ocr profile");
        // 45 != 20 + 30 → the balance-sheet identity fails.
        let unbalanced = "\
| Pozycja | 2025 |
| --- | --- |
| Aktywa razem | 45 000 |
| Zobowiązania razem | 20 000 |
| Kapitał własny | 30 000 |
";
        let outcome = tier4_with_provider(
            &state,
            &MockVisionProvider::ocr(unbalanced),
            &company_id,
            &document_id,
            b"%PDF",
            "application/pdf",
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 runs");

        assert_eq!(outcome.reason, "proposals_flagged");
        assert!(outcome.produced_fact_ids.is_empty());
        assert_eq!(outcome.proposals.len(), 3);
        let facts = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("facts");
        assert!(facts.is_empty(), "a validation-failing set writes no facts");
    }

    /// T4.3-2 (G-1 stays green): a never-bootstrapped company bootstraps the
    /// profile (labels only) and lands ALL output as PROPOSALS — ZERO facts.
    #[test]
    fn tier4_bootstrap_emits_proposals_only_and_stores_profile_v1() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        assert!(state
            .fundamentals_provenance()
            .get_ocr_profile(&company_id)
            .expect("profile read")
            .is_none());
        let bootstrap_json = r#"{"scale":"thousands","valueColumn":"current_period_first","labelMap":{"aktywa razem":"total_assets","zobowiązania razem":"total_liabilities","kapitał własny":"total_equity"}}"#;
        let provider = MockVisionProvider::with_bootstrap(BALANCED_OCR, bootstrap_json);

        let outcome = tier4_with_provider(
            &state,
            &provider,
            &company_id,
            &document_id,
            b"%PDF",
            "application/pdf",
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 runs");

        assert_eq!(outcome.reason, "bootstrap_proposals");
        assert!(
            outcome.produced_fact_ids.is_empty(),
            "a bootstrap run NEVER writes facts (G-1)"
        );
        assert_eq!(outcome.proposals.len(), 3);
        // The bootstrapped profile is now stored at version 1.
        let profile = state
            .fundamentals_provenance()
            .get_ocr_profile(&company_id)
            .expect("profile read")
            .expect("profile stored");
        assert_eq!(profile.version, 1);
        // No facts at all.
        let facts = state
            .list_financial_facts(crate::storage::ListFinancialFactsInput {
                company_id: Some(company_id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("facts");
        assert!(facts.is_empty(), "bootstrap must not write facts");
    }

    /// T4.3-6: no `VisionExtraction` member configured → the wrapper degrades with
    /// `no_vision_provider` (the run stays deterministically-quiet, never errors).
    #[test]
    fn tier4_degrades_when_no_vision_provider_configured() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        // Default settings pin the general provider to gemini, but `vision_extraction`
        // is not configured — tier-4 must NOT fall back to the general provider.
        let outcome = run_tier4_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 degrades, never errors");
        assert_eq!(outcome.reason, "no_vision_provider");
        assert!(outcome.produced_fact_ids.is_empty());
    }

    /// T4.3-7: an ESEF/structured document must never reach the OCR path — tier-4
    /// degrades with `not_pdf` (the ESEF tier owns those).
    #[test]
    fn tier4_degrades_not_pdf_for_an_esef_document() {
        let (state, company_id, document_id) = seed_esef();
        let outcome = run_tier4_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 degrades, never errors");
        assert_eq!(outcome.reason, "not_pdf");
    }

    /// T4.3-4: a transient OCR provider failure (rate limit) propagates as `Err`
    /// (the code the queue retries on), never a silent degrade.
    #[test]
    fn tier4_transient_ocr_failure_propagates_as_err() {
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        let err = tier4_with_provider(
            &state,
            &MockVisionProvider::ocr_failing("limit"),
            &company_id,
            &document_id,
            b"%PDF",
            "application/pdf",
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect_err("a transient OCR failure must be an Err");
        assert_eq!(err.0, "provider_limit");
        assert!(is_transient_provider_code(err.0));
    }

    /// T4.3-5: the caller gate is honored — a `DeniedSweep` gate never invokes the
    /// tier-4 hook, even when determinism ends Flagged (a drifted PDF profile).
    #[test]
    fn tier4_gate_denied_sweep_does_not_invoke_the_hook() {
        let (state, company_id, document_id) =
            seed_pdf(&["Total Assets Line 45 000", "Total Liabilities Line 20 000"]);
        let profile = ascii_profile(
            &company_id,
            &[
                ("total assets line", "total_assets"),
                ("total liabilities line", "total_liabilities"),
                ("total equity line", "total_equity"),
            ],
        );
        state
            .fundamentals_provenance()
            .upsert_profile(&profile)
            .expect("seed profile");

        // Determinism flags (dropped equity line). With the gate DENIED, tier-4
        // must not run: `tier4` stays None.
        let denied = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-12-31",
            MODE_AUTOPILOT,
            Tier4Gate::DeniedSweep,
        )
        .expect("runs");
        assert_eq!(denied.acceptance, Acceptance::Flagged);
        assert_eq!(denied.tier4, None, "a denied gate must not invoke tier-4");

        // With the gate ALLOWED, the hook runs (and degrades here: no vision
        // provider), so `tier4` is populated — proving the gate is what gates it.
        let allowed = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-12-31",
            MODE_AUTOPILOT,
            Tier4Gate::Allowed,
        )
        .expect("runs");
        assert_eq!(allowed.tier4.as_deref(), Some("no_vision_provider"));
    }

    // ---- tier-4 degrade log trail (T-A5) -----------------------------------

    // The process-global capturing logger lives in `crate::test_support` so the
    // backfill/source tests share one installed logger and buffer (lifted from
    // here, T-A4). Assertions filter by the unique `report_document_id` under
    // test rather than clearing the buffer, since other tests append too.
    use crate::test_support::{captured_logs, install_capture_logger};

    /// T-A5: a tier-4 degrade leaves a production-visible trail. A bootstrap whose
    /// LLM profile JSON fails to parse degrades `bootstrap_failed` AND emits one
    /// `log::warn!` carrying the reason, the ids, the OCR markdown length, and a
    /// prefix of the offending LLM response (the one degrade that includes it).
    #[test]
    fn tier4_bootstrap_failure_degrades_and_logs_a_warning() {
        install_capture_logger();
        let (state, company_id, document_id) = seed_pdf(&["prose only"]);
        // No stored OCR profile → the bootstrap path; a non-JSON bootstrap response
        // fails `from_bootstrap_json` → degrade `bootstrap_failed`.
        let bad_response = "totally not json — the model wrote prose";
        let provider = MockVisionProvider::with_bootstrap(BALANCED_OCR, bad_response);

        let outcome = tier4_with_provider(
            &state,
            &provider,
            &company_id,
            &document_id,
            b"%PDF",
            "application/pdf",
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("tier-4 degrades, never errors");
        assert_eq!(outcome.reason, "bootstrap_failed");

        let logs = captured_logs().lock().expect("logs");
        let line = logs
            .iter()
            .rev()
            .find(|l| l.contains(&document_id) && l.contains("bootstrap_failed"))
            .expect("a tier-4 degrade warn for this document was emitted");
        assert!(line.contains(&company_id), "warn carries the company id");
        assert!(
            line.contains(&format!("ocr_markdown_len={}", BALANCED_OCR.len())),
            "warn carries the OCR markdown length: {line}"
        );
        assert!(
            line.contains("llm_response_prefix") && line.contains("not json"),
            "bootstrap_failed warn carries the LLM response prefix: {line}"
        );
    }
}
