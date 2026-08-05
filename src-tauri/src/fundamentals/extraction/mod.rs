//! Structured-first fundamentals extraction (ADR 0061).
//!
//! The deterministic pipeline that turns a report document into validated
//! [`super::validation::FactSet`]s. Each tier is a self-contained parser that
//! emits [`ExtractedFact`]s tagged with the [`SourceTier`] they came from; the
//! orchestrator (wired in the autopilot slice) walks tiers highest-first and
//! runs every candidate set through the [`super::validation`] gate before any
//! fact is persisted.
//!
//! Tiers, highest trust first (ADR 0086 dec. 3, superseding ADR 0061 dec. 1/3;
//! `Agent` added by ADR 0093 dec. 1):
//! 1. [`esef`] — ESEF/iXBRL inline-XBRL facts (tagged IFRS concepts). Tier 1.
//! 2. [`espi_cover_note`] — the ESPI cover-note "WYBRANE DANE FINANSOWE" table,
//!    parsed from the already-ingested komunikat body (zero-fetch, WDF).
//! 3. `Agent` — an MCP-connected agent's read of an issuer document, recorded
//!    via the port's write tools (`source_tier='agent'`, `extraction_method=
//!    'mcp_agent'`). NOT an issuer tier (no deterministic pipeline produced
//!    it), but ranked above the aggregator: an agent reads the issuer's own
//!    document, the aggregator is third-party. ADR 0093 decision 1.
//! 4. [`html`] — the BiznesRadar aggregator, now the PRIMARY source for core KPIs
//!    (`source_tier='html_aggregator'`, pulled daily by a separate job).
//!
//! The PDF fact-extraction arm is RETIRED (ADR 0086 dec. 1) and the
//! structured-xHTML/positional "wybrane dane" tier is RETIRED (ADR 0095): no
//! tier reads financial facts out of PDF statements or bare pdf2htmlEX
//! renders — the PDF arm's shared number/label helpers moved to
//! [`text_numbers`], still serving the html tier. `SourceTier::Pdf` and
//! `extraction_method='html_positional'` stay recognized as legacy READ
//! values only; the coherence guard,
//! `storage::kpi_extraction::write_fact_provenance_fields`, refuses
//! `source_tier='pdf'` on every NEW write. The pipeline is deterministic
//! end to end; a document no deterministic tier parses is flagged, never
//! guessed.

use rust_decimal::Decimal;
use std::collections::BTreeMap;

use super::validation::FactSet;

pub mod classify;
pub mod container;
pub mod esef;
pub mod esef_package;
pub mod espi_cover_note;
pub mod html;
pub mod pipeline;
pub mod text_numbers;

/// Real committed ESPI filing sample tests (ADR 0094, epic #40 / #139): exercises
/// the container/ESEF/PDF-honesty seams against actual issuer filings instead of
/// inline string literals. Test-only; never compiled into the shipped binary.
#[cfg(test)]
mod real_report_samples;

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
    /// Structured/positional xHTML reader token — the deterministic PDF-fact
    /// parser (ADR 0086 dec. 1) and the tier-3b positional pdf2htmlEX parser
    /// (ADR 0095) are both retired. The token is KEPT only as a legacy READ
    /// value — `storage::kpi_extraction::write_fact_provenance_fields`
    /// refuses `source_tier='pdf'` on every NEW write.
    Pdf,
    /// An MCP-connected agent's write, provenanced `source_tier='agent'` /
    /// `extraction_method='mcp_agent'` (ADR 0093 decision 1, epic #285). Ranked
    /// BELOW every issuer tier above — no deterministic pipeline verified it, so
    /// [`is_issuer`](Self::is_issuer) is `false` for this tier — and ABOVE
    /// [`HtmlAggregator`](Self::HtmlAggregator): an agent reads the issuer's own
    /// document, the aggregator is third-party. An issuer tier landing on an
    /// agent-held slot always UPGRADES it (the existing `Upgraded` machinery); a
    /// mis-extracted agent figure can never block the audited correction. The
    /// agent tier never overwrites an issuer-held or manual slot — a
    /// disagreement there is a `Divergent` outcome, reported, never resolved
    /// silently.
    Agent,
    /// HTML financial-data aggregator (BiznesRadar). **Primary source of the
    /// core KPI history** (ADR 0086 dec. 2) and the LOWEST precedence tier: it
    /// fills every slot no issuer (or agent) tier owns, never overwrites one
    /// that is owned, and against an owned slot it acts as a reversed witness —
    /// recording a `witness_disagreement` outcome on divergence and a
    /// corroboration stamp on agreement (epic #229 T5).
    HtmlAggregator,
}

impl SourceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceTier::Esef => "esef",
            SourceTier::StructuredXhtml => "structured_xhtml",
            SourceTier::EspiCoverNote => "espi_cover_note",
            SourceTier::Pdf => "pdf",
            SourceTier::Agent => "agent",
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
            "agent" => Some(SourceTier::Agent),
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

    /// Whether this tier is an ISSUER-produced tier — one read DETERMINISTICALLY
    /// from the issuer's own filing (ADR 0086 decision 4: the positional `Pdf`
    /// tier is the issuer's filing read deterministically and counts as an
    /// issuer tier). Amended by ADR 0093 decision 1: the
    /// `Agent` tier also reads the issuer's own document, but no deterministic
    /// pipeline verified it, so it is **not** an issuer tier either — it is
    /// distinguished from `HtmlAggregator` (third-party) elsewhere by rank, not
    /// by this predicate.
    ///
    /// This predicate answers "is this genuinely the issuer's own verified
    /// filing" — used where that specific fact matters (e.g. the derived
    /// kpi_relevance observation, `refresh_derived_kpi_relevance`). It is
    /// deliberately NOT the same question as reversed-witnessing precedence
    /// ("does the stored tier outrank `HtmlAggregator`?", i.e.
    /// `outranks(SourceTier::HtmlAggregator)`) — the agent tier answers that one
    /// `true` while answering `is_issuer` `false`. Call sites that mean
    /// witnessing/precedence must use `outranks`, not this method.
    pub fn is_issuer(self) -> bool {
        !matches!(self, SourceTier::HtmlAggregator | SourceTier::Agent)
    }

    /// Whether `extraction_method` (the `financial_facts` sub-tier marker) is a
    /// route THIS tier can legitimately come from (bug #324 guard). Tier drives
    /// trust semantics (ADR 0061/0086); a provenance write where the two
    /// disagree silently mislabels a fact's real origin — e.g. 7 real rows on
    /// the maintainer's DB carried `source_tier='esef'` while
    /// `extraction_method='html_positional'` named the positional/`pdf` route
    /// that actually produced them, because a tier-precedence UPGRADE
    /// (`record_structured_fact`) rewrote the provenance tier without syncing
    /// the fact's `extraction_method` to match.
    ///
    /// `manual` is exempt (always coherent): a production hand-entered fact
    /// never gets a provenance row at all (untouchable, ADR 0086 decision 3),
    /// so the marker only ever appears in a provenance row as an internal
    /// test-harness seeding trick, never live data. An unrecognized
    /// `extraction_method` is never asserted incoherent either — this map
    /// fails open on an unmapped literal rather than tripping every write in a
    /// future caller until the map is updated; see
    /// `every_known_extraction_method_pairs_with_exactly_its_own_tiers` for
    /// the exhaustive enumerated allowlist this guards.
    pub fn matches_extraction_method(self, extraction_method: &str) -> bool {
        match extraction_method {
            "html_positional" => self == SourceTier::Pdf,
            "espi_cover_note" => self == SourceTier::EspiCoverNote,
            "mcp_agent" => self == SourceTier::Agent,
            // The deterministic tiered pipeline's generic write (esef /
            // structured_xhtml / espi_cover_note / html_aggregator, all
            // `jobs::structured_extraction`), plus the retired PDF fact arm's
            // legacy rows (still readable, never written fresh).
            "api" => matches!(
                self,
                SourceTier::Esef
                    | SourceTier::StructuredXhtml
                    | SourceTier::EspiCoverNote
                    | SourceTier::Pdf
                    | SourceTier::HtmlAggregator
            ),
            "manual" => true,
            _ => true,
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
        assert!(SourceTier::Esef < SourceTier::StructuredXhtml);
        assert!(SourceTier::StructuredXhtml < SourceTier::EspiCoverNote);
        assert!(SourceTier::EspiCoverNote < SourceTier::Pdf);
        // ADR 0093 decision 1: `Agent` sits below every issuer tier and above
        // `HtmlAggregator`.
        assert!(SourceTier::Pdf < SourceTier::Agent);
        assert!(SourceTier::Agent < SourceTier::HtmlAggregator);
    }

    /// ADR 0093 decision 1: every issuer tier outranks `Agent`, and `Agent`
    /// outranks `HtmlAggregator` — pinned directly against `outranks`, not just
    /// the derived `Ord`, since the precedence machinery calls `outranks`.
    #[test]
    fn every_issuer_tier_outranks_agent_which_outranks_the_aggregator() {
        for issuer_tier in [
            SourceTier::Esef,
            SourceTier::StructuredXhtml,
            SourceTier::EspiCoverNote,
            SourceTier::Pdf,
        ] {
            assert!(
                issuer_tier.outranks(SourceTier::Agent),
                "{issuer_tier:?} must outrank Agent"
            );
        }
        assert!(SourceTier::Agent.outranks(SourceTier::HtmlAggregator));
        assert!(!SourceTier::HtmlAggregator.outranks(SourceTier::Agent));
    }

    /// ADR 0093 decision 1: `Agent` is explicitly NOT an issuer tier (no
    /// deterministic pipeline verified it), unlike every other non-aggregator
    /// tier.
    #[test]
    fn agent_is_not_an_issuer_tier() {
        assert!(!SourceTier::Agent.is_issuer());
        assert!(!SourceTier::HtmlAggregator.is_issuer());
        for issuer_tier in [
            SourceTier::Esef,
            SourceTier::StructuredXhtml,
            SourceTier::EspiCoverNote,
            SourceTier::Pdf,
        ] {
            assert!(
                issuer_tier.is_issuer(),
                "{issuer_tier:?} must be an issuer tier"
            );
        }
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
            SourceTier::Agent,
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

    /// Bug #324 guard: the exhaustive `extraction_method` → allowed
    /// `source_tier` set every provenance write must respect. Every known
    /// `financial_facts.extraction_method` literal in the codebase is listed
    /// here (see `storage::kpi_extraction::write_fact_provenance`'s
    /// `debug_assert`); a tier NOT in a method's allowed set must be rejected —
    /// this is the case that reddens on the reported incoherent pair
    /// (`esef` + `html_positional`).
    #[test]
    fn every_known_extraction_method_pairs_with_exactly_its_own_tiers() {
        let all_tiers = [
            SourceTier::Esef,
            SourceTier::StructuredXhtml,
            SourceTier::EspiCoverNote,
            SourceTier::Pdf,
            SourceTier::Agent,
            SourceTier::HtmlAggregator,
        ];
        let cases: &[(&str, &[SourceTier])] = &[
            ("html_positional", &[SourceTier::Pdf]),
            ("espi_cover_note", &[SourceTier::EspiCoverNote]),
            ("mcp_agent", &[SourceTier::Agent]),
            (
                "api",
                &[
                    SourceTier::Esef,
                    SourceTier::StructuredXhtml,
                    SourceTier::EspiCoverNote,
                    SourceTier::Pdf,
                    SourceTier::HtmlAggregator,
                ],
            ),
        ];
        for (method, allowed) in cases {
            for tier in all_tiers {
                assert_eq!(
                    tier.matches_extraction_method(method),
                    allowed.contains(&tier),
                    "tier {tier:?} x method {method:?}"
                );
            }
        }
        // The reported bug #324 pair, pinned directly.
        assert!(
            !SourceTier::Esef.matches_extraction_method("html_positional"),
            "esef + html_positional is the exact incoherent pair bug #324 reported"
        );
        // `manual` and an unmapped literal are unconstrained (fail open).
        for tier in all_tiers {
            assert!(tier.matches_extraction_method("manual"));
            assert!(tier.matches_extraction_method("some_future_method"));
        }
    }
}
