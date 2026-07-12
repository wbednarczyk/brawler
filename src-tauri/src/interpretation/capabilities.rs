//! The interpretative-layer capability contracts (ADR 0035).
//!
//! Each trait is one task-level capability that feature code depends on. They
//! are `async` (consistent with the async AI boundary, ADR 0028) so a future
//! model-backed implementation can do IO/inference without blocking; static
//! implementations simply return ready results. Since ADR 0080 retired the
//! embedding model, [`Classifier`] is the only capability with a live consumer
//! (ESPI signal classification, `storage/signals.rs`); the similarity, matcher,
//! and semantic-search contracts were removed with their implementations and
//! re-enter through a fresh eval-gated ADR if a consumer ever arrives.

use async_trait::async_trait;

use super::types::{Classification, ClassificationRequest, InterpretationError};

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
