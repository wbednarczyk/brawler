//! Parser fuzzing on the stable toolchain (ADR 0049, T3).
//!
//! Real RSS/HTML/XML from many sources is hostile: truncated tags, stray
//! ampersands, unbalanced CDATA, junk attributes, unexpected Unicode. These
//! tests feed **adversarial markup** — assembled by a proptest structured
//! generator — into every source parser and assert the parser **never panics**
//! (proptest fails the test on any panic) and **never amplifies** its input into
//! an unbounded number of items.
//!
//! Why proptest and not `arbitrary`/`cargo-fuzz`: every Brawler parser consumes
//! `&str`. proptest string/recursive strategies generate adversarial *text*
//! directly and run in the normal stable test binary (deterministic, seeded,
//! shrinking). `arbitrary` (raw-bytes → structured input) and coverage-guided
//! `cargo-fuzz` earn their keep on byte-oriented parsers, which this codebase
//! does not have; adding either would mean a second nightly toolchain in the Nix
//! shell for no gain (full rationale in ADR 0049). The standard gate runs a
//! bounded 128 cases per test; set the `PROPTEST_CASES` env var higher for a
//! heavier on-demand run.

use brawler_lib::fundamentals::extraction::html;
use brawler_lib::source_adapters::{
    bankier_calendar, bankier_company, bankier_rss, biznesradar_fundamentals,
    biznesradar_ownership, biznesradar_recommendations, company_directory, gpw_company_registry,
    gpw_espi_ebi, gpw_market_events, knf_short_selling, newconnect_company_directory,
};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};

const FETCHED_AT: &str = "2026-06-08T10:00:00Z";

/// A generator of adversarial markup: random structural tokens (open/close tags,
/// entities, CDATA delimiters, raw angle brackets/ampersands) interleaved with
/// short junk text, concatenated into a document. Exercises the tokenizer and
/// element-walk paths far better than uniform random strings would.
fn adversarial_markup() -> impl Strategy<Value = String> {
    let token = prop_oneof![
        Just("<item>".to_string()),
        Just("</item>".to_string()),
        Just("<title>".to_string()),
        Just("</title>".to_string()),
        Just("<link>".to_string()),
        Just("<description>".to_string()),
        Just("<tr>".to_string()),
        Just("<td>".to_string()),
        Just("<a href=\"x\">".to_string()),
        Just("<![CDATA[".to_string()),
        Just("]]>".to_string()),
        Just("&amp;".to_string()),
        Just("&".to_string()),
        Just("<".to_string()),
        Just(">".to_string()),
        Just("ESPI".to_string()),
        Just("ISIN".to_string()),
        // Short junk text including Polish diacritics and markup metacharacters.
        "[a-zA-Z0-9 ąćęłńóśżźĄĆĘŁŃÓŚŻŹ<>&;/\"'=:.-]{0,12}",
    ];
    prop::collection::vec(token, 0..48).prop_map(|parts| parts.concat())
}

/// `PROPTEST_CASES`, parsed as `u32`; falls back to the standard-gate default
/// of 128 cases when unset or unparseable (a heavier on-demand run sets it
/// higher, per the module doc comment above).
fn bounded_cases(env_value: Option<&str>) -> u32 {
    env_value.and_then(|v| v.parse().ok()).unwrap_or(128)
}

#[test]
fn bounded_cases_falls_back_to_128() {
    assert_eq!(bounded_cases(None), 128);
    assert_eq!(bounded_cases(Some("2000")), 2000);
    assert_eq!(bounded_cases(Some("x")), 128);
}

// ---------------------------------------------------------------------------
// Structured generators (issue #194 S2): a GUARANTEED valid shape mixed with
// malformed entries, so each parser's real transformation is exercised, not
// only its rejection path. Kept next to `adversarial_markup` — same file,
// same idiom (bounded, deterministic strategies).
// ---------------------------------------------------------------------------

/// A holder/issuer display name the KNF register might carry: plain, or with
/// a raw (undecoded) `&amp;` entity — the exact shape `decode_entities` must
/// unescape.
fn knf_name() -> impl Strategy<Value = String> {
    prop_oneof![
        "[A-Z][a-z]{2,9}( [A-Z][a-z]{2,9}){0,2}",
        "[A-Z][a-z]{2,9} &amp; [A-Z][a-z]{2,9}",
    ]
}

/// A syntactically valid ISIN: two letters, nine alnum, one check digit.
fn knf_isin() -> impl Strategy<Value = String> {
    "[A-Z]{2}[A-Z0-9]{9}[0-9]"
}

/// One record the KNF register JSON might carry: a genuinely valid row (both
/// percent renderings the register uses, an entity-bearing name, a
/// timestamped date) or one of the two ways a real feed drops a row (no
/// ISIN, an unparsable percent).
#[derive(Debug, Clone)]
enum KnfRecordGen {
    Valid {
        holder: String,
        issuer: String,
        isin: String,
        pct_bp: u32,
        comma_decimal: bool,
        percent_suffix: bool,
        year: i64,
        month: u32,
        day: u32,
    },
    MissingIsin {
        holder: String,
        issuer: String,
    },
    UnparsablePercent {
        holder: String,
        issuer: String,
        isin: String,
    },
}

fn knf_record_gen() -> impl Strategy<Value = KnfRecordGen> {
    prop_oneof![
        2 => (
            knf_name(),
            knf_name(),
            knf_isin(),
            0u32..=9999,
            any::<bool>(),
            any::<bool>(),
            1990i64..=2099,
            1u32..=12,
            1u32..=28,
        )
            .prop_map(
                |(holder, issuer, isin, pct_bp, comma_decimal, percent_suffix, year, month, day)| {
                    KnfRecordGen::Valid {
                        holder,
                        issuer,
                        isin,
                        pct_bp,
                        comma_decimal,
                        percent_suffix,
                        year,
                        month,
                        day,
                    }
                }
            ),
        1 => (knf_name(), knf_name())
            .prop_map(|(holder, issuer)| KnfRecordGen::MissingIsin { holder, issuer }),
        1 => (knf_name(), knf_name(), knf_isin()).prop_map(|(holder, issuer, isin)| {
            KnfRecordGen::UnparsablePercent { holder, issuer, isin }
        }),
    ]
}

/// `(holder, issuer, isin, pct, YYYY-MM-DD)` — the normalized entry a
/// [`KnfRecordGen::Valid`] record must produce.
type KnfExpectedEntry = (String, String, String, f64, String);

/// Renders one generated record's JSON object, and — for a
/// [`KnfRecordGen::Valid`] record only — the normalized entry it must
/// produce.
fn knf_record_json(record: &KnfRecordGen) -> (serde_json::Value, Option<KnfExpectedEntry>) {
    match record {
        KnfRecordGen::Valid {
            holder,
            issuer,
            isin,
            pct_bp,
            comma_decimal,
            percent_suffix,
            year,
            month,
            day,
        } => {
            let mut rendered = format!("{:.2}", *pct_bp as f64 / 100.0);
            if *comma_decimal {
                rendered = rendered.replace('.', ",");
            }
            if *percent_suffix {
                rendered = format!("{rendered} %");
            }
            let date = format!("{year:04}-{month:02}-{day:02}T00:00:00");
            let expected = (
                holder.replace("&amp;", "&"),
                issuer.replace("&amp;", "&"),
                isin.clone(),
                *pct_bp as f64 / 100.0,
                format!("{year:04}-{month:02}-{day:02}"),
            );
            (
                serde_json::json!({
                    "HOLDER_FULL_NAME": holder,
                    "ISSUER_NAME": issuer,
                    "ISIN": isin,
                    "NET_SHORT_POSITION_O": rendered,
                    "POSITION_DATE": date,
                }),
                Some(expected),
            )
        }
        KnfRecordGen::MissingIsin { holder, issuer } => (
            serde_json::json!({
                "HOLDER_FULL_NAME": holder,
                "ISSUER_NAME": issuer,
                "NET_SHORT_POSITION_O": "0,50",
                "POSITION_DATE": "2026-01-01",
            }),
            None,
        ),
        KnfRecordGen::UnparsablePercent {
            holder,
            issuer,
            isin,
        } => (
            serde_json::json!({
                "HOLDER_FULL_NAME": holder,
                "ISSUER_NAME": issuer,
                "ISIN": isin,
                "NET_SHORT_POSITION_O": "n/a",
                "POSITION_DATE": "2026-01-01",
            }),
            None,
        ),
    }
}

/// A small, mutually-exclusive slice of the real Polish label dictionary
/// (`fundamentals::extraction::text_numbers::default_dictionary`, not
/// reachable from here — `pub(super)`): six EXACT-match labels sharing no
/// prefix, so a generated row's dictionary hit is unambiguous.
const DICTIONARY_LABELS: [(&str, &str); 6] = [
    ("Aktywa razem", "total_assets"),
    ("Zobowiązania razem", "total_liabilities"),
    ("Kapitał własny", "total_equity"),
    ("Zapasy", "inventories"),
    ("Przychody ze sprzedaży", "revenue"),
    ("Zysk netto", "net_profit"),
];

/// One generated aggregator-table statement row: a genuine dictionary hit
/// with a per-period amount, a row whose label the dictionary never maps
/// (skipped silently, never an error), or a dictionary label paired with an
/// unparsable amount (also skipped silently).
#[derive(Debug, Clone)]
enum FinRowGen {
    Valid { label_idx: usize, amounts: [i64; 4] },
    UnmappedLabel { amounts: [i64; 4] },
    GarbageAmount { label_idx: usize },
}

fn amounts4_strategy() -> impl Strategy<Value = [i64; 4]> {
    (
        0i64..=999_999,
        0i64..=999_999,
        0i64..=999_999,
        0i64..=999_999,
    )
        .prop_map(|(a, b, c, d)| [a, b, c, d])
}

fn fin_row_gen() -> impl Strategy<Value = FinRowGen> {
    prop_oneof![
        3 => (0usize..DICTIONARY_LABELS.len(), amounts4_strategy())
            .prop_map(|(label_idx, amounts)| FinRowGen::Valid { label_idx, amounts }),
        1 => amounts4_strategy().prop_map(|amounts| FinRowGen::UnmappedLabel { amounts }),
        1 => (0usize..DICTIONARY_LABELS.len())
            .prop_map(|label_idx| FinRowGen::GarbageAmount { label_idx }),
    ]
}

/// Renders `n` (0..=999_999) the way a Polish statement groups thousands
/// with a plain space — the separator `text_numbers::parse_amount` accepts.
fn format_amount_grouped(n: i64) -> String {
    let digits = n.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::new();
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(*byte as char);
    }
    out
}

/// One `<tr>` for a generated row, filling exactly `periods` value cells.
fn fin_row_html(row: &FinRowGen, periods: usize) -> String {
    match row {
        FinRowGen::Valid { label_idx, amounts } => {
            let (label, _metric) = DICTIONARY_LABELS[*label_idx];
            let cells: String = amounts[..periods]
                .iter()
                .map(|amount| format!("<td>{}</td>", format_amount_grouped(*amount)))
                .collect();
            format!("<tr><td>{label}</td>{cells}</tr>")
        }
        FinRowGen::UnmappedLabel { amounts } => {
            let cells: String = amounts[..periods]
                .iter()
                .map(|amount| format!("<td>{}</td>", format_amount_grouped(*amount)))
                .collect();
            format!("<tr><td>Sektor / inny wiersz</td>{cells}</tr>")
        }
        FinRowGen::GarbageAmount { label_idx } => {
            let (label, _metric) = DICTIONARY_LABELS[*label_idx];
            let cells: String = "<td>n/a</td>".repeat(periods);
            format!("<tr><td>{label}</td>{cells}</tr>")
        }
    }
}

/// The metric -> amount map each of `periods` columns must yield: only the
/// FIRST valid row for a given label counts — the parser dedups by
/// `metric_key`, first match wins (`html.rs`'s `facts_for_column`).
fn fin_expected_by_period(rows: &[FinRowGen], periods: usize) -> Vec<HashMap<&'static str, i64>> {
    let mut used = HashSet::new();
    let mut result = vec![HashMap::new(); periods];
    for row in rows {
        if let FinRowGen::Valid { label_idx, amounts } = row {
            if used.insert(*label_idx) {
                let metric = DICTIONARY_LABELS[*label_idx].1;
                for (p, slot) in result.iter_mut().enumerate() {
                    slot.insert(metric, amounts[p]);
                }
            }
        }
    }
    result
}

/// A holder name for the akcjonariat generator: never itself a percentage,
/// never empty.
fn ownership_holder_name() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,9}( [A-Z][a-z]{2,9}){0,2}"
}

/// One generated `qTableFull` row: a genuine holder (capital/votes percent,
/// a `dd.mm.yyyy` update date) or one of the four ways the real parser drops
/// a row: a `<th>` header/summary row, too few cells, an empty holder name,
/// or a percentage over 100 (column drift).
#[derive(Debug, Clone)]
enum OwnershipRowGen {
    Valid {
        holder: String,
        capital_bp: u32,
        votes_bp: u32,
        year: i64,
        month: u32,
        day: u32,
    },
    HeaderRow,
    TooFewCells,
    EmptyHolder,
    OverHundred,
}

fn ownership_row_gen() -> impl Strategy<Value = OwnershipRowGen> {
    prop_oneof![
        3 => (
            ownership_holder_name(),
            0u32..=10_000,
            0u32..=10_000,
            2000i64..=2099,
            1u32..=12,
            1u32..=28,
        )
            .prop_map(|(holder, capital_bp, votes_bp, year, month, day)| OwnershipRowGen::Valid {
                holder,
                capital_bp,
                votes_bp,
                year,
                month,
                day,
            }),
        1 => Just(OwnershipRowGen::HeaderRow),
        1 => Just(OwnershipRowGen::TooFewCells),
        1 => Just(OwnershipRowGen::EmptyHolder),
        1 => Just(OwnershipRowGen::OverHundred),
    ]
}

fn ownership_row_html(row: &OwnershipRowGen) -> String {
    match row {
        OwnershipRowGen::Valid {
            holder,
            capital_bp,
            votes_bp,
            year,
            month,
            day,
        } => {
            let capital = Decimal::new(*capital_bp as i64, 2);
            let votes = Decimal::new(*votes_bp as i64, 2);
            format!(
                "<tr><td>{holder}</td><td>{capital} %</td><td>1000</td><td>50000</td><td>{votes} %</td><td>1000000</td><td>{day:02}.{month:02}.{year:04}</td></tr>"
            )
        }
        OwnershipRowGen::HeaderRow => "<tr><th>Razem</th><td>93.22 %</td><td>1</td><td>2</td><td>93.22 %</td><td>3</td><td>01.01.2020</td></tr>".to_owned(),
        OwnershipRowGen::TooFewCells => "<tr><td>Ghost</td><td>1.00 %</td><td>2</td></tr>".to_owned(),
        OwnershipRowGen::EmptyHolder => "<tr><td></td><td>1.00 %</td><td>1</td><td>2</td><td>1.00 %</td><td>3</td><td>01.01.2020</td></tr>".to_owned(),
        OwnershipRowGen::OverHundred => "<tr><td>Drifted Holder</td><td>150.00 %</td><td>1</td><td>2</td><td>10.00 %</td><td>3</td><td>01.01.2020</td></tr>".to_owned(),
    }
}

/// One rating BiznesRadar's recommendation page vocabulary uses, verbatim.
fn recommendation_rating() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("kupuj"),
        Just("akumuluj"),
        Just("trzymaj"),
        Just("redukuj"),
        Just("sprzedaj"),
    ]
}

fn recommendation_price() -> impl Strategy<Value = String> {
    "[0-9]{2,3}\\.[0-9]{2}"
}

fn recommendation_name() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,8}( [A-Z][a-z]{2,8}){0,2}"
}

fn polish_month_abbrev(month: u32) -> &'static str {
    match month {
        1 => "sty",
        2 => "lut",
        3 => "mar",
        4 => "kwi",
        5 => "maj",
        6 => "cze",
        7 => "lip",
        8 => "sie",
        9 => "wrz",
        10 => "paź",
        11 => "lis",
        _ => "gru",
    }
}

/// One generated `table.recommendations` row: a fully populated valid row, a
/// valid row that omits every OPTIONAL field (target price, price-at-issue,
/// author parens, PDF anchor — all tolerated absences), or one of the three
/// ways the real parser treats a row as STRUCTURAL drift and fails the
/// WHOLE parse (too few columns, an empty rating, an unparsable date).
#[derive(Debug, Clone)]
enum RecommendationRowGen {
    FullValid {
        rating: &'static str,
        target_price: String,
        price_at_issue: String,
        day: u32,
        month: u32,
        year: i64,
        hour: u32,
        minute: u32,
        analyst: String,
        firm: String,
        relative_href: bool,
    },
    OptionalMissingValid {
        rating: &'static str,
        day: u32,
        month: u32,
        year: i64,
        hour: u32,
        minute: u32,
        firm: String,
    },
    TooFewColumns,
    EmptyRating,
    BadDate,
}

fn recommendation_row_gen() -> impl Strategy<Value = RecommendationRowGen> {
    prop_oneof![
        3 => (
            recommendation_rating(),
            recommendation_price(),
            recommendation_price(),
            1u32..=28,
            1u32..=12,
            2000i64..=2099,
            0u32..=23,
            0u32..=59,
            recommendation_name(),
            recommendation_name(),
            any::<bool>(),
        )
            .prop_map(
                |(rating, target_price, price_at_issue, day, month, year, hour, minute, analyst, firm, relative_href)| {
                    RecommendationRowGen::FullValid {
                        rating,
                        target_price,
                        price_at_issue,
                        day,
                        month,
                        year,
                        hour,
                        minute,
                        analyst,
                        firm,
                        relative_href,
                    }
                }
            ),
        2 => (
            recommendation_rating(),
            1u32..=28,
            1u32..=12,
            2000i64..=2099,
            0u32..=23,
            0u32..=59,
            recommendation_name(),
        )
            .prop_map(|(rating, day, month, year, hour, minute, firm)| {
                RecommendationRowGen::OptionalMissingValid {
                    rating,
                    day,
                    month,
                    year,
                    hour,
                    minute,
                    firm,
                }
            }),
        1 => Just(RecommendationRowGen::TooFewColumns),
        1 => Just(RecommendationRowGen::EmptyRating),
        1 => Just(RecommendationRowGen::BadDate),
    ]
}

fn recommendation_row_html(row: &RecommendationRowGen) -> String {
    match row {
        RecommendationRowGen::FullValid {
            rating,
            target_price,
            price_at_issue,
            day,
            month,
            year,
            hour,
            minute,
            analyst,
            firm,
            relative_href,
        } => {
            let date = format!(
                "{day:02} {} {year:04} {hour:02}:{minute:02}",
                polish_month_abbrev(*month)
            );
            let href = if *relative_href {
                "/storage/a/bc/report.pdf".to_owned()
            } else {
                "https://broker.example.com/report.pdf".to_owned()
            };
            format!(
                "<tr><td><span>{rating}</span></td><td>{target_price}</td><td>x</td><td>x</td><td>{price_at_issue}</td><td>{date}</td><td>{analyst} ({firm})</td><td><a href=\"{href}\">plik</a></td></tr>"
            )
        }
        RecommendationRowGen::OptionalMissingValid {
            rating,
            day,
            month,
            year,
            hour,
            minute,
            firm,
        } => {
            let date = format!(
                "{day:02} {} {year:04} {hour:02}:{minute:02}",
                polish_month_abbrev(*month)
            );
            format!(
                "<tr><td><span>{rating}</span></td><td></td><td>x</td><td>x</td><td></td><td>{date}</td><td>{firm}</td><td></td></tr>"
            )
        }
        RecommendationRowGen::TooFewColumns => {
            "<tr><td><span>kupuj</span></td><td>1</td><td>2</td></tr>".to_owned()
        }
        RecommendationRowGen::EmptyRating => {
            "<tr><td><span></span></td><td>1</td><td>x</td><td>x</td><td>2</td><td>18 cze 2026 08:40</td><td>Firm</td><td></td></tr>".to_owned()
        }
        RecommendationRowGen::BadDate => {
            "<tr><td><span>kupuj</span></td><td>1</td><td>x</td><td>x</td><td>2</td><td>not a date</td><td>Firm</td><td></td></tr>".to_owned()
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: bounded_cases(std::env::var("PROPTEST_CASES").ok().as_deref()),
        ..ProptestConfig::default()
    })]

    #[test]
    fn bankier_rss_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(items) = bankier_rss::parse_rss_items(&doc, FETCHED_AT) {
            prop_assert!(items.len() <= doc.len() + 1, "rss item amplification: {} from {}", items.len(), doc.len());
        }
    }

    #[test]
    fn gpw_market_events_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(events) = gpw_market_events::parse_market_events(&doc, FETCHED_AT) {
            prop_assert!(events.len() <= doc.len() + 1, "market-event amplification");
        }
    }

    #[test]
    fn gpw_espi_ebi_listing_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(listings) = gpw_espi_ebi::parse_report_listings(&doc, FETCHED_AT) {
            prop_assert!(listings.len() <= doc.len() + 1, "listing amplification");
        }
    }

    #[test]
    fn gpw_espi_ebi_detail_parser_is_total(doc in adversarial_markup()) {
        // A detail parse either succeeds or errors; it must never panic.
        let _ = gpw_espi_ebi::parse_report_detail(&doc);
    }

    #[test]
    fn gpw_company_registry_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(entries) = gpw_company_registry::parse_company_registry_html(&doc) {
            prop_assert!(entries.len() <= doc.len() + 1, "registry amplification");
        }
    }

    #[test]
    fn newconnect_directory_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(entries) = newconnect_company_directory::parse_company_directory_page_html(&doc) {
            prop_assert!(entries.len() <= doc.len() + 1, "newconnect amplification");
        }
    }

    #[test]
    fn company_directory_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(entries) = company_directory::parse_company_directory_html(&doc, "GPW", "https://example.test") {
            prop_assert!(entries.len() <= doc.len() + 1, "directory amplification");
        }
    }

    #[test]
    fn bankier_calendar_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(events) = bankier_calendar::parse_calendar_events(&doc, FETCHED_AT) {
            prop_assert!(events.len() <= doc.len() + 1, "calendar amplification");
        }
    }

    #[test]
    fn bankier_company_identifiers_parser_is_total(doc in adversarial_markup()) {
        // Identifiers parse either resolves slug+tag or errors; never panics.
        let _ = bankier_company::parse_company_identifiers(&doc);
    }

    #[test]
    fn bankier_company_listing_parser_is_total(doc in adversarial_markup()) {
        let target = bankier_company::BankierCompanyTarget {
            company_id: "company_gpw_cdr".to_owned(),
            ticker: "CDR".to_owned(),
            qualified_ticker: "GPW:CDR".to_owned(),
            bankier_slug: None,
            bankier_tag_id: None,
        };
        // Almost all inputs are invalid JSON (→ Err); the contract is no panic.
        let _ = bankier_company::parse_company_listing_json(&target, &doc, FETCHED_AT);
    }

    // -- knf_short_selling (issue #194 S2) -----------------------------------

    #[test]
    fn knf_short_selling_parser_is_total_on_markup(doc in adversarial_markup()) {
        // The register is JSON, not markup — almost every doc is Err; the
        // contract is no panic.
        let _ = knf_short_selling::parse_short_entries(&doc);
    }

    #[test]
    fn knf_short_selling_parser_is_total_on_arbitrary_text(doc in "\\PC{0,300}") {
        let _ = knf_short_selling::parse_short_entries(&doc);
    }

    /// Structured meaning: a mix of genuinely valid KNF records (entity-bearing
    /// names, both percent renderings, a timestamped date) and the two ways a
    /// real feed drops a row (no ISIN, an unparsable percent) — every valid
    /// record must survive with its normalized fields; every malformed one
    /// must simply vanish, never panic.
    #[test]
    fn knf_short_entries_parses_structured_records(records in prop::collection::vec(knf_record_gen(), 0..8)) {
        let mut json_records = Vec::new();
        let mut expected = Vec::new();
        for record in &records {
            let (json, exp) = knf_record_json(record);
            json_records.push(json);
            if let Some(exp) = exp {
                expected.push(exp);
            }
        }
        let payload = serde_json::json!({ "records": json_records }).to_string();
        let entries = knf_short_selling::parse_short_entries(&payload)
            .expect("a generated register payload must parse");
        prop_assert!(entries.len() <= records.len());
        for (holder, issuer, isin, pct, date) in &expected {
            let entry = entries.iter().find(|e| &e.isin == isin);
            prop_assert!(entry.is_some(), "valid record for isin {isin} missing");
            let entry = entry.unwrap();
            prop_assert_eq!(&entry.holder_name, holder);
            prop_assert_eq!(&entry.issuer_name, issuer);
            prop_assert!(
                (entry.net_position_pct - pct).abs() < 1e-9,
                "percent mismatch: {} vs {pct}", entry.net_position_pct
            );
            prop_assert_eq!(&entry.position_date, date);
        }
    }

    // -- biznesradar_fundamentals (issue #194 S2) ----------------------------

    /// Garbage totality: neither `has_report_table` nor `parse_witness_page`
    /// may panic or amplify. `has_report_table` requires a literal `<table`
    /// substring (HTML5 tag names are read verbatim); `adversarial_markup`
    /// never emits one, so absent that marker the page must never read as a
    /// report page.
    #[test]
    fn biznesradar_fundamentals_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if !doc.contains("<table") {
            prop_assert!(!biznesradar_fundamentals::has_report_table(&doc));
        }
        let facts = biznesradar_fundamentals::parse_witness_page(&doc, "2026-03-31", 2026, None);
        prop_assert!(facts.len() <= doc.len() + 1, "witness fact amplification");
    }

    /// Structured meaning: a generated single-period aggregator table, valid
    /// dictionary rows mixed with unmapped labels and unparsable amounts.
    #[test]
    fn biznesradar_fundamentals_structured_table_preserves_valid_rows(rows in prop::collection::vec(fin_row_gen(), 0..6)) {
        let mut page_html = String::from(
            "<html><body><p>Dane w tys. zł</p><table><tr><th>Pozycja</th><th>2026</th></tr>",
        );
        for row in &rows {
            page_html.push_str(&fin_row_html(row, 1));
        }
        page_html.push_str("</table></body></html>");

        if rows.is_empty() {
            prop_assert!(!biznesradar_fundamentals::has_report_table(&page_html));
        } else {
            prop_assert!(biznesradar_fundamentals::has_report_table(&page_html));
        }

        let facts = biznesradar_fundamentals::parse_witness_page(&page_html, "2026-12-31", 2026, None);
        prop_assert!(facts.len() <= rows.len());
        let expected = fin_expected_by_period(&rows, 1);
        for (metric, amount) in &expected[0] {
            let fact = facts.iter().find(|f| &f.metric_key == metric);
            prop_assert!(fact.is_some(), "metric {metric} missing");
            let expected_value = Decimal::from(*amount) * Decimal::from(1000i64);
            prop_assert_eq!(fact.unwrap().value, expected_value);
        }
    }

    // -- biznesradar_ownership (issue #194 S2) -------------------------------

    #[test]
    fn biznesradar_ownership_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(page) = biznesradar_ownership::parse_akcjonariat_page(&doc) {
            prop_assert!(page.holders.len() <= doc.len() + 1, "akcjonariat amplification");
        }
    }

    /// Structured meaning: a generated "Główni akcjonariusze" `qTableFull`
    /// page, valid holder rows mixed with the four ways a real row is
    /// dropped (a `<th>` row, too few cells, an empty holder, a >100%
    /// column-drift percentage).
    #[test]
    fn biznesradar_ownership_structured_table_preserves_valid_rows(rows in prop::collection::vec(ownership_row_gen(), 0..6)) {
        let mut page_html = String::from(
            "<html><body><h2>Główni akcjonariusze</h2><table class=\"qTableFull\"><tr><th>Akcjonariusz</th><th>Udział</th><th>Liczba akcji</th><th>Wartość rynkowa</th><th>Udział na WZA</th><th>Liczba głosów</th><th>Data aktualizacji</th></tr>",
        );
        for row in &rows {
            page_html.push_str(&ownership_row_html(row));
        }
        page_html.push_str("</table></body></html>");

        let page = biznesradar_ownership::parse_akcjonariat_page(&page_html)
            .expect("a generated page with a Główni akcjonariusze table must parse");
        prop_assert!(page.holders.len() <= rows.len());
        prop_assert!(!page.holders.iter().any(|h| h.holder_name.is_empty()));
        let hundred = Decimal::from(100);
        prop_assert!(page.holders.iter().all(|h| h.capital_pct.is_none_or(|v| v <= hundred)));
        prop_assert!(page.holders.iter().all(|h| h.votes_pct.is_none_or(|v| v <= hundred)));

        let mut expected_basis: Option<String> = None;
        let mut seen_names: HashSet<String> = HashSet::new();
        for row in &rows {
            if let OwnershipRowGen::Valid { holder, capital_bp, votes_bp, year, month, day } = row {
                // A rare regex-collision on the same generated name makes the
                // per-value check ambiguous (which row's percentages does the
                // stored holder reflect?) — skip it, the length/bound checks
                // above still cover it.
                if !seen_names.insert(holder.clone()) {
                    continue;
                }
                let capital = Decimal::new(*capital_bp as i64, 2);
                let votes = Decimal::new(*votes_bp as i64, 2);
                let found = page.holders.iter().find(|h| &h.holder_name == holder);
                prop_assert!(found.is_some(), "valid holder {holder} missing");
                let found = found.unwrap();
                prop_assert_eq!(found.capital_pct, Some(capital));
                prop_assert_eq!(found.votes_pct, Some(votes));
                let iso = format!("{year:04}-{month:02}-{day:02}");
                if expected_basis.as_deref().is_none_or(|current| iso.as_str() > current) {
                    expected_basis = Some(iso);
                }
            }
        }
        prop_assert_eq!(page.basis_as_of, expected_basis);
    }

    // -- biznesradar_recommendations (issue #194 S2) -------------------------

    #[test]
    fn biznesradar_recommendations_parser_is_total_and_bounded(doc in adversarial_markup()) {
        if let Ok(entries) = biznesradar_recommendations::parse_recommendations(&doc, "https://example.test/X") {
            prop_assert!(entries.len() <= doc.len() + 1, "recommendation amplification");
        }
    }

    /// Structured meaning: a generated `table.recommendations` page, fully
    /// valid rows and rows that omit only TOLERATED optional fields, mixed
    /// with rows that are STRUCTURAL drift (too few columns, empty rating,
    /// unparsable date) — the real parser fails the WHOLE parse on any of
    /// those, so their presence must flip the result to `Err`, never panic.
    #[test]
    fn biznesradar_recommendations_structured_rows_preserve_valid_entries(rows in prop::collection::vec(recommendation_row_gen(), 0..6)) {
        let mut page_html = String::from(
            "<html><body><table class=\"recommendations\"><tr><th>Rodzaj</th><th>Cena docelowa</th><th>Kurs aktualny</th><th>CD/K</th><th>Kurs z dnia wydania</th><th>Data upublicznienia</th><th>Autor</th><th>Plik</th></tr>",
        );
        for row in &rows {
            page_html.push_str(&recommendation_row_html(row));
        }
        page_html.push_str("</table></body></html>");

        let has_broken = rows.iter().any(|row| {
            matches!(
                row,
                RecommendationRowGen::TooFewColumns
                    | RecommendationRowGen::EmptyRating
                    | RecommendationRowGen::BadDate
            )
        });
        let result = biznesradar_recommendations::parse_recommendations(&page_html, "https://example.test/X");

        if has_broken {
            prop_assert!(result.is_err(), "a structurally broken row must fail the whole parse");
        } else {
            let entries = result.expect("well-formed rows must parse");
            prop_assert_eq!(entries.len(), rows.len());
            for (row, entry) in rows.iter().zip(entries.iter()) {
                match row {
                    RecommendationRowGen::FullValid {
                        rating,
                        target_price,
                        price_at_issue,
                        day,
                        month,
                        year,
                        hour,
                        minute,
                        analyst,
                        firm,
                        relative_href,
                    } => {
                        prop_assert_eq!(entry.rating.as_str(), *rating);
                        prop_assert_eq!(entry.target_price.as_deref(), Some(target_price.as_str()));
                        prop_assert_eq!(entry.price_at_issue.as_deref(), Some(price_at_issue.as_str()));
                        let expected_date = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00");
                        prop_assert_eq!(&entry.published_at, &expected_date);
                        prop_assert_eq!(entry.analyst.as_deref(), Some(analyst.as_str()));
                        prop_assert_eq!(&entry.firm, firm);
                        let expected_href = if *relative_href {
                            "https://www.biznesradar.pl/storage/a/bc/report.pdf".to_owned()
                        } else {
                            "https://broker.example.com/report.pdf".to_owned()
                        };
                        prop_assert_eq!(entry.report_url.as_deref(), Some(expected_href.as_str()));
                    }
                    RecommendationRowGen::OptionalMissingValid {
                        rating,
                        day,
                        month,
                        year,
                        hour,
                        minute,
                        firm,
                    } => {
                        prop_assert_eq!(entry.rating.as_str(), *rating);
                        prop_assert!(entry.target_price.is_none());
                        prop_assert!(entry.price_at_issue.is_none());
                        let expected_date = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00");
                        prop_assert_eq!(&entry.published_at, &expected_date);
                        prop_assert!(entry.analyst.is_none());
                        prop_assert_eq!(&entry.firm, firm);
                        prop_assert!(entry.report_url.is_none());
                    }
                    RecommendationRowGen::TooFewColumns
                    | RecommendationRowGen::EmptyRating
                    | RecommendationRowGen::BadDate => {
                        prop_assert!(false, "unreachable: filtered out by has_broken");
                    }
                }
            }
        }
    }

    // -- fundamentals::extraction::html (issue #194 S2) ----------------------

    /// Garbage totality: total facts across every returned period never
    /// exceed the input length.
    #[test]
    fn html_all_financials_parser_is_total_and_bounded(doc in adversarial_markup()) {
        let all = html::parse_all_financials(&doc);
        let total: usize = all.iter().map(|(_, facts)| facts.len()).sum();
        prop_assert!(total <= doc.len() + 1, "aggregator amplification");
    }

    /// Structured meaning: 1..4 generated annual period columns, 1..8 rows
    /// mixing valid dictionary hits with unmapped labels and unparsable
    /// amounts — every valid row's amount must surface, scaled, under its
    /// own period; total facts never exceed periods × rows.
    #[test]
    fn html_all_financials_reads_every_generated_period_and_row(
        periods_count in 1usize..=4,
        rows in prop::collection::vec(fin_row_gen(), 1..=8),
    ) {
        let years: Vec<i64> = (0..periods_count).map(|i| 2000 + i as i64).collect();
        let mut page_html = String::from("<html><body><p>Dane w tys. zł</p><table><tr><th>Pozycja</th>");
        for year in &years {
            page_html.push_str(&format!("<th>{year}</th>"));
        }
        page_html.push_str("</tr>");
        for row in &rows {
            page_html.push_str(&fin_row_html(row, periods_count));
        }
        page_html.push_str("</table></body></html>");

        let expected = fin_expected_by_period(&rows, periods_count);
        let all = html::parse_all_financials(&page_html);
        let total_facts: usize = all.iter().map(|(_, facts)| facts.len()).sum();
        prop_assert!(total_facts <= periods_count * rows.len());

        for (p, year) in years.iter().enumerate() {
            if expected[p].is_empty() {
                continue;
            }
            let facts = all.iter().find(|(period, _)| period.fiscal_year == *year).map(|(_, f)| f);
            prop_assert!(facts.is_some(), "expected period {year} missing from output");
            let facts = facts.unwrap();
            for (metric, amount) in &expected[p] {
                let fact = facts.iter().find(|f| &f.metric_key == metric);
                prop_assert!(fact.is_some(), "metric {metric} missing for year {year}");
                let expected_value = Decimal::from(*amount) * Decimal::from(1000i64);
                prop_assert_eq!(
                    fact.unwrap().value,
                    expected_value,
                    "value mismatch for {metric}@{year}",
                    metric = metric,
                    year = year
                );
            }
        }
    }
}
