//! Sample-based unit tests for the ownership shareholders-table parser
//! (v0.56 T1). Uses small hand-built "test sample" documents (not real
//! filings); real-data accuracy is measured separately by the `#[ignore]`
//! `storage::tests::real_data_ownership` harness.

use proptest::prelude::*;

use super::*;
use crate::report_diff::extraction::{extract_report, Section, SourceFormat};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A single text section (heading + body) — the PDF-path input shape.
fn section(heading: &str, body: &str) -> Section {
    Section {
        ordinal: 0,
        heading: heading.to_owned(),
        body: body.to_owned(),
    }
}

/// A realistic ESEF-style shareholders table: 5 columns (name, share count,
/// % capital, vote count, % votes) with capital ≠ votes, thousands separators,
/// and a "pozostali (free float)" aggregate row. Padded past the extractor's
/// `MIN_XHTML_CHARS` so it reaches the `Extracted` state.
fn sample_xhtml() -> String {
    let filler = "Niniejszy raport okresowy zawiera dane porownawcze oraz komentarz. ".repeat(70);
    format!(
        "<html><body>\
         <p>{filler}</p>\
         <h2>Akcjonariusze posiadajacy co najmniej 5% ogolnej liczby glosow na WZ</h2>\
         <table>\
         <tr><th>Akcjonariusz</th><th>Liczba akcji</th><th>% kapitalu</th>\
         <th>Liczba glosow</th><th>% glosow</th></tr>\
         <tr><td>Jan Kowalski</td><td>1 234 567</td><td>12,34</td>\
         <td>2 000 000</td><td>15,00</td></tr>\
         <tr><td>Aviva OFE</td><td>987 654</td><td>9,88</td>\
         <td>987 654</td><td>7,41</td></tr>\
         <tr><td>Pozostali (free float)</td><td>7 777 779</td><td>77,78</td>\
         <td>10 000 000</td><td>77,59</td></tr>\
         </table></body></html>"
    )
}

fn parse_sample_xhtml() -> OwnershipParseOutcome {
    let outcome = extract_report(sample_xhtml().as_bytes(), SourceFormat::Xhtml);
    parse_shareholders(&outcome.sections, SourceFormat::Xhtml)
}

// ---------------------------------------------------------------------------
// xhtml table parsing
// ---------------------------------------------------------------------------

#[test]
fn xhtml_table_parses_exact_rows() {
    let out = parse_sample_xhtml();
    assert_eq!(out.state, OwnershipParseState::Found);
    assert_eq!(out.rows.len(), 2, "two holder rows, aggregate excluded");

    assert_eq!(out.rows[0].holder_raw, "Jan Kowalski");
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("12.34"));
    assert_eq!(out.rows[0].votes_pct.as_deref(), Some("15.00"));

    assert_eq!(out.rows[1].holder_raw, "Aviva OFE");
    assert_eq!(out.rows[1].capital_pct.as_deref(), Some("9.88"));
    assert_eq!(out.rows[1].votes_pct.as_deref(), Some("7.41"));
}

#[test]
fn xhtml_capital_and_votes_are_kept_separate() {
    // The gap between % capital and % votes is the signal (ADR 0072 tripwire):
    // the parser must never conflate them.
    let out = parse_sample_xhtml();
    for row in &out.rows {
        assert_ne!(row.capital_pct, row.votes_pct);
    }
}

#[test]
fn xhtml_aggregate_row_is_excluded_and_recorded() {
    let out = parse_sample_xhtml();
    assert!(
        out.rows.iter().all(|r| !r.holder_raw.contains("Pozostali")),
        "aggregate must not appear as a holder row"
    );
    assert_eq!(out.aggregates.len(), 1);
    assert!(out.aggregates[0].label_raw.contains("Pozostali"));
    assert_eq!(out.aggregates[0].capital_pct.as_deref(), Some("77.78"));
    assert_eq!(out.aggregates[0].votes_pct.as_deref(), Some("77.59"));
}

// ---------------------------------------------------------------------------
// PDF text-layout parsing
// ---------------------------------------------------------------------------

#[test]
fn pdf_text_parses_exact_rows() {
    let body = "\
Akcjonariusz                    % kapitalu        % glosow
Jan Kowalski                    12,34%            15,00%
Aviva OFE                       9,88%             7,41%
Pozostali                       77,78%            77,59%
";
    let sections = vec![section(
        "znaczne pakiety akcji",
        &format!("Struktura akcjonariatu\n{body}"),
    )];
    let out = parse_shareholders(&sections, SourceFormat::Pdf);

    assert_eq!(out.state, OwnershipParseState::Found);
    assert_eq!(out.rows.len(), 2);
    assert_eq!(out.rows[0].holder_raw, "Jan Kowalski");
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("12.34"));
    assert_eq!(out.rows[0].votes_pct.as_deref(), Some("15.00"));
    assert_eq!(out.rows[1].holder_raw, "Aviva OFE");
    assert_eq!(out.aggregates.len(), 1);
    assert!(out.aggregates[0].label_raw.contains("Pozostali"));
}

#[test]
fn pdf_normalizes_comma_percent_and_spaces() {
    // comma→dot, trailing '%', and an internal space in the cell are all stripped.
    let sections = vec![section(
        "<preamble>",
        "Akcjonariat\nSkarb Panstwa            51,00 %            60,00%\n",
    )];
    let out = parse_shareholders(&sections, SourceFormat::Pdf);
    assert_eq!(out.rows.len(), 1);
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("51.00"));
    assert_eq!(out.rows[0].votes_pct.as_deref(), Some("60.00"));
}

#[test]
fn one_sided_disclosure_leaves_the_other_none() {
    // Only % capital is disclosed (single column) → votes stays None.
    let sections = vec![section(
        "<preamble>",
        "Struktura akcjonariatu\n\
         Akcjonariusz            % kapitalu\n\
         Skarb Panstwa           51,00\n",
    )];
    let out = parse_shareholders(&sections, SourceFormat::Pdf);
    assert_eq!(out.rows.len(), 1);
    assert_eq!(out.rows[0].holder_raw, "Skarb Panstwa");
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("51.00"));
    assert_eq!(out.rows[0].votes_pct, None);
}

// ---------------------------------------------------------------------------
// Heading-variant matching
// ---------------------------------------------------------------------------

#[test]
fn every_heading_variant_locates_the_section() {
    let variants = [
        "Akcjonariusze",
        "Akcjonariat",
        "Struktura akcjonariatu",
        "Znaczne pakiety akcji",
        "Akcjonariusze posiadajacy co najmniej 5%",
        "Wykaz akcjonariuszy",
    ];
    for heading in variants {
        let body = format!("{heading}\nJan Kowalski            12,34            15,00\n");
        let out = parse_shareholders(&[section("<preamble>", &body)], SourceFormat::Pdf);
        assert_eq!(
            out.state,
            OwnershipParseState::Found,
            "variant did not locate a section: {heading}"
        );
        assert_eq!(out.rows.len(), 1, "variant {heading}");
        assert_eq!(out.matched_heading.as_deref(), Some(heading));
    }
}

#[test]
fn a_letter_to_shareholders_is_not_a_holdings_section() {
    // "List do akcjonariuszy" is narrative, not the holdings table — the anchored
    // heading regex must not fire on it (no numbers to parse → SectionMissing).
    let out = parse_shareholders(
        &[section(
            "<preamble>",
            "List do akcjonariuszy\nSzanowni Panstwo,\n",
        )],
        SourceFormat::Pdf,
    );
    assert_eq!(out.state, OwnershipParseState::SectionMissing);
}

// ---------------------------------------------------------------------------
// Determinism, no-panic, golden
// ---------------------------------------------------------------------------

#[test]
fn parse_is_deterministic() {
    let a = parse_sample_xhtml();
    let b = parse_sample_xhtml();
    assert_eq!(a, b);
}

proptest! {
    #[test]
    fn parse_never_panics(body in ".*", heading in ".*", xhtml in any::<bool>()) {
        let sections = vec![section(&heading, &body)];
        let format = if xhtml { SourceFormat::Xhtml } else { SourceFormat::Pdf };
        // Must not panic and must return; result is intentionally unused.
        let _ = parse_shareholders(&sections, format);
    }
}

#[test]
fn golden_xhtml_parse_output() {
    insta::assert_debug_snapshot!("ownership_xhtml_golden", parse_sample_xhtml());
}

// ---------------------------------------------------------------------------
// Real-corpus regression shapes (reduced test samples, not real filings)
// ---------------------------------------------------------------------------

/// Narrative bullet disclosure (PZU shape): full holder names in bullets,
/// values inside running sentences, share counts split across lines. The
/// value line starting with a share-count-sized digit run must not be
/// mistaken for an "akcji…" column header.
#[test]
fn narrative_bullets_parse_full_names_and_percentages() {
    let body = "\
Na 30 czerwca 2023 roku akcjonariuszami PZU posiadajacymi
znaczne pakiety akcji (co najmniej 5%) byli:

\u{2022}  Skarb Panstwa Rzeczypospolitej Polskiej, ktory posiada
295 217 300 akcji, co stanowi 34,19% kapitalu zakladowego
PZU i uprawnia do 295 217 300 glosow na Walnym
Zgromadzeniu;

\u{2022}  Allianz Polska Otwarty Fundusz Emerytalny oraz Allianz
Polska Dobrowolny Fundusz Emerytalny, ktore na
Zwyczajnym Walnym Zgromadzeniu, ktore odbylo sie
7 czerwca 2023 roku, posiadaly 45 742 250 akcji, co
stanowilo 5,30% kapitalu zakladowego PZU i uprawnialo do
45 742 250 glosow na Walnym Zgromadzeniu;

\u{2022}  Nationale-Nederlanden Otwarty Fundusz Emerytalny,
ktory na Zwyczajnym Walnym Zgromadzeniu PZU, ktore
odbylo sie 7 czerwca 2023 roku, posiadal 43 680 074 akcji, co
stanowilo 5,06% kapitalu zakladowego PZU i uprawnialo do
43 680 074 glosow na Walnym Zgromadzeniu.
";
    let sections = vec![section("<preamble>", body)];
    let out = parse_shareholders(&sections, SourceFormat::Pdf);

    assert_eq!(out.state, OwnershipParseState::Found);
    assert_eq!(out.rows.len(), 3, "three narrative bullets → three rows");
    assert_eq!(
        out.rows[0].holder_raw,
        "Skarb Panstwa Rzeczypospolitej Polskiej"
    );
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("34.19"));
    assert_eq!(out.rows[0].votes_pct.as_deref(), Some("34.19"));
    assert_eq!(
        out.rows[1].holder_raw,
        "Allianz Polska Otwarty Fundusz Emerytalny oraz Allianz Polska Dobrowolny Fundusz Emerytalny"
    );
    assert_eq!(out.rows[1].capital_pct.as_deref(), Some("5.30"));
    assert_eq!(
        out.rows[2].holder_raw,
        "Nationale-Nederlanden Otwarty Fundusz Emerytalny"
    );
    assert_eq!(out.rows[2].capital_pct.as_deref(), Some("5.06"));
}

/// Cell-stream table with the free-float aggregate *above* a treasury-shares
/// holder row (ACP shape): "Pozostali akcjonariusze" must stay a data row
/// (not a nested anchor), and the dash-prefixed "- akcje wlasne" qualifier
/// cell must not be eaten as a column header.
#[test]
fn aggregate_above_treasury_row_keeps_both() {
    let body = "\
Akcjonariat na dzien 31 grudnia 2025 roku
Liczba akcji w posiadaniu
/ liczba glosow z nich wynikajaca
Udzial w kapitale zakladowym
/ ogolnej liczbie glosow
TSS Europe B.V.
(1)*
19 207 886
23,14%
Adam Goral Fundacja Rodzinna
(2)*
9 015 503
10,86%
Allianz OFE
(3)
8 300 027
9,99%
Nationale-Nederlanden OFE
(4)
4 171 121
5,03%
Pozostali akcjonariusze
39 815 757
47,98%
Asseco Poland
(5)
- akcje wlasne**
2 490 009
3,00%
Razem
83 000 303
100,00%
";
    let sections = vec![section("Struktura akcjonariatu", body)];
    let out = parse_shareholders(&sections, SourceFormat::Xhtml);

    assert_eq!(out.state, OwnershipParseState::Found);
    let holders: Vec<&str> = out.rows.iter().map(|r| r.holder_raw.as_str()).collect();
    assert_eq!(
        holders,
        [
            "TSS Europe B.V.",
            "Adam Goral Fundacja Rodzinna",
            "Allianz OFE",
            "Nationale-Nederlanden OFE",
            "Asseco Poland",
        ]
    );
    let asseco = out.rows.last().expect("asseco row");
    assert_eq!(asseco.capital_pct.as_deref(), Some("3.00"));
    assert_eq!(asseco.votes_pct.as_deref(), Some("3.00"));
    // NN OFE must keep its own 5,03 — the free-float 47,98 below it belongs
    // to the aggregate row.
    assert_eq!(out.rows[3].capital_pct.as_deref(), Some("5.03"));
    assert_eq!(out.aggregates.len(), 2, "Pozostali + Razem");
    assert_eq!(out.aggregates[0].capital_pct.as_deref(), Some("47.98"));
}

/// A "Źródło: …" caption before any row marks a pie-chart block (PZU shape):
/// the chart's stray labels must not assemble into rows; the real table
/// further on must win instead.
#[test]
fn chart_caption_before_rows_poisons_the_block() {
    let body = "\
Struktura akcjonariatu na 17.05.2023 roku
Zrodlo: Raporty biezace 10/2023
Skarb Panstwa
34,2%
Pozostali
65,8%
Wykaz akcjonariuszy
Akcjonariusz            % kapitalu        % glosow
Jan Kowalski            12,34%            15,00%
";
    let out = parse_shareholders(&[section("<preamble>", body)], SourceFormat::Pdf);
    assert_eq!(out.state, OwnershipParseState::Found);
    assert_eq!(
        out.rows.len(),
        1,
        "only the real table row, no chart labels"
    );
    assert_eq!(out.rows[0].holder_raw, "Jan Kowalski");
    assert_eq!(out.matched_heading.as_deref(), Some("Wykaz akcjonariuszy"));
}

/// A block whose header packs three or more as-of dates is a multi-period
/// comparison table (ambiguous column mapping) — a single-period block must
/// outscore it.
#[test]
fn multi_period_comparison_table_loses_to_single_period_block() {
    let body = "\
Akcjonariusze posiadajacy znaczne pakiety akcji 31.12.2021 31.12.2022 30.06.2023
Jan Kowalski 100 000 200 000 300 000 10,00% 11,00% 12,34%
Aviva OFE 50 000 60 000 70 000 5,00% 6,00% 7,41%
Struktura akcjonariatu
Akcjonariusz            % kapitalu        % glosow
Jan Kowalski            12,34%            15,00%
Aviva OFE               7,41%             8,00%
";
    let out = parse_shareholders(&[section("<preamble>", body)], SourceFormat::Pdf);
    assert_eq!(out.state, OwnershipParseState::Found);
    assert_eq!(
        out.matched_heading.as_deref(),
        Some("Struktura akcjonariatu")
    );
    assert_eq!(out.rows.len(), 2);
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("12.34"));
    assert_eq!(out.rows[0].votes_pct.as_deref(), Some("15.00"));
}

/// Running prose overflows the pending-name accumulator (KGH narrative
/// change-of-holdings paragraphs): the percentages inside the sentence must
/// not form a holder row built from a mid-sentence fragment.
#[test]
fn prose_overflow_never_forms_a_row() {
    let prose_a = "Spolka zostala poinformowana o polaczeniu spolek Powszechne \
Towarzystwo Emerytalne Allianz Polska Spolka Akcyjna oraz Aviva Powszechne \
Towarzystwo Emerytalne Aviva Santander Spolka Akcyjna";
    let prose_b = "W wyniku polaczenia stan na rachunkach funduszy Otwarty \
Fundusz Emerytalny oraz Dobrowolny Fundusz Emerytalny";
    let body = format!(
        "Struktura akcjonariatu\n{prose_a}\n{prose_b}\n\
         wyniosl 12 241 453 akcje stanowiace 6,12% kapitalu\n\
         Akcjonariusz            % kapitalu\n\
         Skarb Panstwa           31,79%\n"
    );
    let out = parse_shareholders(&[section("<preamble>", &body)], SourceFormat::Pdf);
    assert_eq!(out.state, OwnershipParseState::Found);
    assert_eq!(out.rows.len(), 1, "prose fragment must not become a holder");
    assert_eq!(out.rows[0].holder_raw, "Skarb Panstwa");
    assert_eq!(out.rows[0].capital_pct.as_deref(), Some("31.79"));
}
