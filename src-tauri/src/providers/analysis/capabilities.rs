//! AI capability model (ADR 0060 as amended): every distinct AI call site is a
//! named capability, routable to an ordered (provider, model) pool.

/// Sentinel stored in a job row's provider_id when the job is routed by
/// capability at run time instead of an explicit user override (ADR 0060).
pub const CAPABILITY_ROUTED_PROVIDER_ID: &str = "capability_routed";

/// A distinct AI call site the app can route independently. Each variant maps
/// to a `capability_providers` settings key and an ordered fallback pool of
/// (provider, model) pairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiCapability {
    /// Structured-first KPI extraction from a filing document (ESEF/iXBRL/PDF
    /// witness) — the fundamentals-reliability pipeline's document call site.
    KpiExtraction,
    /// Extraction of sourced claims from a document for the claims/provenance
    /// panel.
    ClaimExtraction,
    /// General feed-item analysis (summaries, tagging) — the
    /// `general_analysis_*` legacy call site.
    FeedAnalysis,
    /// Research-brief generation for a company/topic.
    ResearchBrief,
    /// Research-digest generation across multiple briefs/sources.
    ResearchDigest,
    /// ESPI event-date extraction.
    EventDate,
    /// ESPI signal/category classification.
    SignalClassification,
}

/// Whether a capability's provider call takes a document (structured-first
/// pipeline, ADR 0061) or plain text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityKind {
    Document,
    Text,
}

impl AiCapability {
    pub const ALL: [AiCapability; 7] = [
        AiCapability::KpiExtraction,
        AiCapability::ClaimExtraction,
        AiCapability::FeedAnalysis,
        AiCapability::ResearchBrief,
        AiCapability::ResearchDigest,
        AiCapability::EventDate,
        AiCapability::SignalClassification,
    ];

    /// Stable settings-map key (`capability_providers` HashMap key) and wire
    /// identifier for this capability.
    pub fn key(self) -> &'static str {
        match self {
            AiCapability::KpiExtraction => "kpi_extraction",
            AiCapability::ClaimExtraction => "claim_extraction",
            AiCapability::FeedAnalysis => "feed_analysis",
            AiCapability::ResearchBrief => "research_brief",
            AiCapability::ResearchDigest => "research_digest",
            AiCapability::EventDate => "event_date",
            AiCapability::SignalClassification => "signal_classification",
        }
    }

    /// Whether this capability's provider call takes a document or plain text.
    pub fn kind(self) -> CapabilityKind {
        match self {
            AiCapability::KpiExtraction | AiCapability::ClaimExtraction => CapabilityKind::Document,
            AiCapability::FeedAnalysis
            | AiCapability::ResearchBrief
            | AiCapability::ResearchDigest
            | AiCapability::EventDate
            | AiCapability::SignalClassification => CapabilityKind::Text,
        }
    }

    /// Inverse of [`AiCapability::key`]; `None` for an unrecognized key.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.key() == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_key_roundtrips_every_capability() {
        for capability in AiCapability::ALL {
            assert_eq!(AiCapability::from_key(capability.key()), Some(capability));
        }
    }

    #[test]
    fn from_key_rejects_unknown_key() {
        assert_eq!(AiCapability::from_key("not_a_capability"), None);
    }

    #[test]
    fn all_keys_are_unique() {
        let mut keys: Vec<&str> = AiCapability::ALL
            .iter()
            .map(|capability| capability.key())
            .collect();
        let original_len = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), original_len, "capability keys must be unique");
    }

    #[test]
    fn document_capabilities_are_kpi_and_claim_extraction() {
        assert_eq!(AiCapability::KpiExtraction.kind(), CapabilityKind::Document);
        assert_eq!(
            AiCapability::ClaimExtraction.kind(),
            CapabilityKind::Document
        );
    }

    #[test]
    fn remaining_capabilities_are_text() {
        for capability in [
            AiCapability::FeedAnalysis,
            AiCapability::ResearchBrief,
            AiCapability::ResearchDigest,
            AiCapability::EventDate,
            AiCapability::SignalClassification,
        ] {
            assert_eq!(capability.kind(), CapabilityKind::Text);
        }
    }
}
