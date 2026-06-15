//! Capability selection registry for the interpretative layer (ADR 0035,
//! section 6).
//!
//! Each capability resolves to an active *strategy* — currently only `static`
//! (the shipped baseline). The `embedding` strategy is reserved for the
//! embedding-model milestone (v0.46.0) and is not yet available; requesting it
//! returns [`InterpretationError::Unavailable`] rather than a wrong result.
//!
//! Consumers build a capability through these factories instead of constructing
//! a concrete implementation, so swapping the active strategy later requires no
//! consumer change. Selection defaults to static; a user-facing selection UI is
//! deferred until there is a real model-vs-static choice (v0.46.0).

use super::{
    CategoryRule, Classifier, InterpretationError, LexicalSimilarity, RuleClassifier,
    SimilarityProvider,
};

/// The deterministic, always-available baseline strategy.
pub const STATIC_STRATEGY: &str = "static";
/// The on-device embedding-model strategy. Reserved; available from v0.46.0.
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

/// Map a requested strategy that is known-but-not-yet-implemented to the right
/// error, or an unknown strategy to an invalid-request error.
fn unavailable_or_invalid(strategy: &str) -> InterpretationError {
    if strategy == EMBEDDING_STRATEGY {
        InterpretationError::Unavailable(
            "embedding strategy is not available until the embedding-model milestone (v0.46.0)"
                .to_string(),
        )
    } else {
        InterpretationError::InvalidRequest(format!("unknown interpretation strategy: {strategy}"))
    }
}

/// Build the [`Classifier`] for the given strategy. The `static` strategy returns
/// a rule classifier configured with `rules` (the taxonomy is supplied by the
/// consumer, e.g. the ESPI `signal_categories` rules).
pub fn build_classifier(
    strategy: &str,
    rules: Vec<CategoryRule>,
) -> Result<Box<dyn Classifier>, InterpretationError> {
    match strategy {
        STATIC_STRATEGY => Ok(Box::new(RuleClassifier::new(rules))),
        other => Err(unavailable_or_invalid(other)),
    }
}

/// Build the [`SimilarityProvider`] for the given strategy. The `static` strategy
/// returns the lexical (token-Jaccard) baseline.
pub fn build_similarity(
    strategy: &str,
) -> Result<Box<dyn SimilarityProvider>, InterpretationError> {
    match strategy {
        STATIC_STRATEGY => Ok(Box::new(LexicalSimilarity::new())),
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
