//! The interpretative-layer capability contracts (ADR 0035).
//!
//! Each trait is one task-level capability that feature code depends on. They
//! are `async` (consistent with the async AI boundary, ADR 0028) so a future
//! model-backed implementation can do IO/inference without blocking; static
//! implementations simply return ready results. All four traits are defined now
//! (cheap); their static implementations are added just-in-time as consumers
//! arrive — `Classifier` for ESPI classification (v0.40.0), `SimilarityProvider`
//! for story clustering (v0.47.0), `Matcher`/`SemanticSearch` for claims and
//! hybrid search.

use async_trait::async_trait;

use super::types::{
    Classification, ClassificationRequest, InterpretationError, ScoredItem, SearchScope, TextItem,
};

/// Classify a piece of text into one category of a taxonomy.
///
/// The taxonomy and matching rules are configured into the implementation; the
/// per-call [`ClassificationRequest`] carries the text. A confident result names
/// a category; an uncertain one returns the unknown outcome
/// ([`Classification::category`] = `None`) rather than guessing.
#[async_trait]
pub trait Classifier: Send + Sync {
    /// Stable identifier of this implementation (e.g. `"classifier_rules"`).
    fn id(&self) -> &str;

    /// Classify `request.text` into one category (or unknown).
    async fn classify(
        &self,
        request: ClassificationRequest,
    ) -> Result<Classification, InterpretationError>;
}

/// Rank items by similarity — "what is this like" / near-duplicate detection.
/// First model consumer: story clustering.
#[async_trait]
pub trait SimilarityProvider: Send + Sync {
    /// Stable identifier of this implementation (e.g. `"similarity_lexical"`).
    fn id(&self) -> &str;

    /// A relative similarity score in `0.0..=1.0` between two texts.
    async fn score(&self, a: &str, b: &str) -> Result<f32, InterpretationError>;

    /// The `k` candidates most similar to `query`, highest score first.
    async fn most_similar(
        &self,
        query: &str,
        candidates: Vec<TextItem>,
        k: usize,
    ) -> Result<Vec<ScoredItem>, InterpretationError>;
}

/// Rank candidates by relatedness to a query — "what does this match"
/// (e.g. a management claim against financial facts, evidence against a
/// research question). Distinct from [`SimilarityProvider`] in intent: matching
/// a query to a different kind of target rather than near-duplicate detection.
#[async_trait]
pub trait Matcher: Send + Sync {
    /// Stable identifier of this implementation (e.g. `"matcher_lexical"`).
    fn id(&self) -> &str;

    /// The `k` candidates that best match `query`, highest score first.
    async fn match_candidates(
        &self,
        query: &str,
        candidates: Vec<TextItem>,
        k: usize,
    ) -> Result<Vec<ScoredItem>, InterpretationError>;
}

/// Retrieve the most relevant stored items for a query over a corpus scope.
/// The static baseline is keyword retrieval over the existing FTS5 index
/// (ADR 0032); a model implementation makes this hybrid (keyword + vector).
#[async_trait]
pub trait SemanticSearch: Send + Sync {
    /// Stable identifier of this implementation (e.g. `"search_fts"`).
    fn id(&self) -> &str;

    /// The `k` items most relevant to `query` within `scope`, highest score first.
    async fn search(
        &self,
        query: &str,
        scope: SearchScope,
        k: usize,
    ) -> Result<Vec<ScoredItem>, InterpretationError>;
}
