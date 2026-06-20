//! Periodic micro-benchmarks of the hot data-transform kernels (ADR 0049, T5).
//!
//! These cover the kernels that the data-heavy roadmap leans on: the similarity
//! scan (the find_similar inner loop), source-feed parsing throughput, and
//! fundamentals formula parsing. They are **periodic and informational** — run
//! with `make bench`; they NEVER fail `make check` (wall-clock is
//! machine-dependent). The companion `scripts/check/bench-ratchet.mjs` compares
//! the medians against the committed `bench-baseline.json` and flags relative
//! regressions, mirroring the coverage ratchet.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use brawler_lib::fundamentals::metrics::parse_formula;
use brawler_lib::interpretation::similarity_score;
use brawler_lib::source_adapters::bankier_rss;

const FETCHED_AT: &str = "2026-06-08T10:00:00Z";

/// Deterministic unit-ish vector of `dim`, varied by `seed`.
fn seeded_vector(seed: usize, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|i| (((seed * 31 + i * 7) % 97) as f32) / 97.0)
        .collect()
}

/// The find_similar inner loop: score one query against `N` stored vectors at a
/// realistic embedding dimension.
fn bench_similarity_scan(c: &mut Criterion) {
    const N: usize = 1000;
    const DIM: usize = 384;
    let query = seeded_vector(0, DIM);
    let candidates: Vec<Vec<f32>> = (1..=N).map(|s| seeded_vector(s, DIM)).collect();

    c.bench_function("similarity_scan_1000x384", |b| {
        b.iter(|| {
            let mut best = f32::MIN;
            for candidate in &candidates {
                best = best.max(similarity_score(black_box(&query), black_box(candidate)));
            }
            best
        })
    });
}

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

criterion_group!(
    benches,
    bench_similarity_scan,
    bench_rss_parse,
    bench_formula_parse
);
criterion_main!(benches);
