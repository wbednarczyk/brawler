//! Parser goldens + proptest over SYNTHETIC section texts, one per ground-truth
//! class (never copied verbatim from `private/realdata/`). Names/vehicles are
//! invented; the *shapes* (cell-stream, positional multi-date, role-inline,
//! word-per-line, narrative-%, organ subheaders, prose-zero, TOC/remuneration
//! false anchors, subsidiary-table exclusion, glyph guard) mirror the real
//! variability the ground truth catalogued.

use super::*;
use crate::report_diff::extraction::{Section, SourceFormat};

/// One preamble section carrying `text` as its body (the flatten path pushes
/// every body line into the stream, exactly as a real extracted document does).
fn sections(text: &str) -> Vec<Section> {
    vec![Section {
        ordinal: 0,
        heading: "<preamble>".to_owned(),
        body: text.to_owned(),
    }]
}

fn parse(text: &str) -> MgmtHoldingsOutcome {
    parse_management_holdings(&sections(text), SourceFormat::Xhtml)
}

fn person<'a>(outcome: &'a MgmtHoldingsOutcome, name: &str) -> &'a MgmtHoldingRow {
    outcome
        .rows
        .iter()
        .find(|r| r.person_raw.contains(name))
        .unwrap_or_else(|| panic!("expected a row for {name}; got {:?}", outcome.rows))
}

// ---------------------------------------------------------------------------
// Clean single-organ table (TXT class): Imię i nazwisko / Stanowisko / Liczba akcji
// ---------------------------------------------------------------------------

#[test]
fn clean_single_org_table_recovers_person_role_and_shares() {
    let text = "\
Akcje w posiadaniu osób zarządzających i nadzorujących
Imię i nazwisko
Stanowisko
Liczba akcji
Tadeusz Wróblewski
Prezes Zarządu
3 366 250
Halina Marecka
Członek Zarządu
1 210 250
Bogdan Sitko
Przewodniczący Rady Nadzorczej
2 366 280";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::Parsed);
    assert_eq!(out.rows.len(), 3);
    let prezes = person(&out, "Tadeusz Wróblewski");
    assert_eq!(prezes.role, Some(MgmtRole::Management));
    assert_eq!(prezes.shares.as_deref(), Some("3366250"));
    assert_eq!(
        person(&out, "Bogdan Sitko").role,
        Some(MgmtRole::Supervisory)
    );
}

// ---------------------------------------------------------------------------
// Cell-stream before/after with a real zero row (PKO xhtml class)
// ---------------------------------------------------------------------------

#[test]
fn cell_stream_before_after_keeps_zero_and_takes_unambiguous_share() {
    // Columns: 2025 (liczba, wartość) / 2024 (liczba, wartość). Equal columns
    // → one distinct value; a person with all-zero cells is a REAL zero row.
    let text = "\
Stan posiadania akcji Banku przez członków Zarządu Banku
Imię i nazwisko
2025 rok
2024 rok
Robert Zawada
0
0
0
0
Piotr Malinowski
8 000
8 000
8 000
8 000";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::Parsed);
    assert_eq!(person(&out, "Robert Zawada").shares.as_deref(), Some("0"));
    assert_eq!(
        person(&out, "Piotr Malinowski").shares.as_deref(),
        Some("8000")
    );
    assert_eq!(
        person(&out, "Robert Zawada").role,
        Some(MgmtRole::Management)
    );
}

#[test]
fn ambiguous_multi_column_row_yields_null_shares_never_guesses() {
    // Publication / period-end / start columns disagree (a post-balance sale):
    // the as-of column cannot be mapped deterministically → shares = None.
    let text = "\
Akcje Spółki w posiadaniu członków Zarządu i Rady Nadzorczej
Marek Iwanow
Współprzewodniczący Rady Nadzorczej
12 650 000
12 873 520
12 873 520";
    let out = parse(text);
    let row = person(&out, "Marek Iwanow");
    assert_eq!(row.shares, None, "disagreeing columns must not be guessed");
    assert_eq!(row.role, Some(MgmtRole::Supervisory));
}

// ---------------------------------------------------------------------------
// Role-inline rows (PKO pdf class): "Name, Prezes Zarządu" then a numeric line
// ---------------------------------------------------------------------------

#[test]
fn role_inline_name_cell_recovers_role() {
    let text = "\
Stan posiadania akcji Banku przez członków Zarządu Banku
Zbigniew Kowal, Prezes Zarządu
0 0 0 0
Anna Lipka, Wiceprezes Zarządu
5 000 5 000 5 000 5 000";
    let out = parse(text);
    assert_eq!(out.rows.len(), 2);
    assert_eq!(
        person(&out, "Zbigniew Kowal").role,
        Some(MgmtRole::Management)
    );
    assert_eq!(person(&out, "Zbigniew Kowal").shares.as_deref(), Some("0"));
    assert_eq!(person(&out, "Anna Lipka").shares.as_deref(), Some("5000"));
}

// ---------------------------------------------------------------------------
// Word-per-line fragmentation (ACP class) — needs reflow
// ---------------------------------------------------------------------------

#[test]
fn word_per_line_reflow_recovers_person() {
    let text = "\
Akcje
w
posiadaniu
osób
zarządzających
i
nadzorujących
Jacek
Duszka
Przewodniczący
Rady
Nadzorczej
31
458";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::Parsed);
    let row = person(&out, "Jacek Duszka");
    assert_eq!(row.role, Some(MgmtRole::Supervisory));
    assert_eq!(row.shares.as_deref(), Some("31458"));
}

// ---------------------------------------------------------------------------
// Narrative-% with indirect footnotes (SNT class)
// ---------------------------------------------------------------------------

#[test]
fn narrative_pct_captures_indirect_vehicle() {
    let text = "\
Akcje Spółki w posiadaniu członków Zarządu i Rady Nadzorczej
Zarząd
Imię i nazwisko
Liczba akcji
Udział w kapitale %
Cezary Kozielski
2 047 380
24,00
* pośrednio poprzez Melhus Company Ltd
Dariusz Korecki
100 000
1,17";
    let out = parse(text);
    let kozielski = person(&out, "Cezary Kozielski");
    assert_eq!(kozielski.shares.as_deref(), Some("2047380"));
    assert_eq!(
        kozielski.indirect_via_raw.as_deref(),
        Some("Melhus Company Ltd")
    );
    assert_eq!(kozielski.role, Some(MgmtRole::Management));
    assert_eq!(person(&out, "Dariusz Korecki").indirect_via_raw, None);
}

// ---------------------------------------------------------------------------
// In-table organ subheaders (ABE class): "Zarząd"/"Rada Nadzorcza" group labels
// ---------------------------------------------------------------------------

#[test]
fn organ_subheaders_assign_role_to_following_persons() {
    let text = "\
Stan posiadania akcji emitenta przez osoby zarządzające i nadzorujące
Liczba akcji
% akcji
Zarząd
Andrzej Przybysz
0
0,00
Krzysztof Kucharek
25 000
0,15
Rada Nadzorcza
Jan Woźnicki
0
0,00";
    let out = parse(text);
    assert_eq!(
        person(&out, "Andrzej Przybysz").role,
        Some(MgmtRole::Management)
    );
    assert_eq!(
        person(&out, "Andrzej Przybysz").shares.as_deref(),
        Some("0")
    );
    assert_eq!(
        person(&out, "Krzysztof Kucharek").role,
        Some(MgmtRole::Management)
    );
    assert_eq!(
        person(&out, "Jan Woźnicki").role,
        Some(MgmtRole::Supervisory)
    );
}

#[test]
fn indirect_via_family_foundation_captured() {
    let text = "\
Zestawienie stanu posiadania akcji Spółki przez osoby zarządzające i nadzorujące
Liczba posiadanych akcji
Piotr Krupczak
Prezes Zarządu
1 716 965
* bezpośrednio oraz pośrednio przez Krupczak Fundacja Rodzinna";
    let out = parse(text);
    let row = person(&out, "Piotr Krupczak");
    assert_eq!(
        row.indirect_via_raw.as_deref(),
        Some("Krupczak Fundacja Rodzinna")
    );
}

// ---------------------------------------------------------------------------
// Prose-zero (XTB per-organ + GPW aggregate)
// ---------------------------------------------------------------------------

#[test]
fn prose_zero_per_organ_yields_zero_aggregate_per_organ() {
    let text = "\
Stan posiadania akcji Spółki przez Członków Rady Nadzorczej
Osoby nadzorujące nie posiadają akcji Spółki.
Stan posiadania akcji Spółki przez Członków Zarządu
Osoby zarządzające nie posiadają akcji Spółki.";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::ZeroHoldingAggregate);
    assert!(out.rows.is_empty());
    let roles: Vec<Option<MgmtRole>> = out.zero_organs.iter().map(|z| z.role).collect();
    assert!(roles.contains(&Some(MgmtRole::Supervisory)));
    assert!(roles.contains(&Some(MgmtRole::Management)));
}

#[test]
fn prose_zero_aggregate_no_names_yields_single_document_marker() {
    let text = "\
Struktura akcjonariatu i ład korporacyjny
Według najlepszej wiedzy Spółki, osoby zarządzające i nadzorujące Spółkę nie \
posiadają akcji, udziałów w jednostkach powiązanych, ani obligacji.";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::ZeroHoldingAggregate);
    assert_eq!(out.zero_organs.len(), 1);
    assert_eq!(out.zero_organs[0].role, None);
}

// ---------------------------------------------------------------------------
// False anchors + TOC lines + subsidiary tables
// ---------------------------------------------------------------------------

#[test]
fn table_of_contents_line_is_not_an_anchor() {
    // A TOC entry (trailing page number) must be skipped; the real section later
    // is the one that parses.
    let text = "\
9.3. Akcje w posiadaniu osób zarządzających i nadzorujących 68
Some unrelated narrative about strategy and governance.
9.3. Akcje w posiadaniu osób zarządzających i nadzorujących
Imię i nazwisko
Liczba akcji
Marian Cichy
Prezes Zarządu
1 000 000";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::Parsed);
    assert_eq!(out.rows.len(), 1);
    assert_eq!(
        person(&out, "Marian Cichy").shares.as_deref(),
        Some("1000000")
    );
}

#[test]
fn remuneration_section_is_not_a_holdings_anchor() {
    let text = "\
Wynagrodzenia osób zarządzających i nadzorujących
Imię i nazwisko
Wynagrodzenie
Andrzej Nowicki
1 200 000";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::SectionMissing);
    assert!(out.rows.is_empty());
}

#[test]
fn board_composition_and_diversity_are_not_holdings_anchors() {
    let text = "\
Skład osobowy i opis działania organów zarządzających i nadzorujących
Opis polityki różnorodności organów zarządzających i nadzorujących
Andrzej Nowicki jest Prezesem Zarządu od 2019 roku.";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::SectionMissing);
}

#[test]
fn subsidiary_share_table_is_excluded() {
    // The issuer's own table, then a second table of holdings in a RELATED entity
    // — only the own-company rows count.
    let text = "\
Akcje w posiadaniu osób zarządzających i nadzorujących
Imię i nazwisko
Liczba akcji
Jacek Duszka
Przewodniczący Rady Nadzorczej
31 458
Akcje jednostek powiązanych (Grupa Wschód) w posiadaniu osób zarządzających
Adam Nogaj
150
Marek Panecki
300";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::Parsed);
    assert!(
        out.rows
            .iter()
            .all(|r| !r.person_raw.contains("Nogaj") && !r.person_raw.contains("Panecki")),
        "subsidiary-entity holders leaked into rows: {:?}",
        out.rows
    );
    assert_eq!(
        out.rows
            .iter()
            .filter(|r| r.person_raw.contains("Duszka"))
            .count(),
        1
    );
}

#[test]
fn glyph_encoded_document_parks_and_emits_nothing() {
    // A custom-font PUA line (ratio well over the 0.02 threshold) → GlyphEncoded.
    let mut text = String::from(
        "Akcje Spółki w posiadaniu członków Zarządu i Rady Nadzorczej\nImię i nazwisko\n",
    );
    for _ in 0..600 {
        text.push('\u{E000}');
    }
    let out = parse(&text);
    assert_eq!(out.state, MgmtHoldingsState::GlyphEncoded);
    assert!(out.rows.is_empty());
    assert!(out.matched_heading.is_some());
}

#[test]
fn no_holdings_anchor_yields_section_missing() {
    let out = parse("An annual report about strategy, markets, and risk factors.");
    assert_eq!(out.state, MgmtHoldingsState::SectionMissing);
    assert!(out.matched_heading.is_none());
}

// ---------------------------------------------------------------------------
// Person-plausibility gate (F-A2): junk "name" rows the real ABE corpus produced
// must be dropped; genuine holders (incl. a title-prefixed one) must survive.
// ---------------------------------------------------------------------------

#[test]
fn plausibility_gate_drops_entity_address_and_role_junk_rows() {
    // A holdings table interleaved with the exact junk SHAPES the live ABE corpus
    // emitted (entity, legal form, street address, state body, office, brand
    // string, a year mis-read as a share count, a genitive prose fragment, and a
    // courtesy-title-prefixed real name). Names are synthetic; shapes are real.
    let text = "\
Akcje w posiadaniu osób zarządzających i nadzorujących
Imię i nazwisko
Liczba akcji
Jan Kowalczyk
1 000
Ul Europejska
55
Zleceniodawca Wystawca
100
Skarb Państwa
200
Dyrektor Izby
300
Sales Limited
400
Alsen Marketing Samsung
500
Apple Realme Oppo
2024
Rekman Sp
12 000 000
Pan Wojciech Nowacki
700
Michała Wnorowskiego Tomasza
800
Standardami Sprawozdawczości Finansowej
900
Anna Zielińska
2 000";
    let out = parse(text);
    assert_eq!(out.state, MgmtHoldingsState::Parsed);

    let names: Vec<&str> = out.rows.iter().map(|r| r.person_raw.as_str()).collect();
    // Genuine holders survive.
    assert!(names.contains(&"Jan Kowalczyk"), "got {names:?}");
    assert!(names.contains(&"Anna Zielińska"), "got {names:?}");
    // The courtesy title is stripped — stored identity is the bare name.
    assert!(
        names.contains(&"Wojciech Nowacki"),
        "PAN prefix must be stripped, got {names:?}"
    );
    assert!(!names.iter().any(|n| n.contains("Pan")));

    // None of the junk shapes leak into the founder-stamping substrate.
    for junk in [
        "Europejska",
        "Wystawca",
        "Skarb",
        "Państwa",
        "Izby",
        "Limited",
        "Marketing",
        "Samsung",
        "Apple",
        "Oppo",
        "Rekman",
        "Wnorowskiego",
        "Standardami",
        "Sprawozdawczości",
        "Finansowej",
    ] {
        assert!(
            !names.iter().any(|n| n.contains(junk)),
            "junk token {junk:?} leaked into rows: {names:?}"
        );
    }
    // Exactly the three real holders remain.
    assert_eq!(out.rows.len(), 3, "unexpected rows: {names:?}");
}

// ---------------------------------------------------------------------------
// Goldens
// ---------------------------------------------------------------------------

#[test]
fn golden_organ_subheaders_shape() {
    let text = "\
Stan posiadania akcji emitenta przez osoby zarządzające i nadzorujące
Liczba akcji
% akcji
Zarząd
Andrzej Przybysz
0
0,00
Krzysztof Kucharek
25 000
0,15
Rada Nadzorcza
Jan Woźnicki
0
0,00";
    insta::assert_debug_snapshot!(parse(text));
}

#[test]
fn golden_narrative_indirect_shape() {
    let text = "\
Akcje Spółki w posiadaniu członków Zarządu i Rady Nadzorczej
Zarząd
Liczba akcji
Cezary Kozielski
2 047 380
* pośrednio poprzez Melhus Company Ltd";
    insta::assert_debug_snapshot!(parse(text));
}

// ---------------------------------------------------------------------------
// proptest: totality, determinism, idempotent re-parse
// ---------------------------------------------------------------------------

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn never_panics_on_arbitrary_input(text in ".{0,4000}") {
        let _ = parse(&text);
    }

    #[test]
    fn deterministic_on_arbitrary_input(text in ".{0,2000}") {
        let a = parse(&text);
        let b = parse(&text);
        prop_assert_eq!(a.state, b.state);
        prop_assert_eq!(a.rows.len(), b.rows.len());
    }

    #[test]
    fn person_share_never_fabricated_from_pure_prose(name in "[A-Z][a-z]{2,8} [A-Z][a-z]{2,8}") {
        // A person name embedded in prose with no numeric table must not produce
        // a fabricated non-zero share.
        let text = format!("W raporcie wspomniano, że {name} pełni funkcję prezesa.");
        let out = parse(&text);
        for row in &out.rows {
            prop_assert!(row.shares.as_deref() != Some("999999"));
        }
    }
}

#[test]
fn implausible_person_gate_rejects_known_live_junk() {
    // Live-corpus leaks the junk harness caught (ABE + the PZU heading fragment).
    for junk in [
        "STANOWISKA DYREKTORÓW",
        "UL EUROPEJSKA",
        "ZLECENIODAWCA WYSTAWCA",
        "ALSEN MARKETING SAMSUNG",
        "DYREKTOR IZBY",
    ] {
        assert!(
            is_implausible_person(junk, None),
            "gate must reject junk row: {junk}"
        );
    }
    for legit in ["ANDRZEJ PRZYBYŁO", "IWONA PRZYBYŁO", "JAN WOŹNIAK"] {
        assert!(
            !is_implausible_person(legit, Some("100")),
            "gate must keep legitimate person: {legit}"
        );
    }
}
