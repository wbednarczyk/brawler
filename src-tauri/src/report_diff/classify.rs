//! Classify stored report documents into diffable financial statements and order
//! them chronologically (v0.47.0, ADR 0052). Pure, deterministic, and tested:
//! consecutive same-type pairs are what the diff compares.
//!
//! `period_id` is frequently null on real backfilled documents, so the period is
//! parsed from the title/URL; documents whose period cannot be parsed fall back to
//! `created_at` ordering so they still pair, just less precisely.

use serde::Serialize;

use crate::fundamentals::extraction::classify::{classify_doc_kind, DocKind};

/// The financial-statement type the diff aligns within (never across types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum StatementType {
    /// Skonsolidowane sprawozdanie finansowe — consolidated.
    Ssf,
    /// Jednostkowe sprawozdanie finansowe — standalone.
    Jsf,
}

impl StatementType {
    pub fn as_str(self) -> &'static str {
        match self {
            StatementType::Ssf => "ssf",
            StatementType::Jsf => "jsf",
        }
    }
}

/// Classify a document as a diffable financial statement, or `None` if it is not
/// one (announcements, audit agreements, management reports, etc. are excluded).
///
/// A thin projection of the document taxonomy (ADR 0077 §1): only the two
/// periodic-statement kinds are diffable, everything else — auditor work
/// products, governance documents, presentations, filing companions — is not.
/// The exclusion/marker rationale and its G-2 contract corpus live in
/// [`crate::fundamentals::extraction::classify`], so the diff's notion of "a
/// statement" cannot drift from the taxonomy's.
pub fn classify_statement(title: &str, url: &str) -> Option<StatementType> {
    // DIFF capability limit, NOT taxonomy: the report diff compares parsed
    // statement bodies, and an ESEF package (`.xbri`) or a zipped filing is not a
    // body the diff can read. The taxonomy (`classify_doc_kind`, ADR 0077 §1
    // amendment 2026-07-09) now classifies a bare `.xbri` package as a periodic
    // statement, but the diff must still exclude it — hence this projection-side
    // exclusion, documented and tested here, which does not touch the taxonomy.
    let raw = format!("{title} {url}").to_lowercase();
    if raw.contains(".xbri") || raw.contains(".zip") {
        return None;
    }
    match classify_doc_kind(title, url) {
        DocKind::PeriodicSsf => Some(StatementType::Ssf),
        DocKind::PeriodicJsf => Some(StatementType::Jsf),
        _ => None,
    }
}

/// A sortable chronological key parsed from the title/URL: `(year, period_index)`
/// where period_index is 1..=4 for quarters, 2 for H1, 4 for annual. `None` when
/// no period can be parsed.
pub fn period_sort_key(title: &str, url: &str) -> Option<(i32, u8)> {
    let t = format!("{} {}", title, url).to_lowercase();
    let year = parse_year(&t)?;
    let period = parse_period(&t);
    Some((year, period))
}

/// A short human label for the period, for the candidate read model.
pub fn period_label(title: &str, url: &str) -> Option<String> {
    let (year, period) = period_sort_key(title, url)?;
    let suffix = match period {
        1 => "Q1",
        2 => "Q2/H1",
        3 => "Q3",
        4 => "Q4/FY",
        // Unknown intra-year period: show just the year rather than a bare "?".
        _ => return Some(year.to_string()),
    };
    Some(format!("{year} {suffix}"))
}

fn parse_year(t: &str) -> Option<i32> {
    // First standalone 4-digit year in 2000..=2099.
    let bytes = t.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i].is_ascii_digit() {
            // `i` is a char boundary (it points at an ASCII digit), but `i + 4` is a raw
            // byte offset that can land inside a multi-byte char (e.g. a Polish diacritic
            // shortly after a digit). A 4-digit year can only ever be ASCII, so when the
            // window isn't a valid boundary it can't be a match anyway — skip it instead
            // of slicing into a multi-byte char and panicking.
            if t.is_char_boundary(i + 4) {
                let chunk = &t[i..i + 4];
                if let Ok(y) = chunk.parse::<i32>() {
                    if (2000..=2099).contains(&y) {
                        return Some(y);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_period(t: &str) -> u8 {
    if t.contains("q1") || t.contains("i kwarta") || t.contains("1 kwarta") || t.contains("1q") {
        1
    } else if t.contains("q3")
        || t.contains("iii kwarta")
        || t.contains("3 kwarta")
        || t.contains("3q")
    {
        3
    } else if t.contains("q4") || t.contains("roczn") || t.contains("annual") {
        4
    } else if t.contains("q2")
        || t.contains("psr")
        || t.contains("półrocz")
        || t.contains("polrocz")
        || t.contains("h1")
        // The title form "1H" (e.g. "za_1H_2023") — the mirror of the already-
        // present "h1"; bare `contains` matches the existing arm's idiom.
        || t.contains("1h")
        || t.contains("ii kwarta")
    {
        2
    } else {
        // Unknown intra-year period; sort before Q1 so it is not mistaken for newer.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_consolidated_and_standalone() {
        assert_eq!(
            classify_statement("cyber_Folks 2026 Q1 SSF", "x/SSF.pdf"),
            Some(StatementType::Ssf)
        );
        assert_eq!(
            classify_statement("cyber_Folks 2026 Q1 JSF", "x/JSF.pdf"),
            Some(StatementType::Jsf)
        );
    }

    #[test]
    fn excludes_non_statements() {
        assert_eq!(
            classify_statement("Podpisanie umowy o badanie", "x.xhtml"),
            None
        );
        assert_eq!(
            classify_statement("Opóźnienie publikacji raportu rocznego", "x.pdf"),
            None
        );
        assert_eq!(
            classify_statement("Szacunkowa wysokość przychodów", "x.pdf"),
            None
        );
        assert_eq!(classify_statement("Projekty uchwał ZWZ", "x.pdf"), None);
    }

    #[test]
    fn excludes_real_cbf_annual_filing_components() {
        // Supervisory-board report, audit report, and signature/data files bundled in
        // CBF's real annual filing must NOT be treated as the financial statement.
        assert_eq!(
            classify_statement("26_03_17_sprawozdanie_RN_cyber_FolksSA_z_oceny", "x.pdf"),
            None
        );
        assert_eq!(
            classify_statement("SzB_GK_cyber_Folks Sprawozdanie z badania", "x.xhtml"),
            None
        );
        assert_eq!(
            classify_statement("JSF_CBF-2025-12-31-1-pl.xhtml.xades", "x"),
            None
        );
        assert_eq!(classify_statement("CBF-2025-12-31-1-pl.xbri", "x"), None);
        // ...but the actual standalone ESEF statement is kept.
        assert_eq!(
            classify_statement("JSF_CBF Jednostkowe Sprawozdanie finansowe", "x.xhtml"),
            Some(StatementType::Jsf)
        );
    }

    #[test]
    fn esef_package_is_a_statement_in_taxonomy_but_excluded_from_diff() {
        // The taxonomy classifies a bare ESEF `.xbri` package as a periodic
        // statement (ADR 0077 §1 amendment), but the diff cannot compare a
        // package body, so `classify_statement` must still return `None` for it.
        // This is the no-diff-regression proof paired with the unchanged golden.
        assert_eq!(
            classify_doc_kind("CBF-2025-12-31-1-pl.xbri", ""),
            DocKind::PeriodicSsf
        );
        assert_eq!(classify_statement("CBF-2025-12-31-1-pl.xbri", ""), None);
        assert_eq!(
            classify_statement("Skonsolidowany raport 2025", "x.zip"),
            None
        );
        // A `.xbri.xades` signature is already `Other` in the taxonomy — excluded
        // by both layers.
        assert_eq!(
            classify_statement("CBF-2025-12-31-1-pl.xbri.xades", "x"),
            None
        );
    }

    #[test]
    fn classification_corpus_golden() {
        // A representative slice of real GPW/NewConnect filing-component title shapes
        // (the diffable statements + the surrounding filing noise the diff must reject),
        // captured as a golden snapshot. Per ADR 0049 a classification transform ships a
        // golden snapshot; this closes the "denylist rot silently reclassifies" regression
        // class from the v0.47.0 retro — an edit to the exclusion list or the financial
        // markers that flips a known-good classification now surfaces as a snapshot diff,
        // forcing a conscious review instead of a silent behavior change.
        let corpus = [
            // --- must classify (the diffable statements) ---
            ("cyber_Folks 2026 Q1 SSF", "raport/SSF.pdf"),
            ("cyber_Folks 2026 Q1 JSF", "raport/JSF.pdf"),
            ("Skonsolidowane sprawozdanie finansowe GK za 2025", "x.pdf"),
            (
                "Jednostkowe sprawozdanie finansowe za I półrocze 2025",
                "x.pdf",
            ),
            ("Raport kwartalny QSr 1/2026", "raport_kwartalny.pdf"),
            ("Raport okresowy PSr 2025", "psr_2025.pdf"),
            ("Consolidated financial statement 2025", "x.pdf"),
            (
                "JSF_CBF Jednostkowe Sprawozdanie finansowe",
                "CBF-2025-12-31.xhtml",
            ),
            // no explicit SSF/JSF marker → defaults to consolidated
            (
                "Skonsolidowany raport okresowy grupy 2025",
                "raport_okresowy.pdf",
            ),
            // --- must be rejected (real filing noise bundled in ESPI/ESEF filings) ---
            ("26_03_17_sprawozdanie_RN_cyber_FolksSA_z_oceny", "x.pdf"),
            ("SzB_GK_cyber_Folks Sprawozdanie z badania", "x.xhtml"),
            ("Sprawozdanie z przeglądu śródrocznego skróconego", "x.pdf"),
            ("JSF_CBF-2025-12-31-1-pl.xhtml.xades", "x"),
            ("CBF-2025-12-31-1-pl.xbri", "x"),
            (
                "Podpisanie umowy o badanie sprawozdania finansowego",
                "x.xhtml",
            ),
            ("Opóźnienie publikacji raportu rocznego", "x.pdf"),
            ("Szacunkowa wysokość przychodów w 2025", "x.pdf"),
            ("Projekty uchwał ZWZ", "x.pdf"),
            ("Ogłoszenie o zwołaniu ZWZ", "x.pdf"),
            ("List Prezesa Zarządu do akcjonariuszy", "x.pdf"),
            ("Wybrane dane finansowe", "x.pdf"),
            ("Opinia i raport biegłego rewidenta", "x.pdf"),
        ];
        let rows: Vec<_> = corpus
            .iter()
            .map(|(title, url)| {
                (
                    *title,
                    classify_statement(title, url).map(StatementType::as_str),
                    period_label(title, url),
                )
            })
            .collect();
        insta::assert_debug_snapshot!(rows);
    }

    #[test]
    fn orders_periods_chronologically() {
        let q3_25 = period_sort_key("cyber_Folks 2025 Q3 SSF", "x").unwrap();
        let q1_26 = period_sort_key("cyber_Folks 2026 Q1 SSF", "x").unwrap();
        assert!(q3_25 < q1_26);
        assert_eq!(period_label("2026 Q1 SSF", "x").as_deref(), Some("2026 Q1"));
    }

    #[test]
    fn unparseable_period_is_none() {
        assert_eq!(period_sort_key("some report", "x.pdf"), None);
    }

    #[test]
    fn parses_1h_title_form_as_half_year() {
        // T-A1: the title form "za_1H_2023" lowercases to "1h", which no arm
        // matched before — it collapsed to index 0 (unknown) and lost the period.
        // It is the mirror of the already-handled "h1" and must map to H1.
        assert_eq!(period_sort_key("za_1H_2023", ""), Some((2023, 2)));
        assert_eq!(
            period_label("za_1H_2023", "").as_deref(),
            Some("2023 Q2/H1")
        );
    }

    #[test]
    fn parse_year_does_not_panic_on_multibyte_char_after_digit_run() {
        // "1abł " lowercased: an ASCII digit run (just "1") is followed within the
        // 4-byte scan window by the multi-byte 'ł', so `i + 4` lands on its second
        // byte — reproducing the char-boundary slicing panic (same class as
        // signal_dates::find_date_for_labels).
        assert_eq!(period_sort_key("1abł", ""), None);
    }
}

#[cfg(test)]
mod proptests {
    //! Invariant (property-based) coverage of the pure classification/period
    //! transforms (ADR 0049): `parse_year`'s byte-window digit scan panicked on
    //! multi-byte chars shortly after a digit (see
    //! `parse_year_does_not_panic_on_multibyte_char_after_digit_run`) — the same bug
    //! class as `signal_dates::find_date_for_labels`. Arbitrary title/url text,
    //! including unicode, must never panic.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn classify_statement_never_panics(title in ".*", url in ".*") {
            let _ = classify_statement(&title, &url);
        }

        #[test]
        fn period_sort_key_never_panics(title in ".*", url in ".*") {
            let _ = period_sort_key(&title, &url);
        }

        #[test]
        fn period_label_never_panics(title in ".*", url in ".*") {
            let _ = period_label(&title, &url);
        }
    }
}
