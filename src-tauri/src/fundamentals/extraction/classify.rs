//! Deterministic document-kind taxonomy over report-document titles/URLs
//! (ADR 0077 §1). `doc_kind` marks which stored documents can carry
//! extractable financial data; canonical-report-per-period selection (T1.3),
//! the coverage map (F2), and the history-sweep selector (F3) build on it,
//! and [`crate::report_diff::classify::classify_statement`] is a thin
//! projection of it so the two can never drift.
//!
//! Matching is two-phase. Companion-file extensions are checked on the raw
//! lowercased text first (dots matter there: a `.xades` signature or `.xbri`
//! data file often repeats the full statement name in its title). Word
//! markers then run over a separator-normalized form — every non-alphanumeric
//! char becomes a space and the text is space-padded — so real GPW filename
//! shapes (`Raport_z_przegladu_SSF_...`, `zal10_SzB_GK_...`) match the same
//! markers as prose titles. Markers are matched with a leading space (word
//! boundary on the left, inflection-tolerant on the right); plain
//! `contains("ssf")` is wrong here because `MSSF` (Polish IFRS) contains it.
//!
//! The behavior contract is the committed labeled corpus
//! `src-tauri/testdata/doc_titles_labeled.json` (guardrail G-2): a marker
//! edit that flips a known-good label reddens `contract_corpus_holds`,
//! forcing a conscious relabel instead of silent reclassification.

use std::collections::BTreeMap;

/// The taxonomy of stored report documents (ADR 0077 §1). `NULL` in the
/// database means "not yet classified" — this function itself is total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocKind {
    /// Consolidated periodic financial statement (SSF / QSr / annual report).
    PeriodicSsf,
    /// Standalone periodic financial statement (JSF / PSF / BSF).
    PeriodicJsf,
    /// Auditor work product: audit report (SzB), review report, opinion, ESG assurance.
    AuditorOpinion,
    /// Results / investor presentation.
    Presentation,
    /// Corporate governance: GM materials, resolutions, supervisory-board
    /// reports and statements, statute, remuneration, policies.
    Governance,
    /// Everything else: announcements, MAR notifications, signature/data
    /// companion files, selected-data extracts, management activity reports.
    Other,
}

impl DocKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DocKind::PeriodicSsf => "periodic_ssf",
            DocKind::PeriodicJsf => "periodic_jsf",
            DocKind::AuditorOpinion => "auditor_opinion",
            DocKind::Presentation => "presentation",
            DocKind::Governance => "governance",
            DocKind::Other => "other",
        }
    }
}

/// Companion-file extensions checked on the raw lowercased text: signatures
/// and machine-readable data files are never the document they accompany,
/// even when the title repeats the full statement name. `.xades` is listed
/// first in the classifier so a `.xbri.xades` signature stays `Other` even
/// though `.xbri` (the ESEF package itself) now routes to a periodic statement.
const COMPANION_EXTENSIONS: &[&str] = &[".xades", ".xbrl", ".csv"];

/// ESEF package extension: an `.xbri` file IS the periodic filing (it bundles
/// the consolidated + standalone iXBRL bodies), so a bare package with no
/// ssf/jsf marker classifies as a periodic statement rather than a companion
/// (ADR 0077 §1 amendment, 2026-07-09). A `.xbri.xades` signature is still
/// caught by the `.xades` companion check, which runs first.
const PACKAGE_EXTENSIONS: &[&str] = &[".xbri"];

/// Auditor work products (audit report SzB, review report, opinion, ESG
/// assurance). Checked before the periodic gate: real SzB titles embed the
/// statement's own markers (`..._BSSF_MSSF_SzB_PL`).
const AUDITOR_MARKERS: &[&str] = &[
    " z badania",
    " szb ",
    " opinia",
    " z przegladu",
    " z przeglądu",
    " przeglad ",
    " przegląd ",
    " atestacj",
    " bieglego rewidenta",
    " biegłego rewidenta",
    // English shapes (#270). The dictionary was Polish-only, so every
    // English-language auditor product read `other` — ORLEN's own
    // `ORLEN__Group_Review_report_30062025_ENG.pdf` classified `auditor_opinion`
    // only by the luck of a foreign Polish slug, and honest title-only reading
    // (epic #229 T3 slug distrust) demoted it. Each marker is anchored to a
    // second auditor word ("report"/"auditor") rather than the bare token, so a
    // periodic title carrying "report" (`CBF_Annual_Report_2024.pdf`) or an
    // auditor-SELECTION announcement (`MB Statement - auditor selection …`, a
    // governance/other document) cannot reach this branch. Apostrophes normalize
    // to a space, so `auditor's` and `auditor’s` both match " auditor s report".
    " review report",
    // The issuer's own English rendering of `Raport z przegladu` — DataWalk's
    // `…_Report_on_Review_JSF.pdf` and Pekao's `Auditor_s_Report_on_Review_of_…`.
    // Needed as a TITLE marker, not just via the slug: the title also carries
    // `JSF`, so the title-precedence rule below would otherwise read the
    // auditor's review report as the statement it reviews.
    " report on review",
    " independent auditor",
    " auditor s report",
    " auditors report",
    " audit report",
    " assurance report",
    " attestation report",
];

/// Corporate-governance documents: GM materials, resolutions,
/// supervisory-board (RN) reports/statements, statute, remuneration, policies.
const GOVERNANCE_MARKERS: &[&str] = &[
    " rady nadzorczej",
    " supervisory board",
    " sprawozdanie rn ",
    " rn ",
    " z oceny",
    " uchwał",
    " uchwal",
    " zwołaniu",
    " zwolaniu",
    " zgromadzen",
    " general meeting",
    " convening",
    " resolution",
    " statut",
    " wynagrodze",
    " remuneration",
    " polityka",
    " policy",
    " lad korporacyjny",
    " ład korporacyjny",
    // Nominative form of the convening notice (#269) — the dictionary only had
    // the locative `zwołaniu` ("o zwołaniu ZWZ"), so the equally common
    // `Zwolanie_ZWZA_…` / `Wniosek o zwołanie NWZ` titles fell through.
    " zwolanie",
    " zwołanie",
];

/// Non-periodic documents whose titles routinely co-occur with financial
/// markers (a management activity report inside an annual filing, an
/// announcement about auditing the financial statements) — they must be
/// routed to `Other` before the periodic gate sees them.
const OTHER_EARLY_MARKERS: &[&str] = &[
    " sprawozdanie zarzadu",
    " sprawozdanie zarządu",
    " z dzialalnosci",
    " z działalności",
    " wybrane dane",
    " umowy o badanie",
    " umowa o badanie",
    " firma audytorska",
    " firmą audytorską",
    " opozni",
    " opóźni",
    " szacunkow",
    " list prezesa",
    " do akcjonariusz",
    // English rendering of `sprawozdanie zarządu z działalności` (#270): KGHM
    // titles its management-board activity report `MB_report_on_activities_of_
    // the_KGHM_Group_PSr_2024.pdf`. The `PSr` cadence token makes it look like a
    // periodic statement, so without this it would be extracted as one —
    // narrative text mined for financial facts.
    " report on activities",
    " report on the activities",
    // --- #269 systematic pass: documents that ride along with a periodic
    // filing and carry its `SSF`/`JSF`/cadence token in their own title, so the
    // periodic gate would claim them and feed narrative text to extraction.
    //
    // Management-board statements/representations attached to the statements
    // (`LPP_JSF_Oswiadczenie_Zarzadu_do_JSF_2025.xhtml`, `NPGM_OZ_SSF_2025…`) —
    // 35 stored rows classified periodic before this.
    " oswiadcz",
    " oświadcz",
    // The activity report (SzD) when its title is CamelCase-glued, which hides
    // the spaced ` sprawozdanie zarzadu` marker above.
    " sprawozdaniezarzadu",
    " sprawozdaniezarządu",
    " szdz ",
    // A sustainability report is explicitly NOT the financial statement, and
    // its title contains ` financial report` (`…_Non_financial_report_2023`).
    " non financial",
    // Selected-data extracts: the dictionary had ` wybrane dane`, which misses
    // the far more common `Wybrane <skonsolidowane|jednostkowe|wstępne> dane`.
    " wybrane skonsolidowane dane",
    " wybrane jednostkowe dane",
    " wybrane wstepne dane",
    " wybrane wstępne dane",
    " dane operacyjne",
    // Results commentary, forecasts and estimates published alongside a report.
    // ` szacunkow` only caught the adjective, not `Szacunki za 4 kw.`.
    " komentarz",
    " prognoz",
    " szacunk",
    " zaproszenie",
    // MAR art. 19 manager-transaction notifications.
    " mar19",
    " art 19 mar",
    " powiadomienie o transakc",
    " biogram",
];

const PRESENTATION_MARKERS: &[&str] = &[" prezentacj", " presentation"];

/// The document is a periodic financial statement at all. `ssf`-family tokens
/// are boundary-matched — `MSSF` (Polish IFRS) must not count.
const FINANCIAL_MARKERS: &[&str] = &[
    " sprawozdanie finansow",
    " financial statement",
    " ssf",
    " jsf",
    " qsr",
    " psr",
    " psf",
    " bsf",
    " bssf",
    // Live-DB harvest 2026-07-09: CDP's PSSF (półroczne skonsolidowane) was
    // falling through to Other — "pssf" was a consolidated marker but not a
    // financial one, and the boundary-matched " ssf" deliberately skips it.
    " pssf",
    " raport kwartalny",
    " raport okresowy",
    " raport roczny",
    " raport srodroczny",
    " raport śródroczny",
    // Live-DB harvest 2026-07-30 (epic #229 T3): XTB names its half-year
    // statements `Raport_polroczne_{jednostkowe,skonsolidowane}_XTB_HY_2025_*`.
    // Every other cadence word was a financial marker except this one, so the
    // half-year statement only classified as periodic when a NEIGHBOURING file's
    // URL slug happened to supply the marker — which is exactly the URL trust
    // this task removes.
    " raport polroczn",
    " raport półroczn",
    // Same harvest: cadence words the dictionary was missing, each anchored to a
    // `raport`/`report` word so the token cannot fire on prose. Without them the
    // owner's own periodic reports classified as `other` UNLESS a neighbouring
    // file's URL slug supplied a marker (Orlen `RAPORT_IH2025`, Digital Network
    // `raport_1Q2025`, cyber_Folks `Report_H1_2024_ENG`).
    " raport ih",
    " raport 1q",
    " raport 2q",
    " raport 3q",
    " raport 4q",
    " report h1",
    " report h2",
    " annual report",
    // --- #269 systematic pass: cadence words the dictionary never had. Before
    // this, a report named by its PERIOD rather than by `SSF`/`sprawozdanie
    // finansowe` was invisible to canonical selection unless a slug rescued it
    // (`Skonsolidowany_raport_za_III_kwartal_2023`, `Interim_report_for_H1_2024`,
    // `GPW Group quarterly report for Q1 2026`). 100 stored rows recovered.
    //
    // Polish quarter/half-year, incl. the `za I kw. 2026` abbreviation. The
    // announcement classes that also carry these words (`Szacunki za 4 kw.`,
    // `Wybrane wstępne dane … za I kwartał`) are routed out by the early-Other
    // markers above, which run first.
    " kwartal",
    " kwartał",
    " kw ",
    " polrocze",
    " półrocze",
    " srodroczn",
    " śródroczn",
    " raport finansow",
    // English forms. ` financial report` is guarded by ` non financial` above;
    // KGHM's monthly `Production-Sales report` matches none of these.
    " quarterly report",
    " interim report",
    " financial report",
];

const CONSOLIDATED_MARKERS: &[&str] =
    &[" skonsolidowan", " consolidated", " ssf", " bssf", " pssf"];

const STANDALONE_MARKERS: &[&str] = &[
    " jednostkow",
    " standalone",
    " separate financial",
    " jsf",
    " psf",
    " bsf",
];

/// File extensions that show up **glued** to the first word of a title (#269).
/// The attachment store concatenates `<filename><document title>` with no
/// separator, so a real title reads
/// `SF_MO-BRUK_…-pl.xhtmlSprawozdanie finansowe jednostkowe za 2025 r.` — the
/// statement's own name is welded to `xhtml` and every `" word"` marker misses
/// it on the left boundary. 945 stored rows carry the glue. Longest-first
/// (`xhtml` before `html`) so the split lands on the real extension.
const GLUED_TITLE_EXTENSIONS: &[&str] =
    &["xhtml", "xades", "xbri", "html", "pdf", "zip", "xml", "csv"];

/// Lowercase, map every non-alphanumeric char to a space, collapse runs, unglue
/// a leading file extension from each token, and pad — so markers written as
/// `" word"` match on a left word boundary while staying inflection-tolerant on
/// the right.
fn normalize(raw: &str) -> String {
    let mapped: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut parts: Vec<&str> = Vec::new();
    for token in mapped.split_whitespace() {
        // Split at most once, and only when the extension is a strict prefix:
        // a bare `…-pl.xbri` token stays whole, `xhtmlsprawozdanie` becomes two.
        // The extensions are ASCII, so slicing at their length is always on a
        // char boundary even when the rest of the token is not.
        if let Some(ext) = GLUED_TITLE_EXTENSIONS
            .iter()
            .find(|e| token.len() > e.len() && token.starts_with(**e))
        {
            parts.push(&token[..ext.len()]);
            parts.push(&token[ext.len()..]);
        } else {
            parts.push(token);
        }
    }
    format!(" {} ", parts.join(" "))
}

fn matches_any(t: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| t.contains(m))
}

/// Whether a routing gate fires, under **title precedence**.
///
/// **The URL is not a signal about THIS document** (epic #229 T3 doctrine,
/// [`docs/data-model.md`] `doc_kind`): the bankier CDN reuses a neighbouring
/// attachment's filename as the slug, so a marker found only in the URL is
/// evidence about some *other* file in the same filing. The gate therefore
/// fires on a title-borne marker outright, but on a URL-only marker only while
/// the title itself does not already say what the document is
/// (`title_speaks_for_itself`) — content-proven on the maintainer's corpus,
/// where Asseco's own `…_jednostkowe_sprawozdanie_finansowe_HY_2023.pdf` is
/// served under a `23-HY-ACP-Review-Report-…` slug and CD PROJEKT's own
/// `…_PSF_PL.xhtml` under an `Oswiadczenia-Zarzadu-…` slug. Orthogonal to
/// [`classify_doc_kind_with_slug_trust`], which only fires when the slug names
/// a *foreign tracked* issuer — these slugs belong to the same filing or an
/// untracked issuer, so nothing else catches them.
fn gate_fires(
    title_norm: &str,
    combined_norm: &str,
    markers: &[&str],
    title_speaks_for_itself: bool,
) -> bool {
    matches_any(combined_norm, markers)
        && (matches_any(title_norm, markers) || !title_speaks_for_itself)
}

/// Management activity-report markers ("sprawozdanie [zarządu] z działalności"
/// / "SzD"). Kept separate from [`FINANCIAL_MARKERS`] because the SzD is NOT a
/// financial statement — it is deliberately routed to [`DocKind::Other`] via
/// [`OTHER_EARLY_MARKERS`] so financial-metrics extraction never picks it. But
/// the MANAGEMENT-HOLDINGS table (management/supervisory-board shareholdings)
/// lives exactly in this document, so the management-holdings extraction
/// selection needs a positive predicate for it (F-A3, owner dogfooding
/// 2026-07-17: KRU's holdings live only in its `SzD_Grupa_KRUK.xhtml`, a
/// `doc_kind='other'` filing the periodic-only selection was skipping).
const MANAGEMENT_REPORT_MARKERS: &[&str] = &[
    " z dzialalnosci",
    " z działalności",
    " sprawozdanie zarzadu",
    " sprawozdanie zarządu",
    " szd ",
];

/// Whether a stored document is a management activity report (SzD) that carries
/// the management-holdings table. Deliberately conservative and orthogonal to
/// [`classify_doc_kind`] (which keeps the SzD as `Other`): it excludes the
/// auditor / governance / presentation / companion documents whose titles can
/// co-mention "działalność", then requires a management-report marker. A false
/// positive only costs one parse attempt that records a benign residual; a false
/// negative silently loses a company's holdings (the KRU class), so the tests
/// pin both KRU and ABE SzD shapes as positives and the RN report as a negative.
pub fn is_management_report(title: &str, url: &str) -> bool {
    let raw = format!("{title} {url}").to_lowercase();
    if COMPANION_EXTENSIONS.iter().any(|ext| raw.contains(ext)) {
        return false;
    }
    let t = normalize(&raw);
    let t_title = normalize(&title.to_lowercase());
    // Same title-precedence as the classifier's auditor gate: a URL-only
    // auditor marker (a neighbouring attachment's slug) must not veto a
    // document whose own title names a management activity report — that veto
    // would silently lose the company's holdings table (the KRU class).
    let title_names_a_management_report = matches_any(&t_title, MANAGEMENT_REPORT_MARKERS);
    if gate_fires(
        &t_title,
        &t,
        AUDITOR_MARKERS,
        title_names_a_management_report,
    ) || matches_any(&t, GOVERNANCE_MARKERS)
        || matches_any(&t, PRESENTATION_MARKERS)
    {
        return false;
    }
    matches_any(&t, MANAGEMENT_REPORT_MARKERS)
}

/// [`classify_doc_kind`] with the URL's slug made **optional** (epic #229 T3,
/// #171): `slug_trusted = false` classifies from the title alone.
///
/// The attachment host reuses one issuer's filename across unrelated filings —
/// content-proven on the maintainer's corpus, where XTB's own H1-2025 statements
/// are served under `DataWalk-…` slugs (the stored bytes name XTB) and
/// cyber_Folks'/Orlen's Q3-2024 statements under `Grupy-Energa` slugs (their PDF
/// `/Author` names the owner). The slug then classifies the WRONG document:
/// `XTB_-_Skonsolidowane_SSF_-_H1_2025.pdf` behind a `…xhtml.xades` slug reads as
/// a signature companion (`Other`, so no canonical slot and no extraction), and
/// `Raport_polroczne_jednostkowe_XTB_HY_2025_PL.pdf` behind a `DataWalk-SSF-…`
/// slug reads as consolidated. Both are decided by another company's filename.
///
/// An **empty title** keeps the URL: a slug is a poor signal, no signal is worse.
/// Callers decide trust with
/// [`crate::storage::TrackedIssuerIndex::url_slug_names_foreign_issuer`], so a
/// slug is only ever dropped on positive evidence that it names someone else.
pub fn classify_doc_kind_with_slug_trust(title: &str, url: &str, slug_trusted: bool) -> DocKind {
    if slug_trusted || title.trim().is_empty() {
        return classify_doc_kind(title, url);
    }
    classify_doc_kind(title, "")
}

/// Classify a stored report document from its title and URL. Total and
/// deterministic; the labeled corpus (G-2) is the behavior contract.
pub fn classify_doc_kind(title: &str, url: &str) -> DocKind {
    let raw = format!("{title} {url}").to_lowercase();
    if COMPANION_EXTENSIONS.iter().any(|ext| raw.contains(ext)) {
        return DocKind::Other;
    }
    let t = normalize(&raw);
    // Auditor gate with **title precedence** (epic #229 T3 doctrine, #270): an
    // auditor marker found only in the URL is about a neighbouring attachment,
    // so it must not demote a document whose own title names a financial
    // statement (or is an ESEF package, which IS the filing). A title-borne
    // marker still wins outright — real SzB titles embed the statement's own
    // markers (`..._BSSF_MSSF_SzB_PL`), which is why this gate runs first.
    let title_lower = title.to_lowercase();
    let t_title = normalize(&title_lower);
    let title_names_a_statement = PACKAGE_EXTENSIONS
        .iter()
        .any(|ext| title_lower.contains(ext))
        || matches_any(&t_title, FINANCIAL_MARKERS);
    // All three routing gates run under title precedence (#269): a marker the
    // title does not carry belongs to the neighbouring attachment whose slug
    // was reused, and must not overrule a title that names a statement.
    if gate_fires(&t_title, &t, AUDITOR_MARKERS, title_names_a_statement) {
        return DocKind::AuditorOpinion;
    }
    if gate_fires(&t_title, &t, GOVERNANCE_MARKERS, title_names_a_statement) {
        return DocKind::Governance;
    }
    if gate_fires(&t_title, &t, OTHER_EARLY_MARKERS, title_names_a_statement) {
        return DocKind::Other;
    }
    if matches_any(&t, PRESENTATION_MARKERS) {
        return DocKind::Presentation;
    }
    // An ESEF `.xbri` package IS the periodic filing even when its bare filename
    // carries no ssf/jsf/financial marker (e.g. `CBF-2025-12-31-1-pl.xbri`), so
    // route it through the periodic branch alongside marker-bearing titles; the
    // default-consolidated fall-through below mirrors the no-marker default.
    let is_package = PACKAGE_EXTENSIONS.iter().any(|ext| raw.contains(ext));
    if is_package || matches_any(&t, FINANCIAL_MARKERS) {
        // **Title-first on the consolidated/standalone axis** (owner decision
        // #275), mirroring the auditor gate above: when the TITLE speaks on the
        // axis it is the whole evidence, because a marker found only in the URL
        // belongs to the neighbouring attachment whose slug was reused. A title
        // silent on both markers still falls back to the combined text — a poor
        // signal beats no signal, and that is the only case a slug decides.
        //
        // Within whichever text is consulted the classic rules are unchanged:
        // consolidated wins when both markers appear (a QSr bundles condensed
        // standalone data inside a consolidated report), and a periodic report
        // with no explicit marker defaults to consolidated — the common case for
        // group reports, and the pre-taxonomy `classify_statement` behavior this
        // projection must preserve.
        let axis = if matches_any(&t_title, CONSOLIDATED_MARKERS)
            || matches_any(&t_title, STANDALONE_MARKERS)
        {
            &t_title
        } else {
            &t
        };
        if matches_any(axis, CONSOLIDATED_MARKERS) {
            return DocKind::PeriodicSsf;
        }
        if matches_any(axis, STANDALONE_MARKERS) {
            return DocKind::PeriodicJsf;
        }
        return DocKind::PeriodicSsf;
    }
    DocKind::Other
}

/// One stored document competing to be its period's canonical report. Period
/// and disclosure keys are computed by callers (coverage map F2, history-sweep
/// selector F3) and passed in, keeping this module pure and caller-agnostic —
/// it must not import `report_diff` or `storage` (`report_diff` already imports
/// this module; the reverse edge would be a cycle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalReportCandidate {
    pub document_id: String,
    pub doc_kind: DocKind,
    /// (fiscal year, period index) in the `report_diff::classify::period_sort_key` shape.
    pub period: (i32, u8),
    /// Domain disclosure date `YYYY-MM-DD` (the `jobs::autopilot::report_disclosure_key`
    /// semantics — publication date, NEVER created_at; guardrail d60305c).
    pub disclosure_key: String,
    /// Structured ESEF/xhtml document (disclosure-date tie-break, ADR 0061 decision 1b).
    pub structured: bool,
}

/// Pick the single canonical report for each fiscal period (ADR 0077 §1). Pure
/// and deterministic — independent of input order. Only the two periodic kinds
/// participate; every other `DocKind` is ignored, so a period with only
/// governance/auditor/presentation documents is absent from the result.
///
/// Within a period the winner is decided by this rule chain, highest first:
/// 1. **Kind** — any `PeriodicSsf` beats any `PeriodicJsf`, regardless of dates
///    (the consolidated statement is the canonical one).
/// 2. **Newest revision** — greater `disclosure_key` (lexicographic on
///    `YYYY-MM-DD`) wins within the winning kind.
/// 3. **Structured tie-break** — on an equal `disclosure_key`, a structured
///    ESEF/xhtml document beats an unstructured one (ADR 0061 decision 1b).
/// 4. **Determinism** — still tied, the smaller `document_id` wins, so the
///    result never depends on input order.
pub fn canonical_reports_per_period(
    candidates: &[CanonicalReportCandidate],
) -> BTreeMap<(i32, u8), &CanonicalReportCandidate> {
    let mut best: BTreeMap<(i32, u8), &CanonicalReportCandidate> = BTreeMap::new();
    for c in candidates {
        if !matches!(c.doc_kind, DocKind::PeriodicSsf | DocKind::PeriodicJsf) {
            continue;
        }
        let wins = match best.get(&c.period) {
            Some(current) => selection_key(c) > selection_key(current),
            None => true,
        };
        if wins {
            best.insert(c.period, c);
        }
    }
    best
}

/// The total ordering behind [`canonical_reports_per_period`]: a candidate with
/// the greater key is the better canonical report. `document_id` is reversed so
/// the *smaller* id breaks a full tie.
fn selection_key(c: &CanonicalReportCandidate) -> (u8, &str, bool, std::cmp::Reverse<&str>) {
    let kind_rank = match c.doc_kind {
        DocKind::PeriodicSsf => 1,
        // Only PeriodicJsf reaches here (callers of this key pre-filter to the
        // two periodic kinds); rank it below ssf.
        _ => 0,
    };
    (
        kind_rank,
        c.disclosure_key.as_str(),
        c.structured,
        std::cmp::Reverse(c.document_id.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Corpus {
        #[allow(dead_code)]
        readme: String,
        entries: Vec<Entry>,
    }

    #[derive(serde::Deserialize)]
    struct Entry {
        title: String,
        url: String,
        kind: String,
    }

    /// #269: the store concatenates `<filename><document title>` with no
    /// separator, welding the statement's own name to the file extension. Real
    /// shapes from the maintainer's database (945 stored rows carry the glue);
    /// every `" word"` marker misses them until `normalize` splits the token.
    #[test]
    fn glued_extension_does_not_hide_the_document_title() {
        assert_eq!(
            classify_doc_kind(
                "SF_MO-BRUK_S_A_MSSF-2025-12-31-1-pl.xhtmlSprawozdanie finansowe jednostkowe za 2025 r.",
                ""
            ),
            DocKind::PeriodicJsf
        );
        assert_eq!(
            classify_doc_kind(
                "2026.03.19 SB statements - Audit Committee.xhtmlSupervisory Board statement",
                ""
            ),
            DocKind::Governance
        );
        // A bare package extension is NOT a glued token — it must stay whole so
        // the ESEF package still routes through the periodic branch.
        assert_eq!(
            classify_doc_kind("CBF-2025-12-31-1-pl.xbri", ""),
            DocKind::PeriodicSsf
        );
        // `xhtml` is split before `html`, so the marker sees the real word.
        assert!(normalize("a.xhtmlopinia").contains(" opinia "));
    }

    /// Epic #229 T3 (#171): a foreign issuer's slug must not classify the
    /// owner's document. Real strings from the maintainer's database — XTB's own
    /// H1-2025 statements are served under DataWalk slugs (content-verified: the
    /// stored bytes name XTB). Case (a) still needs slug distrust to recover the
    /// document; case (b) no longer does — the title-first precedence (#275)
    /// decides the consolidated/standalone axis before trust is consulted.
    #[test]
    fn foreign_slug_does_not_classify_the_owners_document() {
        // (a) A signature-companion slug demotes a real consolidated statement to
        //     `Other` — no canonical slot, no extraction, invisible coverage gap.
        let consolidated = "XTB_-_Skonsolidowane_SSF_-_H1_2025.pdf";
        let xades_slug = "https://www.bankier.pl/static/att/emitent/2025-08/\
                          Skonsolidowane-sprawozdanie-finansowe-Grupy-DataWalk-za-I-polrocze-2025\
                          .xhtml_202508281209683797.xades";
        assert_eq!(
            classify_doc_kind(consolidated, xades_slug),
            DocKind::Other,
            "precondition: the foreign .xades slug wins today"
        );
        assert_eq!(
            classify_doc_kind_with_slug_trust(consolidated, xades_slug, false),
            DocKind::PeriodicSsf,
            "distrusting the slug must recover the owner's consolidated statement"
        );

        // (b) A consolidated slug used to flip a STANDALONE statement to
        //     consolidated, so the wrong document competed for (and could win)
        //     the SSF slot. That is no longer a precondition to work around:
        //     **owner decision #275** extended the title-first precedence to the
        //     consolidated/standalone axis, so `jednostkowe` in the owner's own
        //     title now decides regardless of slug trust. Slug distrust is the
        //     narrower instrument (it needs the issuer index and only fires on a
        //     *foreign tracked* issuer); the precedence covers the same-issuer
        //     and untracked-issuer slugs it cannot see, which is why both paths
        //     must now agree here.
        let standalone = "Raport_polroczne_jednostkowe_XTB_HY_2025_PL.pdf";
        let ssf_slug = "https://www.bankier.pl/static/att/emitent/2025-08/\
                        DataWalk-SSF-2025-06-30-0-pl_202508281209683797.xhtml";
        assert_eq!(
            classify_doc_kind(standalone, ssf_slug),
            DocKind::PeriodicJsf,
            "#275: the title speaks on the axis, so the foreign SSF slug cannot"
        );
        assert_eq!(
            classify_doc_kind_with_slug_trust(standalone, ssf_slug, false),
            DocKind::PeriodicJsf,
            "`jednostkowe` in the owner's own title is the standalone signal"
        );

        // (c) A trusted slug is untouched, and an empty title keeps the URL — a
        //     poor signal beats no signal.
        assert_eq!(
            classify_doc_kind_with_slug_trust(standalone, ssf_slug, true),
            classify_doc_kind(standalone, ssf_slug)
        );
        assert_eq!(
            classify_doc_kind_with_slug_trust("", ssf_slug, false),
            classify_doc_kind("", ssf_slug)
        );
    }

    /// Epic #229 T3 doctrine (#270): an auditor marker that fires **only from
    /// the URL** must not demote a document whose own title names a financial
    /// statement. Real strings from the maintainer's database — the bankier CDN
    /// serves both of these under a *neighbouring* attachment's slug, and
    /// neither is rescued by [`classify_doc_kind_with_slug_trust`] (the ACP slug
    /// is the same issuer's, mBank is not a tracked issuer here).
    #[test]
    fn url_only_auditor_marker_never_demotes_a_titled_statement() {
        // (a) Asseco's own STANDALONE half-year statement, served under the
        //     neighbouring review-report slug. Title-only it is unambiguously
        //     standalone; with the slug attached the ssf/jsf axis still follows
        //     the URL's "consolidated" (the separate, deliberate behavior that
        //     `classify_doc_kind_with_slug_trust` governs) — what this test
        //     pins is that it stays a PERIODIC statement either way.
        let standalone = "Asseco_Poland_jednostkowe_sprawozdanie_finansowe_HY_2023.pdf";
        let review_slug = "https://www.bankier.pl/static/att/emitent/2023-08/\
                           23-HY-ACP-Review-Report-PAS-consolidated-condensed-P-01.22-\
                           _202308241043519806.pdf";
        assert_eq!(classify_doc_kind(standalone, ""), DocKind::PeriodicJsf);
        assert_ne!(
            classify_doc_kind(standalone, review_slug),
            DocKind::AuditorOpinion,
            "a neighbouring attachment's slug must not turn a titled statement into an opinion"
        );
        assert!(matches!(
            classify_doc_kind(standalone, review_slug),
            DocKind::PeriodicSsf | DocKind::PeriodicJsf
        ));

        // (b) The same shape with a Polish title and an English auditor slug.
        assert_eq!(
            classify_doc_kind(
                "Sprawozdanie_finansowe_ABS_2023.xhtml",
                "https://www.bankier.pl/static/att/emitent/2024-02/\
                 mBank-Auditor-s-report-2023-Group_202402290354542351.xhtml"
            ),
            DocKind::PeriodicSsf
        );

        // (c) A title-borne auditor marker still wins outright — real SzB
        //     titles embed the statement's own markers.
        assert_eq!(
            classify_doc_kind("CD_PROJEKT_2024_BSSF_MSSF_SzB_PL.xhtml", ""),
            DocKind::AuditorOpinion
        );

        // (d) The same veto is lifted for the management-activity report, whose
        //     holdings table is lost outright on a false negative (the KRU class).
        assert!(is_management_report(
            "HY'23_Sprawozdanie_Zarzadu_z_dzialalnosci_Grupy_Asseco.pdf",
            review_slug
        ));
    }

    /// G-2: the committed labeled corpus is the taxonomy's contract. Any
    /// marker change that flips a known-good label must fail here and be
    /// resolved by a conscious corpus relabel, never silently.
    #[test]
    fn contract_corpus_holds() {
        let corpus: Corpus =
            serde_json::from_str(include_str!("../../../testdata/doc_titles_labeled.json"))
                .expect("labeled corpus parses");
        assert!(
            corpus.entries.len() >= 50,
            "corpus thinned out — it must stay a representative real-title set"
        );
        let mut mismatches = Vec::new();
        let mut covered = std::collections::BTreeSet::new();
        for e in &corpus.entries {
            covered.insert(e.kind.as_str());
            let got = classify_doc_kind(&e.title, &e.url);
            if got.as_str() != e.kind {
                mismatches.push(format!(
                    "  {:?} -> got {}, labeled {}",
                    e.title,
                    got.as_str(),
                    e.kind
                ));
            }
        }
        assert!(
            mismatches.is_empty(),
            "classify_doc_kind disagrees with the labeled corpus:\n{}",
            mismatches.join("\n")
        );
        for kind in [
            "periodic_ssf",
            "periodic_jsf",
            "auditor_opinion",
            "presentation",
            "governance",
            "other",
        ] {
            assert!(
                covered.contains(kind),
                "corpus must cover every DocKind at least once: missing {kind}"
            );
        }
    }

    fn cand(
        id: &str,
        doc_kind: DocKind,
        period: (i32, u8),
        disclosure_key: &str,
        structured: bool,
    ) -> CanonicalReportCandidate {
        CanonicalReportCandidate {
            document_id: id.to_string(),
            doc_kind,
            period,
            disclosure_key: disclosure_key.to_string(),
            structured,
        }
    }

    /// A period's winning document_id, for terse assertions.
    fn winner_id<'a>(
        map: &BTreeMap<(i32, u8), &'a CanonicalReportCandidate>,
        period: (i32, u8),
    ) -> Option<&'a str> {
        map.get(&period).map(|c| c.document_id.as_str())
    }

    #[test]
    fn management_report_predicate_recognizes_szd_but_not_financials_or_governance() {
        // KRU (F-A3): the holdings table lives only in the SzD — an abbreviated
        // filename ("SZD_Grupa_KRUK_2025rok.xhtml") whose title carries the full
        // "z Działalności" phrase. It is `doc_kind='other'`, so the periodic-only
        // selection skipped it and KRU got zero management-holdings rows.
        assert!(is_management_report(
            "SZD Grupa KRUK 2025rok.xhtml Sprawozdania z Działalności GRUPA KRUK",
            "https://bonnier.pl/static/att/emitent/2026-03/..._SZD_Grupa_KRUK_2025rok.xhtml"
        ));
        // ABE's annual SzD (also doc_kind='other').
        assert!(is_management_report(
            "Sprawozdanie z działalności Zarządu",
            "https://bonnier.pl/.../Y24_25_Sprawozdanie_z_dzialalnos-ci_zarza-du.xhtml"
        ));
        // Financial statements are NOT management reports (their holdings, if any,
        // come via the existing periodic selection — not this predicate).
        assert!(!is_management_report(
            "Skonsolidowane sprawozdanie finansowe",
            "grupakruk_sf_2q_2023.pdf"
        ));
        assert!(!is_management_report(
            "ESEF",
            "esef_ssf_grupakruk_2025_12_31.xbri"
        ));
        // A supervisory-board activity report is governance, never a holdings source.
        assert!(!is_management_report(
            "Sprawozdanie z działalności Rady Nadzorczej",
            "sprawozdanie_rady_nadzorczej.xhtml"
        ));
        // An auditor report that co-mentions działalność stays excluded.
        assert!(!is_management_report(
            "Sprawozdanie z badania sprawozdania z działalności",
            "szb_2024.xhtml"
        ));
        // A companion signature file is never the document it accompanies.
        assert!(!is_management_report(
            "Sprawozdanie z działalności Zarządu",
            "Sprawozdanie_z_dzialalnosci.xhtml.xades"
        ));
    }

    #[test]
    fn ssf_beats_a_newer_jsf() {
        let c = vec![
            cand("jsf", DocKind::PeriodicJsf, (2025, 4), "2026-04-01", true),
            cand("ssf", DocKind::PeriodicSsf, (2025, 4), "2026-03-01", false),
        ];
        assert_eq!(
            winner_id(&canonical_reports_per_period(&c), (2025, 4)),
            Some("ssf")
        );
    }

    #[test]
    fn newest_revision_wins_within_a_kind() {
        let c = vec![
            cand("old", DocKind::PeriodicSsf, (2025, 2), "2025-09-01", true),
            cand("new", DocKind::PeriodicSsf, (2025, 2), "2025-09-30", true),
        ];
        assert_eq!(
            winner_id(&canonical_reports_per_period(&c), (2025, 2)),
            Some("new")
        );
    }

    #[test]
    fn disclosure_tie_prefers_structured() {
        let c = vec![
            cand("flat", DocKind::PeriodicSsf, (2024, 4), "2025-03-15", false),
            cand("esef", DocKind::PeriodicSsf, (2024, 4), "2025-03-15", true),
        ];
        assert_eq!(
            winner_id(&canonical_reports_per_period(&c), (2024, 4)),
            Some("esef")
        );
    }

    #[test]
    fn full_tie_prefers_smaller_document_id() {
        let c = vec![
            cand("bbb", DocKind::PeriodicSsf, (2024, 2), "2024-09-01", true),
            cand("aaa", DocKind::PeriodicSsf, (2024, 2), "2024-09-01", true),
        ];
        assert_eq!(
            winner_id(&canonical_reports_per_period(&c), (2024, 2)),
            Some("aaa")
        );
    }

    #[test]
    fn periods_group_independently() {
        let c = vec![
            cand("p1jsf", DocKind::PeriodicJsf, (2025, 1), "2025-05-01", true),
            cand("p1ssf", DocKind::PeriodicSsf, (2025, 1), "2025-05-01", true),
            cand("p2ssf", DocKind::PeriodicSsf, (2025, 3), "2025-11-01", true),
        ];
        let map = canonical_reports_per_period(&c);
        assert_eq!(map.len(), 2);
        assert_eq!(winner_id(&map, (2025, 1)), Some("p1ssf"));
        assert_eq!(winner_id(&map, (2025, 3)), Some("p2ssf"));
    }

    #[test]
    fn non_periodic_kinds_are_ignored() {
        let c = vec![
            cand("gov", DocKind::Governance, (2025, 4), "2026-03-01", true),
            cand(
                "aud",
                DocKind::AuditorOpinion,
                (2025, 4),
                "2026-03-01",
                true,
            ),
            cand("pres", DocKind::Presentation, (2025, 4), "2026-03-01", true),
            cand("misc", DocKind::Other, (2025, 4), "2026-03-01", true),
        ];
        assert!(canonical_reports_per_period(&c).is_empty());
    }

    #[test]
    fn empty_input_yields_empty_map() {
        assert!(canonical_reports_per_period(&[]).is_empty());
    }

    #[test]
    fn canonical_selection_golden() {
        // A representative set (ADR 0049 golden): a revision pair, ssf-over-jsf,
        // a disclosure-date tie broken by `structured`, and a period whose only
        // documents are non-periodic (must be absent). Snapshot is period ->
        // (document_id, kind, disclosure_key) for the selected canonical report.
        let candidates = vec![
            // (2025, 4): ssf beats a strictly newer jsf.
            cand(
                "A1_ssf",
                DocKind::PeriodicSsf,
                (2025, 4),
                "2026-03-10",
                true,
            ),
            cand(
                "A2_jsf",
                DocKind::PeriodicJsf,
                (2025, 4),
                "2026-03-25",
                true,
            ),
            // (2025, 2): revision pair — the later disclosure wins.
            cand(
                "B1_ssf",
                DocKind::PeriodicSsf,
                (2025, 2),
                "2025-09-01",
                false,
            ),
            cand(
                "B2_ssf",
                DocKind::PeriodicSsf,
                (2025, 2),
                "2025-09-18",
                false,
            ),
            // (2024, 4): same kind + disclosure date — structured wins.
            cand(
                "C1_ssf",
                DocKind::PeriodicSsf,
                (2024, 4),
                "2025-03-01",
                false,
            ),
            cand(
                "C2_ssf",
                DocKind::PeriodicSsf,
                (2024, 4),
                "2025-03-01",
                true,
            ),
            // (2023, 4): only non-periodic documents — absent from the map.
            cand(
                "D1_aud",
                DocKind::AuditorOpinion,
                (2023, 4),
                "2024-03-01",
                true,
            ),
            cand("D2_gov", DocKind::Governance, (2023, 4), "2024-03-01", true),
        ];
        let rows: Vec<_> = canonical_reports_per_period(&candidates)
            .iter()
            .map(|(period, c)| {
                (
                    *period,
                    c.document_id.as_str(),
                    c.doc_kind.as_str(),
                    c.disclosure_key.as_str(),
                )
            })
            .collect();
        insta::assert_debug_snapshot!(rows);
    }
}

#[cfg(test)]
mod proptests {
    //! Invariant coverage (ADR 0049): arbitrary title/url text, including
    //! unicode, must never panic — same class as the `parse_year`
    //! char-boundary panic in `report_diff::classify`.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn classify_doc_kind_never_panics(title in ".*", url in ".*") {
            let _ = classify_doc_kind(&title, &url);
        }
    }

    fn arb_doc_kind() -> impl Strategy<Value = DocKind> {
        prop_oneof![
            Just(DocKind::PeriodicSsf),
            Just(DocKind::PeriodicJsf),
            Just(DocKind::AuditorOpinion),
            Just(DocKind::Presentation),
            Just(DocKind::Governance),
            Just(DocKind::Other),
        ]
    }

    fn arb_candidate() -> impl Strategy<Value = CanonicalReportCandidate> {
        (
            "[a-z0-9]{1,6}",
            arb_doc_kind(),
            2000i32..2030,
            0u8..5,
            "2[0-9]{3}-[0-1][0-9]-[0-3][0-9]",
            any::<bool>(),
        )
            .prop_map(
                |(document_id, doc_kind, year, period, disclosure_key, structured)| {
                    CanonicalReportCandidate {
                        document_id,
                        doc_kind,
                        period: (year, period),
                        disclosure_key,
                        structured,
                    }
                },
            )
    }

    /// Owned value projection so results from two different slices compare.
    fn project(
        m: &BTreeMap<(i32, u8), &CanonicalReportCandidate>,
    ) -> BTreeMap<(i32, u8), (String, DocKind, String, bool)> {
        m.iter()
            .map(|(k, c)| {
                (
                    *k,
                    (
                        c.document_id.clone(),
                        c.doc_kind,
                        c.disclosure_key.clone(),
                        c.structured,
                    ),
                )
            })
            .collect()
    }

    /// Deterministic Fisher-Yates permutation from a seed — no rand dependency.
    fn permute(items: &[CanonicalReportCandidate], seed: u64) -> Vec<CanonicalReportCandidate> {
        let mut v = items.to_vec();
        let mut state = seed | 1;
        for i in (1..v.len()).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = ((state >> 33) as usize) % (i + 1);
            v.swap(i, j);
        }
        v
    }

    proptest! {
        /// Order independence: any permutation of the input yields the same map.
        #[test]
        fn permutation_never_changes_result(
            candidates in prop::collection::vec(arb_candidate(), 0..12),
            seed in any::<u64>(),
        ) {
            let baseline = project(&canonical_reports_per_period(&candidates));
            let shuffled = permute(&candidates, seed);
            let got = project(&canonical_reports_per_period(&shuffled));
            prop_assert_eq!(baseline, got);
        }

        /// Never panics on arbitrary candidate data.
        #[test]
        fn canonical_reports_never_panics(
            candidates in prop::collection::vec(arb_candidate(), 0..16),
        ) {
            let _ = canonical_reports_per_period(&candidates);
        }

        /// Every winner is a periodic document keyed under its own period and
        /// present in the input.
        #[test]
        fn winners_are_periodic_and_present(
            candidates in prop::collection::vec(arb_candidate(), 0..16),
        ) {
            let map = canonical_reports_per_period(&candidates);
            for (period, winner) in &map {
                prop_assert!(matches!(
                    winner.doc_kind,
                    DocKind::PeriodicSsf | DocKind::PeriodicJsf
                ));
                prop_assert_eq!(*period, winner.period);
                prop_assert!(candidates.iter().any(|c| std::ptr::eq(c, *winner)));
            }
        }
    }
}
