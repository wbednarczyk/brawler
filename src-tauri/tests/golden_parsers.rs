//! Golden snapshots of source-adapter parse outputs (ADR 0049, T2).
//!
//! Each source adapter turns a real-world sample (HTML/RSS/XML/JSON) into a
//! structured set of items. These tests lock the *whole parsed shape* with
//! `insta` debug snapshots, so a change to any adapter's output is a reviewable
//! diff instead of a silent drift — the cheapest way to cover the full structure
//! that hand-written field-by-field assertions under-cover. Snapshots are
//! committed and regenerated, never hand-edited (`cargo insta accept` or
//! `INSTA_UPDATE=always`).
//!
//! Determinism: every parser that takes a `fetched_at` is given a fixed
//! timestamp, so snapshots are stable across runs and machines.

use brawler_lib::source_adapters::{
    bankier_company::{self, BankierCompanyTarget},
    bankier_rss, gpw_company_registry, gpw_espi_ebi, newconnect_company_directory,
};

/// Fixed ingestion timestamp so parsed `fetched_at` fields are deterministic.
const FETCHED_AT: &str = "2026-06-08T10:00:00Z";

macro_rules! sample {
    ($name:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/", $name))
    };
}

#[test]
fn bankier_rss_listing_parse_is_stable() {
    let items = bankier_rss::parse_rss_items(sample!("bankier_gielda_rss.xml"), FETCHED_AT)
        .expect("bankier rss sample parses");
    insta::assert_debug_snapshot!("bankier_rss_items", items);
}

#[test]
fn gpw_company_registry_parse_is_stable() {
    let entries =
        gpw_company_registry::parse_company_registry_html(sample!("gpw_company_registry.html"))
            .expect("gpw registry sample parses");
    insta::assert_debug_snapshot!("gpw_company_registry_entries", entries);
}

#[test]
fn gpw_espi_ebi_listing_parse_is_stable() {
    let listings =
        gpw_espi_ebi::parse_report_listings(sample!("gpw_espi_ebi_listing.html"), FETCHED_AT)
            .expect("gpw espi/ebi listing sample parses");
    insta::assert_debug_snapshot!("gpw_espi_ebi_listings", listings);
}

#[test]
fn gpw_espi_ebi_detail_parse_is_stable() {
    let detail = gpw_espi_ebi::parse_report_detail(sample!("gpw_espi_ebi_detail.html"))
        .expect("gpw espi/ebi detail sample parses");
    insta::assert_debug_snapshot!("gpw_espi_ebi_detail", detail);
}

#[test]
fn gpw_espi_ebi_detail_without_attachments_parse_is_stable() {
    let detail =
        gpw_espi_ebi::parse_report_detail(sample!("gpw_espi_ebi_detail_no_attachments.html"))
            .expect("gpw espi/ebi no-attachments detail sample parses");
    insta::assert_debug_snapshot!("gpw_espi_ebi_detail_no_attachments", detail);
}

#[test]
fn newconnect_company_directory_parse_is_stable() {
    let entries = newconnect_company_directory::parse_company_directory_page_html(sample!(
        "newconnect_company_directory.html"
    ))
    .expect("newconnect directory sample parses");
    insta::assert_debug_snapshot!("newconnect_company_directory_entries", entries);
}

#[test]
fn bankier_company_identifiers_parse_is_stable() {
    let identifiers =
        bankier_company::parse_company_identifiers(sample!("bankier_company_cdr.html"))
            .expect("bankier company identifiers sample parses");
    insta::assert_debug_snapshot!("bankier_company_identifiers", identifiers);
}

#[test]
fn bankier_company_listing_parse_is_stable() {
    let target = BankierCompanyTarget {
        company_id: "company_gpw_cdr".to_owned(),
        ticker: "CDR".to_owned(),
        qualified_ticker: "GPW:CDR".to_owned(),
        bankier_slug: None,
        bankier_tag_id: None,
    };
    // `_all` skips the live recent-window filter, so the snapshot does not depend
    // on the relationship between FETCHED_AT and the sample's article dates.
    let items = bankier_company::parse_company_listing_json_all(
        &target,
        sample!("bankier_company_cdr_listing.json"),
        FETCHED_AT,
    )
    .expect("bankier company listing sample parses");
    insta::assert_debug_snapshot!("bankier_company_items", items);
}
