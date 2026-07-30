//! Real-data **trust audit** (epic #229 T1 — measure before repairing).
//!
//! T2/T3/T4 of the fundamentals data-trust epic each propose a repair
//! (container truth, document association, currency integrity). A repair sized
//! by intuition is a guess; this harness produces the numbers those PRs paste
//! before/after: how many stored documents carry a misleading URL, how many
//! disagree with their own bytes, how many look genuinely mis-associated, which
//! canonical report slots those rows hold, and where currency/kpi-relevance
//! state is off.
//!
//! # Two callers, one core
//!
//! [`run_trust_audit`] is a pure function over an [`AppState`]: the env-gated
//! printer ([`real_data_trust_audit`]) runs it against a throwaway copy of the
//! owner's database, and [`synthetic_trust_audit_finds_seeded_defects`] runs it
//! against a seeded in-memory database in ordinary CI. The audit logic is
//! therefore testable without the real DB — a harness whose only exercise is a
//! machine nobody else has is a harness nobody can trust.
//!
//! # Machine tokens
//!
//! Every emitted line starts `token=<class>` in the machine vocabulary of
//! [`super::real_data_shape_inventory::shape_token`] (lower-case words joined by
//! `-`), so the output greps and diffs. Unlike the shape inventory, this output
//! is **not** committed — it is operator-facing and deliberately carries row ids
//! (document/company/fact), which is exactly what a repair migration needs as
//! evidence. Never paste it into a public file verbatim.
//!
//! # Running it (local, owner's machine only)
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/trust-worktest.sqlite3
//! BRAWLER_REAL_DB=../private/realdata/trust-worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo nextest run -p brawler real_data_trust_audit --run-ignored all --no-capture
//! ```
//!
//! Without `BRAWLER_REAL_DB` it SKIPs loudly; without `BRAWLER_REAL_DATA_DIR`
//! the file-reading classes (2, and class 3's content evidence) are skipped with
//! a printed note. Opening a database applies migrations, so it refuses the
//! master snapshot and the live application database.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::real_data_shape_inventory::shape_token;
use super::*;
use crate::commands::fundamentals_coverage::compute_fundamentals_coverage;
use crate::fundamentals::extraction::container::{detect_container, Container};
use crate::storage::sources::trust_audit_support::TrackedIssuers;

// ---------------------------------------------------------------------------
// Class vocabulary
// ---------------------------------------------------------------------------

const CLASS_URL_MISMATCH: &str = "title-url-issuer-mismatch";
const CLASS_CONTAINER: &str = "container-mismatch";
const CLASS_MISASSOCIATION: &str = "misassociation-candidate";
const CLASS_CANONICAL: &str = "canonical-slot-flagged";
const CLASS_COVERAGE: &str = "coverage-rollup";
const CLASS_CURRENCY: &str = "currency-anomaly";
const CLASS_KPI: &str = "kpi-relevance-gap";
const CLASS_NOTE: &str = "note";

/// Every class token the audit can emit. The guard test asserts each one is a
/// machine token, so a class renamed into prose reddens instead of silently
/// polluting the output.
const AUDIT_CLASSES: [&str; 8] = [
    CLASS_URL_MISMATCH,
    CLASS_CONTAINER,
    CLASS_MISASSOCIATION,
    CLASS_CANONICAL,
    CLASS_COVERAGE,
    CLASS_CURRENCY,
    CLASS_KPI,
    CLASS_NOTE,
];

/// How many bytes of a text-readable document the content-token evidence reads.
const CONTENT_EVIDENCE_BYTES: usize = 64 * 1024;

/// Above this many currency anomalies the audit prints group counts only — a
/// four-figure id dump is noise, not evidence.
const CURRENCY_ID_PRINT_LIMIT: usize = 100;

/// A `token=` value: lower-case machine words joined by `-`. Mirrors
/// [`shape_token`]'s vocabulary segment by segment (it maps `_` to `-` and
/// refuses everything else), so the two cannot drift.
fn is_machine_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('-')
            .all(|segment| !segment.is_empty() && shape_token(segment) == segment)
}

// ---------------------------------------------------------------------------
// Report shape
// ---------------------------------------------------------------------------

/// Class 1 — the URL names a tracked issuer that is not the owner, while the
/// title carries no such contradiction (the misleading-CDN-slug shape).
#[derive(Debug)]
pub(super) struct UrlIssuerMismatch {
    pub(super) document_id: String,
    pub(super) owner_company_id: String,
    pub(super) url_company_id: String,
    /// Whether the owner is named in the title at all — a `false` here plus a
    /// foreign issuer in the URL is the harder case for T3.
    pub(super) owner_named_in_title: bool,
}

/// Class 2 — the stored bytes disagree with the name/content-type they were
/// stored under.
#[derive(Debug, Default)]
pub(super) struct ContainerAudit {
    pub(super) scanned: usize,
    pub(super) unreadable: usize,
    pub(super) extension_mismatch: usize,
    pub(super) content_type_mismatch: usize,
    /// `(extension, detected container)` → rows.
    pub(super) by_pair: BTreeMap<(String, &'static str), usize>,
    pub(super) document_ids: Vec<String>,
}

/// Class 3 — a document whose title/filename names a foreign issuer and never
/// names its owner. Deliberately broader than the shipped
/// `sources::names_foreign_issuer` guard: no `doc_kind` gate and no URL in the
/// owner check, so the audit sees the rows the guard is allowed to keep.
#[derive(Debug)]
pub(super) struct MisassociationCandidate {
    pub(super) document_id: String,
    pub(super) owner_company_id: String,
    pub(super) foreign_company_id: String,
    /// Owner/foreign name occurrences in the first [`CONTENT_EVIDENCE_BYTES`]
    /// of a text-readable file (`None` when the file was not read).
    pub(super) content_owner_hits: Option<usize>,
    pub(super) content_foreign_hits: Option<usize>,
}

/// Class 4 — a canonical report slot won by a document flagged in classes 1–3.
#[derive(Debug)]
pub(super) struct FlaggedCanonicalSlot {
    pub(super) company_id: String,
    pub(super) fiscal_year: i64,
    pub(super) period_type: String,
    pub(super) document_id: String,
}

/// Class 5 — the #174 denominator, per watchlist company.
#[derive(Debug)]
pub(super) struct CompanyCoverageRollup {
    pub(super) company_id: String,
    pub(super) documents: usize,
    pub(super) fetched: usize,
    pub(super) pending: usize,
    pub(super) metadata_only: usize,
    pub(super) periodic: usize,
    /// `companies_lacking_periodic_coverage` verdict — true = no fetched
    /// periodic document at all.
    pub(super) lacks_periodic_coverage: bool,
    pub(super) autopilot_mode: String,
}

/// Class 6 — a stored currency that is not an ISO-4217-shaped code.
#[derive(Debug, Default)]
pub(super) struct CurrencyAudit {
    pub(super) total: usize,
    /// `(metric_key, currency)` → rows.
    pub(super) by_group: BTreeMap<(String, String), usize>,
    pub(super) fact_ids: Vec<String>,
}

/// The audit's whole result. Rendering is separate from measuring so both
/// callers (real DB, synthetic) share one code path.
#[derive(Debug, Default)]
pub(super) struct TrustAuditReport {
    pub(super) url_mismatches: Vec<UrlIssuerMismatch>,
    pub(super) container: ContainerAudit,
    pub(super) misassociations: Vec<MisassociationCandidate>,
    pub(super) flagged_slots: Vec<FlaggedCanonicalSlot>,
    pub(super) coverage: Vec<CompanyCoverageRollup>,
    pub(super) currency: CurrencyAudit,
    /// `(company_id, kpi_relevance rows)` for every company.
    pub(super) kpi_relevance: Vec<(String, usize)>,
    /// Machine tokens naming what the run could NOT measure.
    pub(super) notes: Vec<&'static str>,
}

impl TrustAuditReport {
    /// Companies with no `kpi_relevance` row at all (post-0106 truth).
    pub(super) fn kpi_relevance_gaps(&self) -> Vec<&str> {
        self.kpi_relevance
            .iter()
            .filter(|(_, rows)| *rows == 0)
            .map(|(company_id, _)| company_id.as_str())
            .collect()
    }

    /// The audit as machine-token lines, headline counts first, then detail.
    pub(super) fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();

        for note in &self.notes {
            lines.push(format!("token={CLASS_NOTE} note={note}"));
        }

        lines.push(format!(
            "token={CLASS_URL_MISMATCH} count={}",
            self.url_mismatches.len()
        ));
        for row in &self.url_mismatches {
            lines.push(format!(
                "token={CLASS_URL_MISMATCH}-row document={} owner={} url_issuer={} \
                 owner_named_in_title={}",
                row.document_id, row.owner_company_id, row.url_company_id, row.owner_named_in_title
            ));
        }

        lines.push(format!(
            "token={CLASS_CONTAINER} count={} scanned={} unreadable={} extension_mismatch={} \
             content_type_mismatch={}",
            self.container.document_ids.len(),
            self.container.scanned,
            self.container.unreadable,
            self.container.extension_mismatch,
            self.container.content_type_mismatch
        ));
        for ((extension, detected), count) in &self.container.by_pair {
            lines.push(format!(
                "token={CLASS_CONTAINER}-pair extension={extension} detected={detected} \
                 count={count}"
            ));
        }
        for document_id in &self.container.document_ids {
            lines.push(format!(
                "token={CLASS_CONTAINER}-row document={document_id}"
            ));
        }

        lines.push(format!(
            "token={CLASS_MISASSOCIATION} count={}",
            self.misassociations.len()
        ));
        for row in &self.misassociations {
            let owner_hits = row
                .content_owner_hits
                .map_or_else(|| "n/a".to_owned(), |hits| hits.to_string());
            let foreign_hits = row
                .content_foreign_hits
                .map_or_else(|| "n/a".to_owned(), |hits| hits.to_string());
            lines.push(format!(
                "token={CLASS_MISASSOCIATION}-row document={} owner={} foreign={} \
                 content_owner_hits={owner_hits} content_foreign_hits={foreign_hits}",
                row.document_id, row.owner_company_id, row.foreign_company_id
            ));
        }

        lines.push(format!(
            "token={CLASS_CANONICAL} count={}",
            self.flagged_slots.len()
        ));
        for slot in &self.flagged_slots {
            lines.push(format!(
                "token={CLASS_CANONICAL}-row company={} fiscal_year={} period={} document={}",
                slot.company_id, slot.fiscal_year, slot.period_type, slot.document_id
            ));
        }

        lines.push(format!(
            "token={CLASS_COVERAGE} count={}",
            self.coverage.len()
        ));
        for row in &self.coverage {
            lines.push(format!(
                "token={CLASS_COVERAGE}-row company={} documents={} fetched={} pending={} \
                 metadata_only={} periodic={} lacks_periodic_coverage={} autopilot={}",
                row.company_id,
                row.documents,
                row.fetched,
                row.pending,
                row.metadata_only,
                row.periodic,
                row.lacks_periodic_coverage,
                row.autopilot_mode
            ));
        }

        lines.push(format!(
            "token={CLASS_CURRENCY} count={}",
            self.currency.total
        ));
        for ((metric_key, currency), count) in &self.currency.by_group {
            lines.push(format!(
                "token={CLASS_CURRENCY}-group metric={metric_key} currency={currency} \
                 count={count}"
            ));
        }
        if self.currency.total <= CURRENCY_ID_PRINT_LIMIT {
            for fact_id in &self.currency.fact_ids {
                lines.push(format!("token={CLASS_CURRENCY}-row fact={fact_id}"));
            }
        } else {
            lines.push(format!(
                "token={CLASS_CURRENCY}-row-ids-suppressed limit={CURRENCY_ID_PRINT_LIMIT}"
            ));
        }

        let gaps = self.kpi_relevance_gaps();
        lines.push(format!(
            "token={CLASS_KPI} count={} companies={}",
            gaps.len(),
            self.kpi_relevance.len()
        ));
        for company_id in gaps {
            lines.push(format!("token={CLASS_KPI}-row company={company_id}"));
        }

        lines
    }
}

// ---------------------------------------------------------------------------
// Container agreement
// ---------------------------------------------------------------------------

/// The container class a name/content-type CLAIMS, at the granularity where a
/// disagreement is real. `xml` and `html` collapse into one markup class on
/// purpose: an `.xhtml` ESEF statement opening with `<?xml` detects as
/// [`Container::Xml`], which is not a mislabelled document — flagging it would
/// bury the genuine pdf/zip/markup swaps this class exists to count.
fn container_class(container: Container) -> &'static str {
    match container {
        Container::Pdf => "pdf",
        Container::Zip => "zip",
        Container::Xml | Container::Html => "markup",
        Container::Unknown => "unknown",
    }
}

/// The container class an extension claims, or `None` when it claims nothing.
fn class_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "pdf" => Some("pdf"),
        "zip" | "xbri" | "xbrl" => Some("zip"),
        "xml" | "xhtml" | "html" | "htm" => Some("markup"),
        _ => None,
    }
}

/// The container class a stored `content_type` claims, or `None`.
fn class_for_content_type(content_type: &str) -> Option<&'static str> {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match essence.as_str() {
        "application/pdf" => Some("pdf"),
        "application/zip" | "application/x-zip-compressed" | "application/octet-stream" => {
            // `octet-stream` claims nothing beyond "bytes" — never a mismatch.
            match essence.as_str() {
                "application/octet-stream" => None,
                _ => Some("zip"),
            }
        }
        "text/html" | "application/xhtml+xml" | "text/xml" | "application/xml" => Some("markup"),
        _ => None,
    }
}

/// Lower-case extension of a stored path (`""` when it has none).
fn path_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The filename a URL ends in — the token an ESPI attachment is actually named
/// by, with the query string and fragment dropped.
fn url_filename(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}

// ---------------------------------------------------------------------------
// The audit core
// ---------------------------------------------------------------------------

/// Measure every trust class over `state`. `data_dir` is the Tauri data
/// directory holding `report_documents/` files; `None` skips the two
/// file-reading measurements and records a note instead of pretending.
///
/// Read-only: nothing here writes to the database or to disk.
pub(super) fn run_trust_audit(state: &AppState, data_dir: Option<&Path>) -> TrustAuditReport {
    let mut report = TrustAuditReport::default();

    let issuers = {
        let connection = state.checkout().expect("database connection");
        TrackedIssuers::load(&connection).expect("load tracked issuers")
    };
    if issuers.is_empty() {
        report.notes.push("tracked-issuers-empty");
    }
    if data_dir.is_none() {
        report.notes.push("data-dir-absent-file-classes-skipped");
    }

    let companies = state.list_companies().expect("list companies");
    let watchlisted: BTreeSet<String> = state
        .list_watchlist_memberships()
        .expect("list watchlist memberships")
        .into_iter()
        .map(|membership| membership.company_id)
        .collect();
    let lacking_coverage: BTreeMap<String, String> = state
        .report_documents()
        .companies_lacking_periodic_coverage(None)
        .expect("companies lacking periodic coverage")
        .into_iter()
        .collect();
    let autopilot_modes = autopilot_modes(state);

    for company in &companies {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");

        // Classes 1–3 over this company's documents; the flagged ids feed class 4.
        let mut flagged: BTreeSet<String> = BTreeSet::new();
        let mut rollup = CompanyCoverageRollup {
            company_id: company.id.clone(),
            documents: documents.len(),
            fetched: 0,
            pending: 0,
            metadata_only: 0,
            periodic: 0,
            lacks_periodic_coverage: lacking_coverage.contains_key(&company.id),
            autopilot_mode: autopilot_modes
                .get(&company.id)
                .cloned()
                .unwrap_or_else(|| "off".to_owned()),
        };

        for document in &documents {
            match document.fetch_status.as_str() {
                "fetched" => rollup.fetched += 1,
                "pending" => rollup.pending += 1,
                "metadata_only" => rollup.metadata_only += 1,
                _ => {}
            }
            if matches!(
                document.doc_kind.as_deref(),
                Some("periodic_ssf" | "periodic_jsf")
            ) {
                rollup.periodic += 1;
            }

            let title = document.title.as_deref().unwrap_or_default();
            let filename = url_filename(&document.url);

            // --- class 1: the URL names someone else ------------------------
            if document.source_type == "espi_attachment" {
                let url_issuers = issuers.named_in(&document.url, false);
                let title_issuers = issuers.named_in(title, false);
                let foreign_in_url = url_issuers
                    .iter()
                    .find(|id| **id != company.id)
                    .map(|id| (*id).to_owned());
                let foreign_in_title = title_issuers.iter().any(|id| **id != company.id);
                if let Some(url_company_id) = foreign_in_url {
                    // "the title/owner check passes" — the title itself never
                    // contradicts the owner, so only the URL is misleading.
                    if !foreign_in_title {
                        flagged.insert(document.id.clone());
                        report.url_mismatches.push(UrlIssuerMismatch {
                            document_id: document.id.clone(),
                            owner_company_id: company.id.clone(),
                            url_company_id,
                            owner_named_in_title: issuers.names(&company.id, title, true),
                        });
                    }
                }
            }

            // --- class 3: metadata names a foreign issuer, never the owner ---
            let label = format!("{title} {filename}");
            let foreign_in_label = issuers
                .named_in(&label, false)
                .into_iter()
                .find(|id| *id != company.id.as_str())
                .map(str::to_owned);
            let candidate_foreign = match foreign_in_label {
                Some(foreign) if !issuers.names(&company.id, &label, true) => Some(foreign),
                _ => None,
            };

            // --- classes 2 + 3's content evidence: the stored bytes ----------
            let bytes = match (data_dir, document.local_path.as_deref()) {
                (Some(dir), Some(local_path)) if document.fetch_status == "fetched" => {
                    match std::fs::read(dir.join(local_path)) {
                        Ok(bytes) => {
                            report.container.scanned += 1;
                            Some(bytes)
                        }
                        Err(_) => {
                            report.container.unreadable += 1;
                            None
                        }
                    }
                }
                _ => None,
            };

            if let Some(bytes) = bytes.as_deref() {
                let detected = detect_container(bytes);
                let detected_class = container_class(detected);
                let local_path = document.local_path.as_deref().unwrap_or_default();
                let extension = path_extension(local_path);
                let extension_disagrees = class_for_extension(&extension)
                    .is_some_and(|claimed| claimed != detected_class);
                let content_type_disagrees = document
                    .content_type
                    .as_deref()
                    .and_then(class_for_content_type)
                    .is_some_and(|claimed| claimed != detected_class);
                if extension_disagrees {
                    report.container.extension_mismatch += 1;
                }
                if content_type_disagrees {
                    report.container.content_type_mismatch += 1;
                }
                if extension_disagrees || content_type_disagrees {
                    flagged.insert(document.id.clone());
                    *report
                        .container
                        .by_pair
                        .entry((extension, detected.as_str()))
                        .or_default() += 1;
                    report.container.document_ids.push(document.id.clone());
                }
            }

            if let Some(foreign_company_id) = candidate_foreign {
                // Content-token evidence (precedent: the #92 re-diagnosis) —
                // only for markup we can read as text; a PDF's bytes say nothing.
                let (content_owner_hits, content_foreign_hits) = match bytes.as_deref() {
                    Some(bytes)
                        if matches!(detect_container(bytes), Container::Html | Container::Xml) =>
                    {
                        let window = &bytes[..bytes.len().min(CONTENT_EVIDENCE_BYTES)];
                        let text = String::from_utf8_lossy(window);
                        (
                            Some(issuers.phrase_occurrences(&company.id, &text)),
                            Some(issuers.phrase_occurrences(&foreign_company_id, &text)),
                        )
                    }
                    _ => (None, None),
                };
                flagged.insert(document.id.clone());
                report.misassociations.push(MisassociationCandidate {
                    document_id: document.id.clone(),
                    owner_company_id: company.id.clone(),
                    foreign_company_id,
                    content_owner_hits,
                    content_foreign_hits,
                });
            }
        }

        if watchlisted.contains(&company.id) {
            report.coverage.push(rollup);
        }

        // --- class 4: canonical slots held by a flagged document -------------
        if !flagged.is_empty() {
            match compute_fundamentals_coverage(state, &company.id) {
                Ok(coverage) => {
                    for period in coverage.periods {
                        let Some(cell) = period.report else {
                            continue;
                        };
                        if flagged.contains(&cell.document_id) {
                            report.flagged_slots.push(FlaggedCanonicalSlot {
                                company_id: company.id.clone(),
                                fiscal_year: period.fiscal_year,
                                period_type: period.period_type,
                                document_id: cell.document_id,
                            });
                        }
                    }
                }
                Err(error) => {
                    log::warn!(
                        "module=trust_audit stage=canonical_slots company={} error={error}",
                        company.id
                    );
                }
            }
        }
    }

    report.currency = currency_audit(state);
    report.kpi_relevance = kpi_relevance_counts(state, &companies);

    report
}

/// `company_id → autopilot mode` for every company that has a settings row.
fn autopilot_modes(state: &AppState) -> BTreeMap<String, String> {
    let connection = state.checkout().expect("database connection");
    let mut statement = connection
        .prepare("SELECT company_id, mode FROM company_autopilot_settings")
        .expect("prepare autopilot modes");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query autopilot modes");
    rows.map(|row| row.expect("autopilot mode row")).collect()
}

/// Class 6: stored currencies that are not exactly three ASCII upper-case
/// letters. NULL is legitimate (a unit-less ratio) and never counted.
fn currency_audit(state: &AppState) -> CurrencyAudit {
    let connection = state.checkout().expect("database connection");
    let mut statement = connection
        .prepare(
            "
            SELECT financial_facts.id,
                   COALESCE(kpi_definitions.metric_key, 'unknown'),
                   financial_facts.currency
            FROM financial_facts
            LEFT JOIN kpi_definitions ON kpi_definitions.id = financial_facts.definition_id
            WHERE financial_facts.currency IS NOT NULL
            ORDER BY financial_facts.id
            ",
        )
        .expect("prepare currency audit");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("query currency audit");

    let mut audit = CurrencyAudit::default();
    for row in rows {
        let (fact_id, metric_key, currency) = row.expect("currency row");
        let iso_shaped = currency.len() == 3
            && currency
                .chars()
                .all(|character| character.is_ascii_uppercase());
        if iso_shaped {
            continue;
        }
        audit.total += 1;
        *audit.by_group.entry((metric_key, currency)).or_default() += 1;
        audit.fact_ids.push(fact_id);
    }
    audit
}

/// Class 7: `kpi_relevance` rows per company, zeros included.
fn kpi_relevance_counts(state: &AppState, companies: &[Company]) -> Vec<(String, usize)> {
    let connection = state.checkout().expect("database connection");
    let mut statement = connection
        .prepare("SELECT company_id, COUNT(*) FROM kpi_relevance GROUP BY company_id")
        .expect("prepare kpi relevance counts");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kpi relevance counts");
    let counts: BTreeMap<String, i64> = rows.map(|row| row.expect("kpi relevance row")).collect();

    companies
        .iter()
        .map(|company| {
            let rows = counts.get(&company.id).copied().unwrap_or(0).max(0) as usize;
            (company.id.clone(), rows)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The env-gated printer
// ---------------------------------------------------------------------------

#[test]
#[ignore = "real-data trust audit; needs BRAWLER_REAL_DB (a throwaway copy), optionally BRAWLER_REAL_DATA_DIR"]
fn real_data_trust_audit() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_trust_audit: BRAWLER_REAL_DB not set");
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP real_data_trust_audit: no database at {db_path}");
        return;
    }
    // Opening a database APPLIES MIGRATIONS — never the master snapshot, never
    // the live application database (same refusal as every real_data_* harness).
    let file_name = std::path::Path::new(&db_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    assert!(
        file_name != "brawler.sqlite3" && !db_path.starts_with("/mnt/d/"),
        "refusing to run: {db_path} is the master snapshot or the live application database. \
         This harness migrates — copy it first (private/realdata/README.md)."
    );

    let data_dir = std::env::var("BRAWLER_REAL_DATA_DIR")
        .ok()
        .map(PathBuf::from);
    if data_dir.is_none() {
        eprintln!(
            "NOTE real_data_trust_audit: BRAWLER_REAL_DATA_DIR not set — the container class and \
             the content-token evidence are skipped"
        );
    }

    let connection = open_database(&db_path).expect("open throwaway real db");
    let state = match data_dir.clone() {
        Some(dir) => AppState::with_data_dir(connection, dir),
        None => AppState::new(connection),
    };

    let report = run_trust_audit(&state, data_dir.as_deref());

    eprintln!("== epic #229 T1 real-data trust audit ==");
    eprintln!("db={db_path}");
    for line in report.render() {
        eprintln!("{line}");
    }
    eprintln!(
        "-- row ids above are evidence for the T3/T4 repair migrations; do not commit this output"
    );

    // A run that measured nothing must not be read as "the data is clean" (the
    // real_data_honesty "measured nothing = false green" rule).
    assert!(
        !report.kpi_relevance.is_empty(),
        "the audit saw no companies at all — wrong database copy?"
    );
}

// ---------------------------------------------------------------------------
// Guards that run in ordinary CI
// ---------------------------------------------------------------------------

#[test]
fn audit_tokens_are_machine_tokens() {
    for class in AUDIT_CLASSES {
        assert!(
            is_machine_token(class),
            "audit class {class:?} must be a machine token — the output is grepped, not read"
        );
    }

    // The filter is a filter, not a convention: prose, ids, uppercase and empty
    // values must all fail it.
    for prose in [
        "title↔URL issuer mismatch",
        "Container Mismatch",
        "misassociation candidate",
        "coverage rollup!",
        "CLASS",
        "trailing-",
        "-leading",
        "",
    ] {
        assert!(
            !is_machine_token(prose),
            "{prose:?} must never pass as an audit token"
        );
    }

    // Every line a report actually emits carries a machine token, including the
    // per-class detail suffixes (`-row`, `-pair`, `-group`).
    let report = TrustAuditReport {
        notes: vec!["data-dir-absent-file-classes-skipped"],
        url_mismatches: vec![UrlIssuerMismatch {
            document_id: "doc-1".to_owned(),
            owner_company_id: "company-1".to_owned(),
            url_company_id: "company-2".to_owned(),
            owner_named_in_title: true,
        }],
        misassociations: vec![MisassociationCandidate {
            document_id: "doc-2".to_owned(),
            owner_company_id: "company-1".to_owned(),
            foreign_company_id: "company-2".to_owned(),
            content_owner_hits: Some(0),
            content_foreign_hits: Some(7),
        }],
        flagged_slots: vec![FlaggedCanonicalSlot {
            company_id: "company-1".to_owned(),
            fiscal_year: 2024,
            period_type: "FY".to_owned(),
            document_id: "doc-2".to_owned(),
        }],
        coverage: vec![CompanyCoverageRollup {
            company_id: "company-1".to_owned(),
            documents: 3,
            fetched: 1,
            pending: 1,
            metadata_only: 1,
            periodic: 1,
            lacks_periodic_coverage: false,
            autopilot_mode: "assisted".to_owned(),
        }],
        currency: CurrencyAudit {
            total: 1,
            by_group: BTreeMap::from([(("eps_basic".to_owned(), "shares".to_owned()), 1)]),
            fact_ids: vec!["fact-1".to_owned()],
        },
        kpi_relevance: vec![("company-1".to_owned(), 0)],
        container: ContainerAudit {
            scanned: 1,
            unreadable: 0,
            extension_mismatch: 1,
            content_type_mismatch: 0,
            by_pair: BTreeMap::from([(("pdf".to_owned(), "html"), 1)]),
            document_ids: vec!["doc-3".to_owned()],
        },
    };

    let lines = report.render();
    assert!(
        lines.len() > 10,
        "expected a rendered line per class + detail"
    );
    for line in &lines {
        let token = line
            .strip_prefix("token=")
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or_else(|| panic!("every audit line starts with a token: {line:?}"));
        assert!(
            is_machine_token(token),
            "audit line {line:?} emitted a non-machine token {token:?}"
        );
    }
}

#[test]
fn synthetic_trust_audit_finds_seeded_defects() {
    let connection = open_in_memory_database().expect("database should initialize");
    let data_dir = std::env::temp_dir().join(format!(
        "brawler-trust-audit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(data_dir.join("report_documents")).expect("create data dir");
    let state = AppState::with_data_dir(connection, data_dir.clone());

    let owner = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "CYBER FOLKS S.A.".to_owned(),
            isin: Some("PLARTBI00019".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("owner company should create");
    let foreign = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "VRC".to_owned(),
            display_name: "VERCOM S.A.".to_owned(),
            isin: Some("PLVRCOM00016".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("foreign company should create");

    // Defect (7) has to be SEEDED now, not inherited: since T7 closed the #203
    // residual, `create_company` seeds the core `kpi_relevance` set, so a fresh
    // company no longer has the gap by accident. Stripping the rows keeps the
    // detector under test — a company can still reach this state (deleted
    // curation, a pre-0106 row the healing migration could not resolve).
    state
        .checkout_for_tests()
        .expect("connection should check out")
        .execute("DELETE FROM kpi_relevance", [])
        .expect("strip the seeded core set to recreate the audited defect");

    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Trust audit".to_owned(),
            description: None,
        })
        .expect("watchlist should create");
    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id,
            company_id: owner.id.clone(),
        })
        .expect("membership should create");

    // (1) Misleading CDN slug: the title is the owner's, the URL names Vercom.
    let url_mismatch_doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: owner.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://static.example.com/vercom-sa/2024/raport-roczny-2024.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Cyber Folks skonsolidowany raport roczny 2024".to_owned()),
            attribution: None,
        })
        .expect("url-mismatch document should create");

    // (3) Foreign-titled, owner never named — and its stored bytes are HTML
    // under a `.pdf` name, which is also (2).
    let misassociated_doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: owner.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://static.example.com/files/2024/VERCOM-raport-roczny-2024.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("VERCOM raport roczny 2024".to_owned()),
            attribution: None,
        })
        .expect("misassociated document should create");
    let stored_relative = "report_documents/vercom-raport-roczny-2024.pdf";
    std::fs::write(
        data_dir.join(stored_relative),
        b"<html><body>VERCOM S.A. raport roczny. VERCOM S.A. dane finansowe.</body></html>",
    )
    .expect("write stored document");
    state
        .mark_report_document_fetched(
            &misassociated_doc.id,
            Some(stored_relative),
            Some("application/pdf"),
            Some("hash-1"),
            Some(78),
        )
        .expect("document should mark fetched");

    // (6) A currency that is not an ISO-4217 code — the #93 EPS unit bug shape.
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: owner.id.clone(),
            fiscal_year: 2024,
            period_type: "FY".to_owned(),
            period_end_date: Some("2024-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("period should create");
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    let eps = definitions
        .iter()
        .find(|definition| definition.metric_key == "eps_basic")
        .expect("eps_basic should exist in the canonical catalog");
    let eps_fact = state
        .create_financial_fact(NewFinancialFact {
            company_id: owner.id.clone(),
            period_id: period.id.clone(),
            definition_id: eps.id.clone(),
            value_numeric: "3.21".to_owned(),
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
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("fact should create");
    // The non-ISO unit is a LEGACY shape: since #93's write-side guard the store
    // refuses to persist it, so the audit's detection is proven against a row
    // forced in behind the API — exactly like the rows already in the real DB.
    state
        .checkout()
        .expect("connection")
        .execute(
            "UPDATE financial_facts SET currency = 'shares' WHERE id = ?1",
            [&eps_fact.id],
        )
        .expect("legacy currency should be forced in");

    let report = run_trust_audit(&state, Some(&data_dir));
    let rendered = report.render().join("\n");

    // (1)
    let url_mismatches: Vec<&str> = report
        .url_mismatches
        .iter()
        .map(|row| row.document_id.as_str())
        .collect();
    assert_eq!(
        url_mismatches,
        vec![url_mismatch_doc.id.as_str()],
        "the misleading-slug row must be the only class-1 hit:\n{rendered}"
    );
    assert_eq!(report.url_mismatches[0].url_company_id, foreign.id);
    assert!(report.url_mismatches[0].owner_named_in_title);

    // (2)
    assert_eq!(
        report.container.document_ids,
        vec![misassociated_doc.id.clone()],
        "HTML stored under a .pdf name is a container mismatch:\n{rendered}"
    );
    assert_eq!(report.container.extension_mismatch, 1);
    assert_eq!(report.container.content_type_mismatch, 1);
    assert_eq!(
        report.container.by_pair.get(&("pdf".to_owned(), "html")),
        Some(&1)
    );

    // (3) — with the content-token evidence: no owner mention, Vercom twice.
    assert_eq!(report.misassociations.len(), 1, "{rendered}");
    let candidate = &report.misassociations[0];
    assert_eq!(candidate.document_id, misassociated_doc.id);
    assert_eq!(candidate.foreign_company_id, foreign.id);
    assert_eq!(candidate.content_owner_hits, Some(0));
    assert_eq!(candidate.content_foreign_hits, Some(2));

    // (4) a flagged row holds the company's only canonical slot — exactly the
    // harm T3 repairs. Both seeded rows classify as FY2024 periodic reports and
    // both are flagged; the structured `.xhtml` wins the slot (ADR 0077 §1
    // tie-break), so the audit must name that one.
    assert_eq!(report.flagged_slots.len(), 1, "{rendered}");
    let slot = &report.flagged_slots[0];
    assert_eq!(slot.company_id, owner.id);
    assert_eq!(slot.fiscal_year, 2024);
    assert_eq!(slot.period_type, "FY");
    assert_eq!(slot.document_id, url_mismatch_doc.id, "{rendered}");

    // (5) one watchlist company, its documents counted.
    assert_eq!(report.coverage.len(), 1, "{rendered}");
    assert_eq!(report.coverage[0].company_id, owner.id);
    assert_eq!(report.coverage[0].documents, 2);
    assert_eq!(report.coverage[0].fetched, 1);
    assert_eq!(report.coverage[0].pending, 1);
    assert_eq!(report.coverage[0].autopilot_mode, "off");

    // (6)
    assert_eq!(report.currency.total, 1, "{rendered}");
    assert_eq!(
        report
            .currency
            .by_group
            .get(&("eps_basic".to_owned(), "shares".to_owned())),
        Some(&1)
    );

    // (7) both companies exist, neither has a kpi_relevance row.
    let mut gaps = report.kpi_relevance_gaps();
    gaps.sort_unstable();
    let mut expected = vec![owner.id.as_str(), foreign.id.as_str()];
    expected.sort_unstable();
    assert_eq!(gaps, expected, "{rendered}");

    std::fs::remove_dir_all(&data_dir).ok();
}
