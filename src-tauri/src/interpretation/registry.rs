//! Capability selection registry for the interpretative layer (ADR 0035,
//! section 6; trimmed by ADR 0080).
//!
//! Each capability resolves to an active *strategy*. Since ADR 0080 retired the
//! embedding model, `static` (the deterministic baseline) is the only shipped
//! strategy; the legacy persisted `embedding` value is tolerated at the
//! settings read (mapped to `static`, see `storage::settings`). Consumers build
//! a capability through these factories instead of constructing a concrete
//! implementation, so a future strategy requires no consumer change.

use super::{CategoryRule, Classifier, InterpretationError, RuleClassifier};

/// The deterministic, always-available baseline strategy.
pub const STATIC_STRATEGY: &str = "static";

/// Build the [`Classifier`] for the given strategy. The `static` strategy returns
/// a rule classifier configured with `rules` (the taxonomy is supplied by the
/// consumer, e.g. the ESPI `signal_categories` rules). Any other strategy is
/// unknown (the embedding strategy was retired by ADR 0080).
pub fn build_classifier(
    strategy: &str,
    rules: Vec<CategoryRule>,
) -> Result<Box<dyn Classifier>, InterpretationError> {
    match strategy {
        STATIC_STRATEGY => Ok(Box::new(RuleClassifier::new(rules))),
        other => Err(InterpretationError::InvalidRequest(format!(
            "unknown interpretation strategy: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_static_classifier() {
        assert!(build_classifier(STATIC_STRATEGY, Vec::new()).is_ok());
    }

    #[test]
    fn unknown_strategy_is_invalid() {
        // The retired `embedding` strategy is now just another unknown value at
        // the registry; settings reads map it to `static` before it gets here.
        for strategy in ["embedding", "nonsense"] {
            assert!(matches!(
                build_classifier(strategy, Vec::new()),
                Err(InterpretationError::InvalidRequest(_))
            ));
        }
    }
}
