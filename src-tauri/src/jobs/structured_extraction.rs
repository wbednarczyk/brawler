//! Structured-first fundamentals extraction service (ADR 0061 S5).
//!
//! Loads a stored report document, runs the deterministic tiered pipeline
//! (ESEF → EspiCoverNote → HTML aggregator), and persists the accepted facts
//! with their provenance (source tier + validation verdict + citation). The
//! pipeline is deterministic end to end.
//!
//! The PDF fact-extraction arm is retired (ADR 0086 dec. 1) and the
//! positional (tier-3b pdf2htmlEX) arm is retired (ADR 0095): a real PDF, or
//! bare non-iXBRL markup, spawns NO extraction attempt here — both routes
//! survive only so [`derive_report_period`] can group the registry. Core
//! KPIs for such a company arrive from the BiznesRadar-primary daily pull (a
//! separate job), not from this seam. A markup/ESEF document no tier parses
//! is an honest gap.

use std::collections::BTreeSet;

use crate::app_state::AppState;
use crate::fundamentals::extraction::container::{detect_container, Container};
use crate::fundamentals::extraction::esef::projection::select_primary_basis;
use crate::fundamentals::extraction::pipeline::{run_pipeline, Acceptance, PipelineInput};
use crate::fundamentals::extraction::{SourceTier, StatementBasis};
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
    // `structure_drift` is retired (ADR 0086 dec. 1): no new row carries it;
    // already-stored rows keep the literal string.
    /// The aggregator disagreed with an issuer-held value — recorded by the
    /// reversed-witnessing paths (the BR-primary pull and the WDF ingest seam),
    /// never by this pipeline (ADR 0086 dec. 4).
    pub const WITNESS_DISAGREEMENT: &str = "witness_disagreement";
    // `witness_fallback` is retired (ADR 0086); already-stored rows keep the
    // literal string, and readers stay tolerant.
    /// A re-read of an already-stored slot disagreed with the committed value
    /// (migration `0123`, epic #229 T5 / #192). The stored value is KEPT — this
    /// records the disagreement so it can be ratified instead of evaporating
    /// with the run result.
    pub const VALUE_DIVERGENCE: &str = "value_divergence";
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
/// would hide that.
fn reason_for(
    outcome: &crate::fundamentals::extraction::pipeline::PipelineOutcome,
) -> &'static str {
    if outcome.acceptance.emits() {
        return reason::EMITTED;
    }
    match outcome.acceptance {
        Acceptance::Empty => reason::NO_DETERMINISTIC_TIER,
        // A non-emitting non-empty outcome is a failed gate: `witness_disagreement`
        // is recorded by the aggregator pull's reversed witnessing, never by this
        // pipeline (ADR 0086 dec. 4).
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

/// One re-read that disagreed with the stored value, recorded as a DURABLE
/// outcome row (epic #229 T5, closing the #192 residual).
///
/// The stored value is never overwritten (that is the divergence policy) — but
/// until now the finding lived only in the run result and a developer-mode
/// diagnostic trimmed after 7 days, so a disagreement between two reads of the
/// issuer's own filing could not be reviewed later. The row is keyed by the
/// synthetic `documentId#metricKey` slot ref (the reversed-witnessing precedent),
/// so two diverging metrics in one document keep separate rows and a
/// re-extraction UPSERTS its row instead of appending a duplicate.
///
/// Detail is the canonical gate shape the Coverage "Flagged periods" panel
/// renders (`failedIdentities` / `failedCrossChecks` / `valueDivergences`), with
/// `actual` = the STORED value and `expected` = the freshly read one; the raw
/// `storedValue` / `incomingValue` / `factId` travel inside `detail` for
/// programmatic inspection (the panel ignores keys it does not render).
///
/// Best-effort by design, exactly like [`record_outcome`]: a bookkeeping failure
/// is logged, never propagated into the extraction verdict.
fn record_value_divergence_outcome(
    state: &AppState,
    slot: &DivergenceSlot<'_>,
    divergence: &FactDivergence,
) {
    let DivergenceSlot {
        company_id,
        report_document_id,
        fiscal_year,
        period_type,
        period_end,
        tier,
    } = *slot;
    let stored = divergence.existing.trim();
    let incoming = divergence.incoming.trim();
    log::info!(
        "module=structured_extraction stage=value_divergence company={company_id} \
         document={report_document_id} metric={} stored={stored} incoming={incoming}",
        divergence.metric_key,
    );
    let detail = serde_json::json!({
        "failedIdentities": [],
        "failedCrossChecks": [],
        "valueDivergences": [{
            "metricKey": divergence.metric_key,
            "detail": {
                "actual": stored,
                "expected": incoming,
                "storedValue": stored,
                "incomingValue": incoming,
                "factId": divergence.fact_id,
            },
        }],
    })
    .to_string();
    let outcome_ref = format!("{report_document_id}#{}", divergence.metric_key);
    if let Err(error) = state.fundamentals_provenance().record_extraction_outcome(
        crate::storage::NewExtractionOutcome {
            company_id,
            report_document_id: &outcome_ref,
            fiscal_year,
            period_type,
            period_end,
            tier: Some(tier),
            acceptance: Acceptance::Flagged.as_str(),
            reason_code: reason::VALUE_DIVERGENCE,
            detail_json: Some(&detail),
            drift_json: None,
            structure_changed: false,
            // No fact was established by this finding — the stored one stays.
            fact_count: 0,
        },
    ) {
        log::warn!(
            "module=structured_extraction stage=value_divergence_record_failed \
             company={company_id} document={report_document_id} error={error}"
        );
    }
}

/// The slot a [`record_value_divergence_outcome`] row is written against — the
/// document/period the re-read ran over, plus the tier that re-read it.
struct DivergenceSlot<'a> {
    company_id: &'a str,
    report_document_id: &'a str,
    fiscal_year: i64,
    period_type: &'a str,
    period_end: &'a str,
    tier: &'a str,
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
    let _ = state
        .diagnostics()
        .record_integrity_event(crate::storage::NewDiagnosticEvent {
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
    /// Whether a deterministic **issuer** tier produced a successful accepted
    /// observation this run — `produced_fact_ids` OR `skipped_fact_ids`
    /// nonempty (epic #398 Item B blocker 1: a re-extraction that only
    /// RE-OBSERVES an already-landed period is still a success, not a gap).
    /// (The ADR 0085 aggregator-fallback flag is retired — ADR 0086; stored
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
/// `confirmation_state` is a frozen compatibility column; `mode` does not
/// affect it — the parameter stays so callers pass it uniformly. `Flagged`/
/// `Empty` never reach here (`Acceptance::emits()` is `false`).
fn confirmation_state_for(_acceptance: Acceptance, _mode: &str) -> &'static str {
    "confirmed"
}

/// The derivation-grammar version stamped on every persisted period (migration
/// 0109). A document's bytes are immutable once ingested, so the ONLY reason to
/// re-derive a cached period is a change to the derivation grammar itself
/// ([`derive_report_period_uncached`] — the ESEF/title/cover-page rules). Bump
/// this when that grammar changes: any cached row stamped with an older version
/// is re-derived and overwritten on next read (self-healing).
///
/// `2` (bug #325): `report_diff::classify`'s marker grammar now pairs a
/// day+month calendar marker with the year found in the SAME match rather
/// than the first clean year anywhere in the text — fixes a positional
/// cover-page document whose true reporting year was split by a pdf2htmlEX
/// extraction artifact (e.g. "20 25") landing on an unrelated nearby year
/// (e.g. a signing dateline's "2026") instead. Bumping re-derives any cached
/// period this could have affected.
pub const DERIVATION_VERSION: i64 = 2;

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
/// be read. **Do not** bump for changes that cannot affect readability. A
/// legacy stored run with no `pipelineVersion` reads as `0` and re-arms once.
///
/// `4`: #511 role families + #509 non-currency units.
/// `5`: one statement basis per document, the true basis stored (#508).
pub const EXTRACTION_PIPELINE_VERSION: u32 = 5;

/// Re-intern a cached `period_type` string back to the `&'static str` the
/// derivation returns. [`crate::report_diff::classify`] only ever yields these
/// four labels (`to_period`), plus ESEF's `FY`. An unrecognised cached label
/// (never written by the current code) yields `None`, forcing a safe re-derive.
pub(crate) fn intern_period_type(period_type: &str) -> Option<&'static str> {
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
    // Read the persisted derivation first — a fresh-enough hit avoids ALL file
    // IO. A hit requires BOTH the current grammar version and matching content
    // provenance (migration 0140): both hashes present and equal — `None ==
    // None` never counts, so legacy rows and recaptured documents re-derive
    // and overwrite (self-healing) instead of serving a period computed from
    // different bytes.
    if let Ok(Some(cached)) = state.financials().cached_derived_period(&document.id) {
        let provenance_matches = matches!(
            (cached.content_hash.as_deref(), document.content_hash.as_deref()),
            (Some(cached_hash), Some(document_hash)) if cached_hash == document_hash
        );
        if cached.derivation_version >= DERIVATION_VERSION && provenance_matches {
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
        // Older version / stale provenance → re-derive and overwrite below.
    }

    let derived = derive_report_period_uncached(state, document);

    // Persist ONLY once the document is ingested (fetched, with a stored file):
    // its bytes — hence its derived period — are then stable. Caching a
    // pre-fetch `None` would poison a document into never being re-derived after
    // it is fetched. The stamped hash comes from the SAME document snapshot the
    // derivation read, never a re-query of the mutable row.
    if document.fetch_status == "fetched" && document.local_path.is_some() {
        let _ = state.financials().store_derived_period(
            &document.id,
            derived.as_ref().map(|(fy, pt, pe)| (*fy, *pt, pe.as_str())),
            DERIVATION_VERSION,
            document.content_hash.as_deref(),
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
pub(crate) fn derive_report_period_uncached(
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
    // Container truth (epic #229 T2): a `.pdf` holding markup is read as
    // markup. A ZIP package or an unrecognised container yields no cover text
    // at all — the ESEF route above already self-derives a package's period
    // from its iXBRL contexts — so it stays a measured `no_period_derived`
    // gap rather than a garbage parse.
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
    /// Bare markup that is NOT inline-XBRL (a pdf2htmlEX render, an HTML
    /// export) → NO extraction attempt (ADR 0095: the positional tier is
    /// retired). The classification survives only for period-grouping
    /// purposes, mirroring [`Self::Pdf`].
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

/// Layer 1 tagged-fact extractor version (ADR 0100 decision 8): a bump rebuilds
/// every stored generation, roles included; 3 = the decision 3 role families.
const TAGGED_FACT_EXTRACTOR_VERSION: i64 = 3;

/// One document's Layer 1 generation, computed PURELY from bytes (ADR 0100
/// decisions 1/3/9, epic #398) — no storage, no freshness check. Shared by
/// [`capture_layer1_tagged_facts`] (the storage-side capture below) and
/// [`run_structured_extraction`]'s pipeline projection input, so the two can
/// never disagree about which rows Layer 1 sees.
pub(crate) struct Layer1Generation {
    pub(crate) facts: Vec<crate::storage::NewTaggedFact>,
    encountered_count: i64,
    stored_count: i64,
    dimensional_count: i64,
    /// Whether the route carried at least one readable instance — a package
    /// with none is `state = "no_instance"`, never conflated with "extracted
    /// zero facts from a real instance". `pub(crate)` so the corpus concept
    /// harvest (`storage::tests::real_data_extraction`) can tell the two apart
    /// the same way the job does.
    pub(crate) has_instance: bool,
    /// Whether ANY presentation-linkbase role was attached to ANY fact in
    /// this generation — `false` for a bare (non-package) iXBRL instance,
    /// which carries no `*_pre.xml` at all, and for a package whose linkbase
    /// failed to parse. Feeds `PipelineInput::has_presentation_linkbase`
    /// (regression fix, epic #398): without this evidence the strict
    /// primary-statement role filter cannot run, so the pipeline falls back
    /// to the pre-epic dimensionless + crosswalk-resolved selection instead
    /// of silently projecting zero facts.
    pub(crate) has_presentation_linkbase: bool,
    /// Count of dimensionless, valued facts that would use (or already use)
    /// the no-linkbase fallback — the ADR's "record the fallback explicitly,
    /// never silently" counter. Always `0` when `has_presentation_linkbase`
    /// is `true`.
    no_linkbase_fallback_count: i64,
    /// Truncation evidence (sol review finding 4): a mid-document XML reader
    /// error, or package entries skipped unread (unreadable / oversized).
    /// Either makes the generation INCOMPLETE — "encountered = stored" holds
    /// only over what was actually walked — so the extraction record must
    /// say `truncated`, never look complete.
    truncation: Option<String>,
}

/// Only an ESEF-bearing route carries an instance to walk; every other route
/// yields the empty generation (nothing to extract). `pub(crate)` so the real
/// (non-CI) corpus regression harness (`storage::tests::t7_cbf_corpus`) can
/// build the exact same pipeline input production does.
pub(crate) fn compute_layer1_generation(bytes: &[u8], route: DocumentRoute) -> Layer1Generation {
    let (instances, skipped_entries): (Vec<(String, Vec<u8>)>, i64) = match route {
        DocumentRoute::IxbrlInstance => (vec![(String::new(), bytes.to_vec())], 0),
        DocumentRoute::ZipPackage => {
            crate::fundamentals::extraction::esef_package::extract_all_instances_counted(bytes)
        }
        DocumentRoute::Pdf | DocumentRoute::Positional | DocumentRoute::Unsupported(_) => {
            return Layer1Generation {
                facts: Vec::new(),
                encountered_count: 0,
                stored_count: 0,
                dimensional_count: 0,
                has_instance: false,
                has_presentation_linkbase: false,
                no_linkbase_fallback_count: 0,
                truncation: None,
            };
        }
    };

    // The presentation linkbase lives in the ZIP package; a bare (non-package)
    // iXBRL instance carries no linkbase to read, so facts get no role rows.
    // Without the strict role filter's own evidence, the pipeline falls back
    // to the pre-epic dimensionless + crosswalk-resolved selection for this
    // document rather than silently projecting zero facts (regression fix,
    // epic #398) — see `has_presentation_linkbase` above.
    let roles = if matches!(route, DocumentRoute::ZipPackage) {
        crate::fundamentals::extraction::esef_package::extract_presentation_roles(bytes)
    } else {
        std::collections::HashMap::new()
    };
    let has_presentation_linkbase = !roles.is_empty();

    let mut all_facts = Vec::new();
    let mut encountered_count = 0i64;
    let mut stored_count = 0i64;
    let mut dimensional_count = 0i64;
    let mut truncation: Option<String> = if skipped_entries > 0 {
        Some(format!(
            "{skipped_entries} package entr{} skipped unread (unreadable or oversized)",
            if skipped_entries == 1 { "y" } else { "ies" }
        ))
    } else {
        None
    };
    for (path, instance_bytes) in &instances {
        let pass = crate::fundamentals::extraction::esef::layer1::extract_tagged_facts(
            instance_bytes,
            path,
            &roles,
        );
        encountered_count += pass.encountered_count;
        stored_count += pass.stored_count;
        dimensional_count += pass.dimensional_count;
        if let Some(error) = pass.reader_error {
            let entry = if path.is_empty() { "instance" } else { path };
            let message = format!("{entry}: {error}");
            truncation = Some(match truncation.take() {
                Some(existing) => format!("{existing}; {message}"),
                None => message,
            });
        }
        all_facts.extend(pass.facts);
    }

    // "Record the fallback explicitly ... so it is visible, never silent":
    // every dimensionless, valued fact in a no-linkbase document is a
    // fallback candidate. Zero when a linkbase exists — the fallback never
    // runs, nothing to count.
    let no_linkbase_fallback_count = if has_presentation_linkbase {
        0
    } else {
        all_facts
            .iter()
            .filter(|f| !f.is_dimensional && f.value_numeric.is_some())
            .count() as i64
    };

    Layer1Generation {
        facts: all_facts,
        encountered_count,
        stored_count,
        dimensional_count,
        has_instance: !instances.is_empty(),
        has_presentation_linkbase,
        no_linkbase_fallback_count,
        truncation,
    }
}

/// Runs Layer 1 raw tagged-fact capture for one document, alongside (never
/// gating) the Layer 2 pipeline in [`run_structured_extraction`] below (ADR
/// 0100 decisions 1/8/9, epic #398 slice). Best-effort: a failure here is
/// logged and swallowed, never propagated — Layer 2's behaviour must stay
/// byte-identical regardless of what this does.
///
/// Freshness is `(source_content_hash, extractor_version)` (decision 8): an
/// unchanged document is skipped, never re-walked (never even re-parsed).
fn capture_layer1_tagged_facts(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    bytes: &[u8],
    route: DocumentRoute,
) {
    if !matches!(
        route,
        DocumentRoute::IxbrlInstance | DocumentRoute::ZipPackage
    ) {
        return;
    }

    let source_hash = crate::report_documents_capture::content_hash_hex(bytes);
    match state.report_tagged_facts().is_extraction_current(
        report_document_id,
        &source_hash,
        TAGGED_FACT_EXTRACTOR_VERSION,
    ) {
        Ok(true) => return, // unchanged bytes + extractor version — nothing to do
        Ok(false) => {}
        Err(error) => {
            record_layer1_capture_diagnostic(
                state,
                report_document_id,
                "freshness_check_failed",
                &error.to_string(),
            );
            return;
        }
    }

    let generation = compute_layer1_generation(bytes, route);
    let extraction = crate::storage::TaggedFactExtraction {
        source_content_hash: Some(source_hash),
        extractor_version: TAGGED_FACT_EXTRACTOR_VERSION,
        state: if !generation.has_instance {
            // A package whose only candidates were SKIPPED unread is
            // truncated, not instance-free: "no_instance" must mean "we
            // looked and there was nothing", never "we could not look"
            // (sol round 2, finding 4).
            if generation.truncation.is_some() {
                "truncated".to_owned()
            } else {
                "no_instance".to_owned()
            }
        } else if let Some(reason) = generation.truncation.as_deref() {
            // Visible incompleteness (sol review finding 4): a reader error
            // or a skipped package entry means the counts cover only what
            // was walked — never presented as a complete extraction.
            log::warn!(
                "tagged-fact capture for report document {report_document_id} is TRUNCATED: {reason}"
            );
            "truncated".to_owned()
        } else {
            "extracted".to_owned()
        },
        encountered_count: generation.encountered_count,
        stored_count: generation.stored_count,
        dimensional_count: generation.dimensional_count,
        no_linkbase_fallback_count: generation.no_linkbase_fallback_count,
        facts: generation.facts,
    };

    if let Err(error) = state.report_tagged_facts().replace_tagged_facts(
        report_document_id,
        company_id,
        &extraction,
    ) {
        record_layer1_capture_diagnostic(
            state,
            report_document_id,
            "capture_write_failed",
            &error.to_string(),
        );
    }
}

/// A swallowed Layer 1 failure is never ONLY a log line (sol round 2,
/// finding 4): capture is best-effort by design (Layer 2 must not fail with
/// it — ADR 0100 decision 1), but the loss has to be visible where the owner
/// looks, so it lands in `diagnostic_events` through the UNGATED integrity
/// path (sol round 3: the ordinary diagnostic API is developer-mode-gated
/// and would silently record nothing on a default profile). The diagnostic
/// write itself is best-effort too — a database that cannot write the
/// extraction likely cannot write the diagnostic either, and the log line
/// remains the floor.
fn record_layer1_capture_diagnostic(
    state: &AppState,
    report_document_id: &str,
    stage: &str,
    error: &str,
) {
    log::warn!("tagged-fact capture {stage} for report document {report_document_id}: {error}");
    let _ = state
        .diagnostics()
        .record_integrity_event(crate::storage::NewDiagnosticEvent {
            occurred_at: None,
            module: "report_tagged_facts_capture".to_owned(),
            scope: Some(crate::storage::DiagnosticScope {
                scope_type: "report_document".to_owned(),
                id: Some(report_document_id.to_owned()),
            }),
            stage: stage.to_owned(),
            severity: "warning".to_owned(),
            message: format!("Layer 1 tagged-fact capture did not persist: {error}"),
            metadata: None,
        });
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
    // routing ADDS coverage via the ESEF/iXBRL route (the positional route is
    // classification-only since ADR 0095). An unsupported
    // container is an explicit outcome row, never an error that aborts the sweep.
    let route = route_document(&bytes);
    // Layer 1 raw tagged-fact capture (ADR 0100 dec. 1/8/9, epic #398 slice):
    // a best-effort side channel that runs alongside the Layer 2 pipeline
    // below and never affects its outcome — any failure here is logged and
    // swallowed, not surfaced as a `run_structured_extraction` error.
    capture_layer1_tagged_facts(state, company_id, report_document_id, &bytes, route);

    // ADR 0100 (epic #398): the ESEF tier's candidate set is now a Layer 1
    // projection (`fundamentals::extraction::esef::projection`), not a fresh
    // `parse_esef` over unwrapped instance bytes — so this match only needs
    // to (a) preserve the existing route-level outcomes (a bare-Positional or
    // real-Pdf route emits nothing, an unreadable ZIP/unsupported container is
    // an honest error) and (b) confirm an instance actually exists before
    // computing the Layer 1 generation below.
    match route {
        // Bare markup that is NOT iXBRL — a pdf2htmlEX visual render or an
        // HTML export. NO extraction attempt: the positional tier is retired
        // (ADR 0095). Mirrors the `DocumentRoute::Pdf` idiom below exactly: the
        // route survives so the registry/period derivation still group this
        // document, but no tier reads financial facts out of it. Returns a
        // benign empty result and records NO outcome, so this document never
        // generates a `no_deterministic_tier` extraction-outcome row.
        DocumentRoute::Positional => {
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
        // Markup that IS inline-XBRL → the ESEF tier reads it as its own instance
        // (a bare `.xhtml` instance stored under any name).
        DocumentRoute::IxbrlInstance => {}
        // A ZIP report package (ESEF/eSprawozdanie): a ZIP with no readable
        // instance is unreadable-by-container, recorded as such — never fed to
        // the PDF reader (ADR 0061 dec. 1).
        DocumentRoute::ZipPackage => {
            if crate::fundamentals::extraction::esef_package::extract_instance(&bytes).is_none() {
                return Err(unsupported_container(
                    Container::Zip,
                    "the ZIP report package holds no readable iXBRL instance",
                ));
            }
        }
        // A real PDF → NO extraction attempt (ADR 0086 dec. 1: the PDF fact
        // arm is retired). The route survives for period grouping only; core
        // KPIs for a PDF-only company arrive from the BiznesRadar-primary
        // daily pull. Records NO outcome, so a PDF never generates a
        // `no_deterministic_tier` row.
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

    // The pipeline's own Layer 1 generation (ADR 0100 decisions 1/4/7): a
    // second, independent computation from the same bytes — never read back
    // from storage — so a Layer 1 write failure above can never silently
    // widen or narrow what Layer 2 projects from.
    let layer1 = compute_layer1_generation(&bytes, route);
    // The document's ONE primary basis (ADR 0100 decision 2, #508).
    let basis = select_primary_basis(&layer1.facts, layer1.has_presentation_linkbase);
    let basis_str = basis.map(StatementBasis::as_str);

    // --- Comparative cross-check + completeness inputs (ADR 0061 dec. 4b/4d) --
    let prior_end = prior_period_end(period_end);
    // Veto-capable, like-for-like priors only (ADR 0086 dec. 3/4; ADR 0100
    // dec. 4, #508): a lower tier or the OTHER statement basis must not fail
    // the issuer filing's comparative cross-check.
    let stored_prior = state
        .financials()
        .stored_fact_set_for_cross_check(
            company_id,
            fiscal_year - 1,
            period_type,
            SourceTier::Esef,
            basis_str,
        )
        .map_err(|e| e.to_string())?;
    let expected_keys = expected_primary_keys(state, company_id)?;

    let input = PipelineInput {
        period_end,
        layer1_facts: Some(&layer1.facts),
        has_presentation_linkbase: layer1.has_presentation_linkbase,
        basis,
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

    // The PDF profile-drift arm is retired (ADR 0086 dec. 1) — `drift_json` is
    // kept (append-only) and always `None`.
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
    // Facts refused by a fact-local store guard (#509 decision 2).
    let mut rejected: Vec<RejectedFact> = Vec::new();
    if outcome.acceptance.emits() {
        let validation_status = outcome.acceptance.validation_status();
        let tier = outcome.tier.map(|t| t.as_str()).unwrap_or("unknown");
        let confirmation_state = confirmation_state_for(outcome.acceptance, mode);
        let store = state.kpi_extraction();
        // Within-batch dedup: every fact in one run shares the SAME selected
        // basis (#508 decision 2), so a plain per-`metric_key` slot stays
        // basis-safe. Keep the FIRST occurrence deterministically — a later
        // same-key fact would otherwise re-observe the row this same run
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
            .metric_histories(
                company_id,
                &history_keys,
                fiscal_year,
                period_type,
                basis_str,
            )
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
            let commit = match store.record_structured_fact(StructuredFactInput {
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
                attribution: None,
                measure_window: None,
                data_quality: None,
                statement_basis: basis_str, // #508: the true, selected basis.
            }) {
                Ok(commit) => commit,
                // #509 decision 2: these four keys are per-fact write-slot
                // fields; any other key/variant (e.g. `period_type`) is fatal.
                Err(crate::storage::StorageError::InvalidFinancialsValue { key, value })
                    if is_fact_local_refusal(key) =>
                {
                    rejected.push(RejectedFact {
                        metric_key: fact.metric_key.clone(),
                        field: key,
                        value,
                    });
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
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
                    let divergence = FactDivergence {
                        fact_id,
                        metric_key,
                        existing,
                        incoming,
                    };
                    // Durable, not just in-memory (epic #229 T5 / #192).
                    record_value_divergence_outcome(
                        state,
                        &DivergenceSlot {
                            company_id,
                            report_document_id,
                            fiscal_year,
                            period_type,
                            period_end,
                            tier,
                        },
                        &divergence,
                    );
                    divergences.push(divergence);
                }
                // Non-catalog key — the pipeline should not emit it; not counted.
                crate::storage::StructuredFactCommit::NoDefinition => {}
            }
        }
    }

    // A successful accepted ESEF observation (epic #398 Item B blocker 1): NOT
    // "at least one Created/Upgraded" — a re-extraction of an already-landed
    // period legitimately lands EVERY fact in `skipped_fact_ids` (Reobserved/
    // Divergent), zero in `produced_fact_ids`, and that is still a genuine
    // success the version-aware re-extraction (below) and the autopilot
    // `extractionAvailable` delta must both count as `true` — only a set with
    // NOTHING committed anywhere (fully quarantined, or empty) is a gap.
    let issuer_emitted = !produced_fact_ids.is_empty() || !skipped_fact_ids.is_empty();

    // --- Persist the OUTCOME, emitting or not (ADR 0061 dec. 2 guardrail) ---
    // The emit branch above is unchanged; this records what the run concluded
    // for every attempt, so a Flagged/Empty period leaves a durable, reviewable
    // trace instead of evaporating with the in-memory result.
    //
    // A quarantine OR a rejection (#509 decision 2) downgrades to Flagged /
    // `validation_failed`, chaining both payload keys onto one detail.
    let flagged = !quarantined.is_empty() || !rejected.is_empty();
    let recorded_acceptance = if flagged {
        Acceptance::Flagged
    } else {
        outcome.acceptance
    };
    let (reason_code, detail_json) = if flagged {
        let mut detail = failing_check_detail(&outcome);
        if !quarantined.is_empty() {
            detail = quarantine_detail(&quarantined, detail);
        }
        if !rejected.is_empty() {
            detail = rejected_detail(&rejected, detail);
        }
        (reason::VALIDATION_FAILED, detail)
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

/// Typed refusal prefix for a re-run request against an outcome slot that names
/// no re-readable document (the reversed-witnessing rows, whose slot ref is an
/// aggregator PAGE URL). A machine-readable code, not prose, so the MCP/API
/// caller can branch on it — the UI simply does not offer the action there.
pub(crate) const RERUN_NOT_APPLICABLE: &str = "rerun_not_applicable";

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

    // Reversed witnessing keys its slot by the aggregator PAGE URL it read
    // (`pageUrl#metricKey`), not by a stored document — there is nothing for the
    // pipeline to re-read, and handing it that ref produced an opaque storage
    // error ("Query returned no rows"). Refuse with a typed code instead; the
    // fix for a disagreement is a fresh aggregator pull or a manual correction,
    // never a re-extraction.
    if outcome.reason_code == reason::WITNESS_DISAGREEMENT {
        return Err(format!(
            "{RERUN_NOT_APPLICABLE}: outcome '{outcome_id}' records an aggregator \
             disagreement against a held value, not a document extraction — there is \
             no stored document to re-read"
        ));
    }

    run_structured_extraction(
        state,
        &outcome.company_id,
        base_document_ref(&outcome.report_document_id),
        outcome.fiscal_year,
        &outcome.period_type,
        &outcome.period_end,
        mode,
    )
}

/// The real stored-document id behind an outcome slot ref.
///
/// Per-metric outcomes (`value_divergence`) key their slot by the synthetic
/// `documentId#metricKey` discriminator, so two diverging metrics in one document
/// keep separate rows. That suffix is bookkeeping, not identity: a re-run must
/// re-extract `documentId`. An ordinary slot ref carries no `#` and passes
/// through untouched.
fn base_document_ref(slot_ref: &str) -> &str {
    slot_ref.split('#').next().unwrap_or(slot_ref)
}
mod outcome_detail;
use outcome_detail::{
    is_fact_local_refusal, quarantine_detail, rejected_detail, QuarantinedFact, RejectedFact,
};

#[cfg(test)]
mod basis_tests;
#[cfg(test)]
mod role_families_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unit_refusal_tests;
