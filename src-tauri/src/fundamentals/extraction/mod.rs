//! Structured-first fundamentals extraction (ADR 0061).
//!
//! The deterministic pipeline that turns a report document into validated
//! [`super::validation::FactSet`]s. Each tier is a self-contained parser that
//! emits [`ExtractedFact`]s tagged with the [`SourceTier`] they came from; the
//! orchestrator (wired in the autopilot slice) walks tiers highest-first and
//! runs every candidate set through the [`super::validation`] gate before any
//! fact is persisted.
//!
//! Tiers, highest trust first:
//! 1. [`esef`] — ESEF/iXBRL inline-XBRL facts (tagged IFRS concepts). Tier 1.
//! 2. Structured xHTML "wybrane dane" tables (reuses the iXBRL table seam).
//! 3. PDF parser + per-company extraction profiles (S3).
//! 4. HTML aggregator witness/fallback (S4).
//! 5. AI over extracted text, via the provider pool (S6) — last resort only.

use rust_decimal::Decimal;
use std::collections::BTreeMap;

use super::validation::FactSet;

pub mod classify;
pub mod esef;
pub mod esef_package;
pub mod html;
pub mod html_positional;
pub mod ocr;
pub mod pdf;
pub mod pipeline;
pub mod profile;

/// Which tier of the pipeline produced a fact. Ordered highest-trust first so
/// `SourceTier::Esef < SourceTier::Pdf` reflects precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceTier {
    /// Tagged inline-XBRL (ESEF annual filing) — the source of truth.
    Esef,
    /// Structured xHTML "selected financial data" table.
    StructuredXhtml,
    /// Deterministic PDF parser + per-company profile.
    Pdf,
    /// HTML financial-data aggregator (witness / fallback).
    HtmlAggregator,
    /// AI over extracted text (last resort).
    AiText,
}

impl SourceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceTier::Esef => "esef",
            SourceTier::StructuredXhtml => "structured_xhtml",
            SourceTier::Pdf => "pdf",
            SourceTier::HtmlAggregator => "html_aggregator",
            SourceTier::AiText => "ai_text",
        }
    }
}

/// The period a fact refers to: an instant (balance-sheet date) or a duration
/// (P&L / cash-flow span). Dates are ISO `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FactPeriod {
    Instant(String),
    Duration { start: String, end: String },
}

impl FactPeriod {
    /// The period-end date, used to align facts to a reporting period.
    pub fn end_date(&self) -> &str {
        match self {
            FactPeriod::Instant(d) => d,
            FactPeriod::Duration { end, .. } => end,
        }
    }
}

/// Consolidation basis, where the source distinguishes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementBasis {
    Consolidated,
    Standalone,
}

impl StatementBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            StatementBasis::Consolidated => "consolidated",
            StatementBasis::Standalone => "standalone",
        }
    }
}

/// One extracted metric value, before it is grouped and validated.
///
/// `value` is signed and in base units (scale already applied). `citation`
/// carries the source concept/label so every persisted fact keeps a primary
/// citation (ADR 0061 guardrail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedFact {
    pub metric_key: String,
    pub value: Decimal,
    pub period: FactPeriod,
    pub basis: Option<StatementBasis>,
    pub currency: Option<String>,
    pub tier: SourceTier,
    /// The source concept/label this value was read from (primary citation).
    pub citation: String,
}

/// The latest period-end date across a set of extracted facts — the report's
/// primary reporting period. Used to self-derive the period for an ESEF filing
/// (whose contexts carry the dates) when no period is supplied out of band.
pub fn primary_period_end(facts: &[ExtractedFact]) -> Option<String> {
    facts.iter().map(|f| f.period.end_date().to_string()).max()
}

/// Groups extracted facts into a [`FactSet`] for the given period-end date,
/// keeping the highest-tier value when a metric is produced more than once.
/// Facts whose period-end does not match `period_end` are ignored.
pub fn fact_set_for_period(facts: &[ExtractedFact], period_end: &str) -> FactSet {
    // Highest trust wins: iterate and keep the fact with the smallest tier
    // (Esef < Pdf) per metric_key.
    let mut best: BTreeMap<String, (SourceTier, Decimal)> = BTreeMap::new();
    for f in facts.iter().filter(|f| f.period.end_date() == period_end) {
        best.entry(f.metric_key.clone())
            .and_modify(|slot| {
                if f.tier < slot.0 {
                    *slot = (f.tier, f.value);
                }
            })
            .or_insert((f.tier, f.value));
    }
    best.into_iter().map(|(k, (_, v))| (k, v)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(metric: &str, value: i64, end: &str, tier: SourceTier) -> ExtractedFact {
        ExtractedFact {
            metric_key: metric.to_string(),
            value: Decimal::from(value),
            period: FactPeriod::Instant(end.to_string()),
            basis: Some(StatementBasis::Consolidated),
            currency: Some("PLN".into()),
            tier,
            citation: metric.to_string(),
        }
    }

    #[test]
    fn groups_only_matching_period_end() {
        let facts = vec![
            fact("total_assets", 100, "2026-03-31", SourceTier::Esef),
            fact("total_assets", 90, "2025-03-31", SourceTier::Esef),
        ];
        let set = fact_set_for_period(&facts, "2026-03-31");
        assert_eq!(set.get("total_assets"), Some(&Decimal::from(100)));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn highest_tier_wins_on_conflict() {
        let facts = vec![
            fact("total_assets", 100, "2026-03-31", SourceTier::Pdf),
            fact("total_assets", 101, "2026-03-31", SourceTier::Esef),
        ];
        let set = fact_set_for_period(&facts, "2026-03-31");
        // ESEF (tier 1) wins over PDF (tier 3).
        assert_eq!(set.get("total_assets"), Some(&Decimal::from(101)));
    }

    #[test]
    fn tier_ordering_is_trust_ordering() {
        assert!(SourceTier::Esef < SourceTier::Pdf);
        assert!(SourceTier::Pdf < SourceTier::HtmlAggregator);
        assert!(SourceTier::HtmlAggregator < SourceTier::AiText);
    }
}
