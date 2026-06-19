//! Capability selection registry for the interpretative layer (ADR 0035,
//! section 6).
//!
//! Each capability resolves to an active *strategy*: `static` (the deterministic
//! baseline) or `embedding` (the on-device model, ADR 0035, v0.45.0). The
//! embedding strategy requires a loaded [`Embedder`]; without one (feature off,
//! weights absent, or load failure) [`build_similarity_with`] returns
//! [`InterpretationError::Unavailable`] rather than a wrong result, and callers
//! fall back to static.
//!
//! Consumers build a capability through these factories instead of constructing
//! a concrete implementation, so swapping the active strategy requires no
//! consumer change. Selection defaults to static.

use std::sync::Arc;

use super::{
    CategoryRule, Classifier, Embedder, EmbeddingSimilarity, InterpretationError,
    LexicalSimilarity, RuleClassifier, SimilarityProvider,
};

/// The deterministic, always-available baseline strategy.
pub const STATIC_STRATEGY: &str = "static";
/// The on-device embedding-model strategy (v0.45.0).
pub const EMBEDDING_STRATEGY: &str = "embedding";

/// The active strategy per capability. Defaults to static for every capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretationSelection {
    pub classifier: String,
    pub similarity: String,
    pub matcher: String,
    pub search: String,
}

impl Default for InterpretationSelection {
    fn default() -> Self {
        Self {
            classifier: STATIC_STRATEGY.to_string(),
            similarity: STATIC_STRATEGY.to_string(),
            matcher: STATIC_STRATEGY.to_string(),
            search: STATIC_STRATEGY.to_string(),
        }
    }
}

/// Map a requested strategy to the right error: a known-but-unavailable strategy
/// (embedding, with no loaded model) to `Unavailable`, an unknown one to
/// `InvalidRequest`.
fn unavailable_or_invalid(strategy: &str) -> InterpretationError {
    if strategy == EMBEDDING_STRATEGY {
        InterpretationError::Unavailable(
            "embedding model is not loaded (feature disabled or weights absent)".to_string(),
        )
    } else {
        InterpretationError::InvalidRequest(format!("unknown interpretation strategy: {strategy}"))
    }
}

/// Build the [`Classifier`] for the given strategy. The `static` strategy returns
/// a rule classifier configured with `rules` (the taxonomy is supplied by the
/// consumer, e.g. the ESPI `signal_categories` rules). The model-backed
/// classifier is deferred to its consumer (ADR 0035), so `embedding` is
/// unavailable here.
pub fn build_classifier(
    strategy: &str,
    rules: Vec<CategoryRule>,
) -> Result<Box<dyn Classifier>, InterpretationError> {
    match strategy {
        STATIC_STRATEGY => Ok(Box::new(RuleClassifier::new(rules))),
        other => Err(unavailable_or_invalid(other)),
    }
}

/// Build the [`SimilarityProvider`] for the `static` strategy (lexical baseline).
/// Requesting `embedding` here is unavailable — use [`build_similarity_with`]
/// with a loaded embedder. Kept for static-only call sites.
pub fn build_similarity(
    strategy: &str,
) -> Result<Box<dyn SimilarityProvider>, InterpretationError> {
    build_similarity_with(strategy, None)
}

/// Build the [`SimilarityProvider`] for the given strategy, supplying an optional
/// loaded [`Embedder`]. `static` returns the lexical baseline; `embedding`
/// returns the model-backed provider when `embedder` is `Some`, else
/// `Unavailable` so the caller falls back to static.
pub fn build_similarity_with(
    strategy: &str,
    embedder: Option<Arc<dyn Embedder>>,
) -> Result<Box<dyn SimilarityProvider>, InterpretationError> {
    match strategy {
        STATIC_STRATEGY => Ok(Box::new(LexicalSimilarity::new())),
        EMBEDDING_STRATEGY => match embedder {
            Some(embedder) => Ok(Box::new(EmbeddingSimilarity::new(embedder))),
            None => Err(unavailable_or_invalid(EMBEDDING_STRATEGY)),
        },
        other => Err(unavailable_or_invalid(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_is_static_everywhere() {
        let selection = InterpretationSelection::default();
        assert_eq!(selection.classifier, STATIC_STRATEGY);
        assert_eq!(selection.similarity, STATIC_STRATEGY);
        assert_eq!(selection.matcher, STATIC_STRATEGY);
        assert_eq!(selection.search, STATIC_STRATEGY);
    }

    #[test]
    fn builds_static_implementations() {
        assert!(build_classifier(STATIC_STRATEGY, Vec::new()).is_ok());
        assert!(build_similarity(STATIC_STRATEGY).is_ok());
    }

    #[test]
    fn embedding_strategy_is_unavailable_for_now() {
        assert!(matches!(
            build_similarity(EMBEDDING_STRATEGY),
            Err(InterpretationError::Unavailable(_))
        ));
        assert!(matches!(
            build_classifier(EMBEDDING_STRATEGY, Vec::new()),
            Err(InterpretationError::Unavailable(_))
        ));
    }

    #[test]
    fn unknown_strategy_is_invalid() {
        assert!(matches!(
            build_similarity("nonsense"),
            Err(InterpretationError::InvalidRequest(_))
        ));
    }
}
