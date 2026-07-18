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
    " annual report",
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

/// Lowercase, map every non-alphanumeric char to a space, collapse runs, and
/// pad — so markers written as `" word"` match on a left word boundary while
/// staying inflection-tolerant on the right.
fn normalize(raw: &str) -> String {
    let mapped: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let collapsed = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(" {collapsed} ")
}

fn matches_any(t: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| t.contains(m))
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
    if matches_any(&t, AUDITOR_MARKERS)
        || matches_any(&t, GOVERNANCE_MARKERS)
        || matches_any(&t, PRESENTATION_MARKERS)
    {
        return false;
    }
    matches_any(&t, MANAGEMENT_REPORT_MARKERS)
}

/// Classify a stored report document from its title and URL. Total and
/// deterministic; the labeled corpus (G-2) is the behavior contract.
pub fn classify_doc_kind(title: &str, url: &str) -> DocKind {
    let raw = format!("{title} {url}").to_lowercase();
    if COMPANION_EXTENSIONS.iter().any(|ext| raw.contains(ext)) {
        return DocKind::Other;
    }
    let t = normalize(&raw);
    if matches_any(&t, AUDITOR_MARKERS) {
        return DocKind::AuditorOpinion;
    }
    if matches_any(&t, GOVERNANCE_MARKERS) {
        return DocKind::Governance;
    }
    if matches_any(&t, OTHER_EARLY_MARKERS) {
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
        // Consolidated wins when both markers appear (a QSr bundles condensed
        // standalone data inside a consolidated report); a periodic report
        // with no explicit marker defaults to consolidated — the common case
        // for group reports, and the pre-taxonomy `classify_statement`
        // behavior this projection must preserve.
        if matches_any(&t, CONSOLIDATED_MARKERS) {
            return DocKind::PeriodicSsf;
        }
        if matches_any(&t, STANDALONE_MARKERS) {
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
