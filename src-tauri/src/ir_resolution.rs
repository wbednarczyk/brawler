//! IR-page report resolution (ADR 0029; AI pick retired by ADR 0084).
//!
//! Source ladder fallback: when a filing carries no usable attachment, locate the
//! specific report on a company's durable IR reports page. Generic link
//! extraction (no per-company scrapers) ranks the candidate links deterministically
//! and **always returns them for the user to choose** (ADR 0084 decision 1
//! retired the AI pick and its confidence-gated auto-capture). Ranking a
//! candidate list is deterministic; choosing from it is the user's (or their
//! MCP agent's) call.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{app_state, document_fetcher::DocumentFetcher};

const MAX_CANDIDATES: usize = 40;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ResolveIrReportInput {
    pub company_id: String,
    pub period_hint: Option<String>,
    pub report_type: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct IrReportCandidate {
    pub url: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct IrReportResolution {
    /// The candidate links found on the IR page, ranked most-report-like first
    /// and always returned for the user to choose (ADR 0084: nothing is
    /// auto-picked).
    pub candidates: Vec<IrReportCandidate>,
}

pub fn resolve_ir_report(
    state: &app_state::AppState,
    fetcher: &dyn DocumentFetcher,
    input: ResolveIrReportInput,
) -> Result<IrReportResolution, String> {
    let ir_url = state
        .get_company_ir_reports_url(&input.company_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "This company has no investor-relations reports page URL set.".to_owned())?;

    let fetched = fetcher
        .fetch(&ir_url)
        .map_err(|error| format!("failed to fetch the IR page: {error}"))?;
    let html = String::from_utf8_lossy(&fetched.bytes);
    let candidates = extract_candidate_links(&html, &ir_url, MAX_CANDIDATES);

    Ok(IrReportResolution { candidates })
}

/// Generic anchor extraction: absolute-resolved, de-duplicated, lightly ranked so
/// report-looking links come first. No per-company selectors (ADR 0029).
fn extract_candidate_links(html: &str, base_url: &str, limit: usize) -> Vec<IrReportCandidate> {
    let base = Url::parse(base_url).ok();
    // The IR landing page routinely links back to itself (logo, breadcrumb,
    // "Reports" nav). That self-link is never the report — excluding it stops the
    // resolver from ever capturing the landing page itself as a report document
    // (issue 3d9f7f9).
    let self_key = base.as_ref().map(normalize_for_self_compare);
    let document = Html::parse_document(html);
    let selector = match Selector::parse("a[href]") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };

    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for element in document.select(&selector) {
        let href = match element.value().attr("href") {
            Some(href) => href.trim(),
            None => continue,
        };
        if href.is_empty()
            || href.starts_with('#')
            || href.starts_with("javascript:")
            || href.starts_with("mailto:")
        {
            continue;
        }
        let absolute = match &base {
            Some(base) => match base.join(href) {
                Ok(url) => url.to_string(),
                Err(_) => continue,
            },
            None => href.to_owned(),
        };
        if !absolute.starts_with("http") || !seen.insert(absolute.clone()) {
            continue;
        }
        // Drop self-references to the IR landing page (path-equal, ignoring
        // fragment/query/trailing slash).
        if let (Some(self_key), Ok(candidate_url)) = (&self_key, Url::parse(&absolute)) {
            if &normalize_for_self_compare(&candidate_url) == self_key {
                continue;
            }
        }
        let label = element
            .text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        candidates.push(IrReportCandidate {
            url: absolute,
            label,
        });
    }

    // Hard bias to PDFs: a real periodic report is a PDF, so PDF links rank first,
    // then other report-looking links (keywords), then everything else.
    candidates.sort_by_key(candidate_rank);
    candidates.truncate(limit);
    candidates
}

/// Scheme + host + path (trailing slash trimmed), ignoring query and fragment —
/// used to recognise a link that points back at the IR page itself.
fn normalize_for_self_compare(url: &Url) -> String {
    let host = url.host_str().unwrap_or("");
    let path = url.path().trim_end_matches('/');
    format!("{}://{}{}", url.scheme(), host, path)
}

/// True when the URL's path (ignoring query/fragment) ends in `.pdf`.
fn is_pdf_url(url: &str) -> bool {
    url.split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_lowercase()
        .ends_with(".pdf")
}

/// Ranking key (lower is better): PDFs first, then keyword-matching report links,
/// then everything else.
fn candidate_rank(candidate: &IrReportCandidate) -> u8 {
    if is_pdf_url(&candidate.url) {
        0
    } else if looks_like_report(candidate) {
        1
    } else {
        2
    }
}

fn looks_like_report(candidate: &IrReportCandidate) -> bool {
    let haystack = format!("{} {}", candidate.url, candidate.label).to_lowercase();
    is_pdf_url(&candidate.url)
        || [
            "raport",
            "report",
            "wyniki",
            "result",
            "quarter",
            "annual",
            "kwart",
            "rocz",
            "sprawozda",
            "okresow",
            "financ",
            "finans",
        ]
        .iter()
        .any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_fetcher::FakeDocumentFetcher;
    use crate::storage::{open_in_memory_database, AppState, NewCompany};

    const SAMPLE_HTML: &str = r##"
        <html><body>
            <a href="/about">About us</a>
            <a href="https://reports.example.com/q3-2025.pdf">Skonsolidowany raport okresowy Q3 2025</a>
            <a href="https://reports.example.com/q2-2025.pdf">Raport Q2 2025</a>
            <a href="mailto:ir@example.com">Contact IR</a>
            <a href="#top">Back to top</a>
        </body></html>
    "##;

    #[test]
    fn extracts_absolute_deduped_links_ranked_by_report_likeness() {
        let candidates = extract_candidate_links(SAMPLE_HTML, "https://example.com/investors/", 40);
        assert!(candidates.iter().all(|c| c.url.starts_with("http")));
        assert!(candidates.iter().all(|c| !c.url.contains("mailto")));
        // Report-looking links rank ahead of the plain "About us" link.
        assert!(candidates[0].url.ends_with(".pdf"));
        assert!(candidates
            .iter()
            .any(|c| c.url == "https://reports.example.com/q3-2025.pdf"));
    }

    #[test]
    fn excludes_links_back_to_the_ir_landing_page() {
        // The landing page often links to itself; that self-link must never become
        // a candidate (issue 3d9f7f9), across trailing-slash/query/fragment variants.
        let html = r##"
            <html><body>
                <a href="https://example.com/investors/">Investor relations</a>
                <a href="/investors">Reports</a>
                <a href="https://example.com/investors/?utm=nav#top">Home</a>
                <a href="https://reports.example.com/q3-2025.pdf">Raport Q3 2025</a>
            </body></html>
        "##;
        let candidates = extract_candidate_links(html, "https://example.com/investors/", 40);
        assert!(
            candidates
                .iter()
                .all(|c| !c.url.contains("example.com/investors")),
            "the IR landing page itself must not be a candidate: {candidates:?}"
        );
        assert!(candidates.iter().any(|c| c.url.ends_with("q3-2025.pdf")));
    }

    #[test]
    fn ranks_pdf_candidates_ahead_of_keyword_html_links() {
        // Both links read as report-ish, but the PDF must win — bias hard to PDFs.
        let html = r##"
            <html><body>
                <a href="https://example.com/results-overview">Quarterly results overview</a>
                <a href="https://reports.example.com/q3-2025.pdf">Q3 2025</a>
            </body></html>
        "##;
        let candidates = extract_candidate_links(html, "https://example.com/investors/", 40);
        assert!(
            candidates[0].url.ends_with(".pdf"),
            "a PDF report must rank ahead of a keyword-only HTML link: {candidates:?}"
        );
    }

    fn company_state() -> (AppState, String) {
        let dir = std::env::temp_dir().join(format!("brawler-ir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = AppState::with_data_dir(open_in_memory_database().expect("db"), dir);
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
        (state, company.id)
    }

    #[test]
    fn resolve_errors_without_ir_url() {
        let (state, company_id) = company_state();
        let fetcher = FakeDocumentFetcher::new_success(Vec::new(), None);
        let error = resolve_ir_report(
            &state,
            &fetcher,
            ResolveIrReportInput {
                company_id,
                period_hint: None,
                report_type: None,
                published_at: None,
            },
        )
        .expect_err("missing IR url errors");
        assert!(error.contains("investor-relations"));
    }
}
