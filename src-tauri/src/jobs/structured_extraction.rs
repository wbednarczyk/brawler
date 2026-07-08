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
use crate::fundamentals::extraction::pdf::parse_pdf_text;
use crate::fundamentals::extraction::pipeline::{run_pipeline, Acceptance, PipelineInput};
use crate::fundamentals::extraction::profile::ExtractionProfile;
use crate::fundamentals::extraction::SourceTier;
use crate::report_diff::extraction::{extract_report, SourceFormat};
use crate::storage::{ListKpiDefinitionsInput, StructuredFactInput, MODE_AUTOPILOT};

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
    /// Whether the pipeline emitted any facts.
    pub emitted: bool,
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
/// document has no stored file, is an unparsable ESEF, or is a PDF whose period
/// can't be classified (the caller then falls back to AI, or surfaces an error).
///
/// - **Xhtml/ESEF**: the period is self-derived from the iXBRL contexts (ESEF is
///   an annual filing → `FY` at the latest context date).
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
        let raw = std::fs::read(state.data_dir().join(local_path)).ok()?;
        let instance = esef_instance_bytes(content_type, local_path, &raw)?;
        let facts = parse_esef(&instance).ok()?; // Not valid iXBRL → no period.
        let period_end = primary_period_end(&facts)?;
        let fiscal_year = period_end.get(0..4).and_then(|y| y.parse::<i64>().ok())?;
        return Some((fiscal_year, "FY", period_end));
    }

    // PDF: period from the document's title/URL.
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
fn is_esef_route(content_type: Option<&str>, local_path: &str) -> bool {
    if SourceFormat::resolve(content_type, local_path) == SourceFormat::Xhtml {
        return true;
    }
    let lower = local_path.to_ascii_lowercase();
    lower.ends_with(".xbri") || lower.ends_with(".zip")
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
pub fn run_structured_extraction(
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
    let local_path = document
        .local_path
        .ok_or_else(|| "the report document has no stored file".to_owned())?;
    let path = state.data_dir().join(&local_path);
    let bytes = std::fs::read(&path).map_err(|e| format!("failed to read report file: {e}"))?;

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

    Ok(StructuredExtractionResult {
        acceptance: outcome.acceptance,
        tier: outcome.tier,
        emitted: !produced_fact_ids.is_empty(),
        produced_fact_ids,
        skipped_fact_ids,
        divergences,
        drift_json,
        structure_changed,
    })
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
        )
        .expect("structured extraction runs");

        assert_eq!(result.tier, Some(SourceTier::Esef));
        assert_eq!(result.acceptance, Acceptance::Accepted);
        assert!(result.emitted, "the ESEF package should emit facts");
        assert_eq!(result.produced_fact_ids.len(), 3);
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
}
