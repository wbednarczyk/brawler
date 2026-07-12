//! Periodic micro-benchmarks of the hot data-transform kernels (ADR 0049, T5).
//!
//! These cover the kernels that the data-heavy roadmap leans on: source-feed
//! parsing throughput and fundamentals formula parsing. They are **periodic
//! and informational** — run
//! with `make bench`; they NEVER fail `make check` (wall-clock is
//! machine-dependent). The companion `scripts/check/bench-ratchet.mjs` compares
//! the medians against the committed `bench-baseline.json` and flags relative
//! regressions, mirroring the coverage ratchet.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use brawler_lib::fundamentals::metrics::parse_formula;
use brawler_lib::source_adapters::bankier_rss;

const FETCHED_AT: &str = "2026-06-08T10:00:00Z";

/// RSS parsing throughput over a synthetic multi-item feed.
fn bench_rss_parse(c: &mut Criterion) {
    const ITEMS: usize = 200;
    let mut xml = String::from("<rss><channel>");
    for i in 0..ITEMS {
        xml.push_str(&format!(
            "<item><title>Report {i} ESPI</title><link>https://example.test/{i}?utm_source=x</link><description>Body {i}</description><pubDate>Tue, 02 Jun 2026 10:00:00 +0000</pubDate></item>"
        ));
    }
    xml.push_str("</channel></rss>");

    c.bench_function("bankier_rss_parse_200_items", |b| {
        b.iter(|| bankier_rss::parse_rss_items(black_box(&xml), FETCHED_AT))
    });
}

/// Fundamentals formula parsing (the metric DSL front end).
fn bench_formula_parse(c: &mut Criterion) {
    const FORMULA: &str = "(net_income - dividends) / (total_assets - current_liabilities) * 100";
    c.bench_function("parse_formula", |b| {
        b.iter(|| parse_formula(black_box(FORMULA)))
    });
}

criterion_group!(benches, bench_rss_parse, bench_formula_parse);
criterion_main!(benches);
