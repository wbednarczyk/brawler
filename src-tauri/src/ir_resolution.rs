//! AI-assisted IR-page report resolution (v0.36.0, ADR 0029).
//!
//! Source ladder fallback: when a filing carries no usable attachment, locate the
//! specific report on a company's durable IR reports page. Generic link extraction
//! (no per-company scrapers) plus an AI pick over the candidate links; a confident
//! pick is captured into `report_documents`, otherwise the candidates are returned
//! for the user to choose. Wiring this to fire automatically on detection is the
//! v0.47.0 autonomous pipeline; here it is user-triggered.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    app_state, document_fetcher::DocumentFetcher, providers::analysis, report_documents_capture,
    storage,
};

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
    /// Set when a confident pick was captured into report_documents.
    pub document: Option<storage::ReportDocument>,
    /// The candidate links found on the IR page (always returned for user pick).
    pub candidates: Vec<IrReportCandidate>,
    /// The URL the AI picked, when it matched a candidate.
    pub picked_url: Option<String>,
    /// low | medium | high, when the model reported one.
    pub confidence: Option<String>,
}

pub fn resolve_ir_report(
    state: &app_state::AppState,
    fetcher: &dyn DocumentFetcher,
    input: ResolveIrReportInput,
) -> Result<IrReportResolution, String> {
    let provider = build_provider(state)?;
    resolve_with_provider(state, fetcher, provider.as_ref(), input)
}

fn resolve_with_provider(
    state: &app_state::AppState,
    fetcher: &dyn DocumentFetcher,
    provider: &dyn analysis::AiAnalysisProvider,
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

    if candidates.is_empty() {
        return Ok(IrReportResolution {
            document: None,
            candidates,
            picked_url: None,
            confidence: None,
        });
    }

    let prompt = build_resolution_prompt(&input, &candidates);
    let candidate_block = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| format!("{}. {} | {}", index + 1, candidate.url, candidate.label))
        .collect::<Vec<_>>()
        .join("\n");

    let text = tauri::async_runtime::block_on(provider.complete_document(
        &prompt,
        &analysis::AnalysisDocument::Text {
            text: candidate_block,
        },
    ))
    .map_err(|error| error.to_string())?;

    let (best_url, confidence) = parse_resolution_output(&text);
    // The model must pick from the candidates we provided; ignore anything else.
    let picked_url =
        best_url.filter(|url| candidates.iter().any(|candidate| &candidate.url == url));
    let auto_capture = matches!(confidence.as_deref(), Some("high") | Some("medium"));

    let document = match (&picked_url, auto_capture) {
        (Some(url), true) => {
            let result = report_documents_capture::capture_report_document(
                state,
                fetcher,
                storage::CaptureReportDocumentInput {
                    company_id: input.company_id.clone(),
                    source_type: "ir_page".to_owned(),
                    url: url.clone(),
                    period_id: None,
                    origin_ref: None,
                    title: None,
                    attribution: Some(ir_url.clone()),
                },
            )
            .map_err(|error| error.to_string())?;
            state.get_report_document(&result.document_id).ok()
        }
        _ => None,
    };

    Ok(IrReportResolution {
        document,
        candidates,
        picked_url,
        confidence,
    })
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

fn build_resolution_prompt(
    input: &ResolveIrReportInput,
    candidates: &[IrReportCandidate],
) -> String {
    let period = input.period_hint.as_deref().unwrap_or("unknown");
    let report_type = input.report_type.as_deref().unwrap_or("periodic report");
    let published_at = input.published_at.as_deref().unwrap_or("unknown");
    let candidate_block = candidates
        .iter()
        .map(|candidate| format!("- {} | {}", candidate.url, candidate.label))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are given candidate report links scraped from a company's investor-relations \
reports page. Pick the single link that is the official report document matching the target. \
Return only JSON with this exact shape: {{\"bestUrl\":\"<one of the candidate URLs, or empty if none match>\",\"confidence\":\"low|medium|high\"}}. \
Choose bestUrl strictly from the provided candidate URLs. If none clearly match, return an empty bestUrl with low confidence. Do not include commentary or markdown fences.\n\n\
Target report:\n\
- period: {period}\n\
- type: {report_type}\n\
- published: {published_at}\n\n\
Candidate report links:\n{candidate_block}"
    )
}

fn parse_resolution_output(text: &str) -> (Option<String>, Option<String>) {
    let json_text = match crate::providers::common::extract_json_object(text, "IR resolution") {
        Ok(json_text) => json_text,
        Err(_) => return (None, None),
    };
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Output {
        best_url: Option<String>,
        confidence: Option<String>,
    }
    let parsed: Output = match serde_json::from_str(json_text) {
        Ok(parsed) => parsed,
        Err(_) => return (None, None),
    };
    let best_url = parsed
        .best_url
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty());
    let confidence = parsed
        .confidence
        .map(|value| value.trim().to_lowercase())
        .filter(|value| ["low", "medium", "high"].contains(&value.as_str()));
    (best_url, confidence)
}

fn build_provider(
    state: &app_state::AppState,
) -> Result<Box<dyn analysis::AiAnalysisProvider>, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let provider_id = settings
        .ai_providers
        .general_analysis_provider
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| analysis::TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned());
    let model = if provider_id == analysis::TEST_SAMPLE_ANALYSIS_PROVIDER_ID {
        analysis::TEST_SAMPLE_ANALYSIS_MODEL.to_owned()
    } else {
        settings.ai_providers.general_analysis_model.clone()
    };
    let timeout_seconds = settings.ai_providers.general_analysis_timeout_seconds;
    let api_key = analysis::registry::read_analysis_provider_api_key(&provider_id);
    // Only consulted for the OpenAI-compatible provider (ADR 0060); ignored otherwise.
    let openai_compatible_base_url = Some(settings.ai_providers.openai_compatible_base_url.trim())
        .filter(|value| !value.is_empty());
    analysis::registry::build_analysis_provider(
        &provider_id,
        api_key,
        &model,
        timeout_seconds,
        openai_compatible_base_url,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_fetcher::FakeDocumentFetcher;
    use crate::providers::analysis::{TestSampleAnalysisProvider, TEST_SAMPLE_IR_PICK_URL};
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
        assert!(candidates.iter().any(|c| c.url == TEST_SAMPLE_IR_PICK_URL));
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

    #[test]
    fn parses_pick_and_ignores_garbage() {
        let (url, confidence) =
            parse_resolution_output(r#"{"bestUrl":"https://x/y.pdf","confidence":"HIGH"}"#);
        assert_eq!(url.as_deref(), Some("https://x/y.pdf"));
        assert_eq!(confidence.as_deref(), Some("high"));

        let (none_url, none_conf) = parse_resolution_output("not json");
        assert!(none_url.is_none() && none_conf.is_none());
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

    #[test]
    fn resolve_picks_and_captures_the_matching_report() {
        let (state, company_id) = company_state();
        state
            .set_company_ir_reports_url(&company_id, Some("https://example.com/investors/"))
            .expect("set ir url");
        // First fetch returns the IR page HTML; capture re-fetches the chosen PDF.
        // FakeDocumentFetcher returns the same bytes for any URL, which is fine here.
        let fetcher = FakeDocumentFetcher::new_success(
            SAMPLE_HTML.as_bytes().to_vec(),
            Some("text/html".to_owned()),
        );

        let resolution = resolve_with_provider(
            &state,
            &fetcher,
            &TestSampleAnalysisProvider,
            ResolveIrReportInput {
                company_id,
                period_hint: Some("Q3 2025".to_owned()),
                report_type: Some("periodic report".to_owned()),
                published_at: None,
            },
        )
        .expect("resolution succeeds");

        assert!(!resolution.candidates.is_empty());
        assert_eq!(
            resolution.picked_url.as_deref(),
            Some(TEST_SAMPLE_IR_PICK_URL)
        );
        assert_eq!(resolution.confidence.as_deref(), Some("high"));
        let document = resolution.document.expect("confident pick is captured");
        assert_eq!(document.source_type, "ir_page");
        assert_eq!(document.url, TEST_SAMPLE_IR_PICK_URL);
    }
}
