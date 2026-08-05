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

use brawler_lib::source_adapters::{
    bankier_calendar, bankier_company, bankier_rss, company_directory, gpw_company_registry,
    gpw_espi_ebi, gpw_market_events, newconnect_company_directory,
};
use proptest::prelude::*;

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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

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
}
