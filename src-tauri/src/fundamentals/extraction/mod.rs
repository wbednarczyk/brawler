//! Structured-first fundamentals extraction (ADR 0061).
//!
//! The deterministic pipeline that turns a report document into validated
//! [`super::validation::FactSet`]s. Each tier is a self-contained parser that
//! emits [`ExtractedFact`]s tagged with the [`SourceTier`] they came from; the
//! orchestrator (wired in the autopilot slice) walks tiers highest-first and
//! runs every candidate set through the [`super::validation`] gate before any
//! fact is persisted.
//!
//! Tiers, highest trust first (ADR 0086 dec. 3, superseding ADR 0061 dec. 1/3):
//! 1. [`esef`] — ESEF/iXBRL inline-XBRL facts (tagged IFRS concepts). Tier 1.
//! 2. Structured xHTML / positional "wybrane dane" tables ([`html_positional`],
//!    persisted under `source_tier='pdf'`, `extraction_method='html_positional'`).
//! 3. [`espi_cover_note`] — the ESPI cover-note "WYBRANE DANE FINANSOWE" table,
//!    parsed from the already-ingested komunikat body (zero-fetch, WDF).
//! 4. [`html`] — the BiznesRadar aggregator, now the PRIMARY source for core KPIs
//!    (`source_tier='html_aggregator'`, pulled daily by a separate job).
//!
//! The PDF fact-extraction arm is RETIRED (ADR 0086 dec. 1): no tier reads
//! financial facts out of PDF statements anymore — its shared number/label
//! helpers moved to [`text_numbers`], serving the html + positional tiers. The
//! pipeline is deterministic end to end; a document no deterministic tier parses
//! is flagged, never guessed. (The tier-5 "AI over extracted text" was already
//! retired with the in-app AI layer, ADR 0084 decision 4.)

use rust_decimal::Decimal;
use std::collections::BTreeMap;

use super::validation::FactSet;

pub mod classify;
pub mod container;
pub mod esef;
pub mod esef_package;
pub mod espi_cover_note;
pub mod html;
pub mod html_positional;
pub mod pipeline;
pub mod text_numbers;

/// Which tier of the pipeline produced a fact. Ordered highest-trust first so
/// `SourceTier::Esef < SourceTier::Pdf` reflects precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceTier {
    /// Tagged inline-XBRL (ESEF annual filing) — the source of truth.
    Esef,
    /// Structured xHTML "selected financial data" table.
    StructuredXhtml,
    /// ESPI cover-note "WYBRANE DANE FINANSOWE" table, parsed from the already
    /// ingested komunikat body (zero-fetch). Ranks below the tagged/structured
    /// tiers — issuer cover-note figures are untagged — and above the PDF
    /// parser. ADR 0061 decision 1 tier 2a.
    EspiCoverNote,
    /// Structured/positional xHTML reader token. Historically the deterministic
    /// PDF-fact parser (retired, ADR 0086 dec. 1); the token is KEPT because the
    /// surviving positional tier ([`html_positional`]) persists under it with
    /// `extraction_method='html_positional'`. Legacy `extraction_method='api'`
    /// rows under this tier are the retired PDF arm's output, distinguishable by
    /// method.
    Pdf,
    /// HTML financial-data aggregator (witness / fallback).
    HtmlAggregator,
}

impl SourceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceTier::Esef => "esef",
            SourceTier::StructuredXhtml => "structured_xhtml",
            SourceTier::EspiCoverNote => "espi_cover_note",
            SourceTier::Pdf => "pdf",
            SourceTier::HtmlAggregator => "html_aggregator",
        }
    }

    /// The tier a stored `financial_fact_provenance.source_tier` string names.
    /// `None` for an unknown/legacy marker (e.g. the retired `ai_text` rows ADR
    /// 0084 left readable) — the caller then treats the stored fact as of
    /// unknown trust and never outranks it.
    pub fn parse(value: &str) -> Option<SourceTier> {
        match value {
            "esef" => Some(SourceTier::Esef),
            "structured_xhtml" => Some(SourceTier::StructuredXhtml),
            "espi_cover_note" => Some(SourceTier::EspiCoverNote),
            "pdf" => Some(SourceTier::Pdf),
            "html_aggregator" => Some(SourceTier::HtmlAggregator),
            _ => None,
        }
    }

    /// Whether `self` is strictly more trusted than `other` (the tier order is
    /// highest-trust first, so a *smaller* variant outranks). ADR 0061 decision
    /// 1: "a KPI is taken from the highest available tier".
    pub fn outranks(self, other: SourceTier) -> bool {
        self < other
    }

    /// Whether this tier is an ISSUER-produced tier — one read from the issuer's
    /// own filing, whose held slot records a reversed-witnessing
    /// `witness_disagreement` when the aggregator diverges (ADR 0086 decision 4,
    /// amended 2026-07-22: the positional `Pdf` tier is the issuer's filing read
    /// deterministically and counts as an issuer tier). The aggregator's own
    /// `HtmlAggregator` tier is the only non-issuer tier.
    pub fn is_issuer(self) -> bool {
        !matches!(self, SourceTier::HtmlAggregator)
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
        assert!(SourceTier::Esef < SourceTier::StructuredXhtml);
        assert!(SourceTier::StructuredXhtml < SourceTier::EspiCoverNote);
        assert!(SourceTier::EspiCoverNote < SourceTier::Pdf);
        assert!(SourceTier::Pdf < SourceTier::HtmlAggregator);
    }

    /// Guardrail: `parse` must round-trip **every** `as_str` marker. Stored
    /// provenance is compared by tier to decide whether a re-extraction may
    /// upgrade an occupied slot (ADR 0061 dec. 1); a tier `as_str` writes but
    /// `parse` cannot read back would silently make that comparison unknowable.
    #[test]
    fn every_tier_marker_round_trips_through_parse() {
        for tier in [
            SourceTier::Esef,
            SourceTier::StructuredXhtml,
            SourceTier::EspiCoverNote,
            SourceTier::Pdf,
            SourceTier::HtmlAggregator,
        ] {
            assert_eq!(
                SourceTier::parse(tier.as_str()),
                Some(tier),
                "tier {tier:?} does not round-trip"
            );
        }
        // An unknown/legacy marker is never mistaken for a known tier — the
        // fact it belongs to is then never outranked (fails safe).
        assert_eq!(SourceTier::parse("ai_text"), None);
        assert_eq!(SourceTier::parse("ai"), None);
    }
}
