//! Local interpretative AI layer (ADR 0035, trimmed by ADR 0080).
//!
//! This is the on-device "interpretative lookup" half of the two-layer AI
//! architecture (the generative half lives in [`crate::providers`]). It exposes
//! the [`Classifier`] capability contract that feature code depends on. The
//! defining rules of this layer (ADR 0035):
//!
//! - **Consumers bind to capabilities, not to models.** A feature asks for
//!   "classify this", never for a concrete implementation. Implementations are
//!   selected at runtime through the registry.
//! - **Static is the shipped baseline.** Deterministic implementations (rules)
//!   are the default and the only shipped strategy (ADR 0080). A future
//!   model-backed implementation re-enters behind the same capability trait
//!   through a fresh eval-gated ADR.
//!
//! The embedding model, the vector index, and the similarity/matcher/search
//! capabilities were removed by ADR 0080: no per-capability eval ever beat the
//! static baseline, and the only reachable consumer was a developer diagnostic.

mod capabilities;
mod registry;
mod rule_classifier;
mod types;

pub use capabilities::Classifier;
pub use registry::{build_classifier, STATIC_STRATEGY};
pub use rule_classifier::{CategoryRule, RuleClassifier, RULE_CLASSIFIER_ID};
pub use types::{Classification, ClassificationRequest, InterpretationError};
