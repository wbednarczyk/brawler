//! The ONE home of "which fact represents a slot" (#496; ADR 0093 dec. 2
//! final-first; ADR 0086): a SQL `ORDER BY` fragment and its Rust mirror.
//! Every reader that picks a representative fact for `(company, metric,
//! period)` — comparison facts, quality frameworks, the expectation review,
//! the KPI table, `latest_shares_outstanding`, `list_financial_facts` —
//! interpolates the fragment or sorts by the rank; never a private copy
//! (data-model.md § Model principles).

use super::FinancialFact;

/// ONE home of "which fact represents a slot" (ADR 0093 dec. 2 final-first;
/// ADR 0086): SQL `ORDER BY` fragment (alias `f`) — final, then reported,
/// then consolidated, then total › owners_of_parent › other. Interpolate,
/// never re-derive; Rust mirror: [`canonical_fact_rank`].
pub(crate) const CANONICAL_FACT_PREFERENCE_ORDER: &str = "CASE f.data_quality WHEN 'final' THEN 0 ELSE 1 END, CASE f.variant WHEN 'reported' THEN 0 ELSE 1 END, CASE f.statement_basis WHEN 'consolidated' THEN 0 ELSE 1 END, CASE f.attribution WHEN 'total' THEN 0 WHEN 'owners_of_parent' THEN 1 ELSE 2 END";

/// Rust mirror of [`CANONICAL_FACT_PREFERENCE_ORDER`] (lower sorts first).
pub(crate) fn canonical_fact_rank(fact: &FinancialFact) -> (u8, u8, u8, u8) {
    (
        u8::from(fact.data_quality != "final"),
        u8::from(fact.variant != "reported"),
        u8::from(fact.statement_basis != "consolidated"),
        match fact.attribution.as_str() {
            "total" => 0,
            "owners_of_parent" => 1,
            _ => 2,
        },
    )
}
