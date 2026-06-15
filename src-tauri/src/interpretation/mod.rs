//! Local interpretative AI layer (ADR 0035).
//!
//! This is the on-device "interpretative lookup" half of the two-layer AI
//! architecture (the generative half lives in [`crate::providers`]). It exposes
//! a small set of **capability** contracts — [`Classifier`], [`SimilarityProvider`],
//! [`Matcher`], and [`SemanticSearch`] — that feature code depends on. The
//! defining rules of this layer (ADR 0035):
//!
//! - **Consumers bind to capabilities, not to models.** A feature asks for
//!   "classify this" or "what is this similar to", never for "an embedding" or
//!   "a model". Each capability has interchangeable implementations selected at
//!   runtime through a registry/config.
//! - **Static is the shipped baseline.** Deterministic implementations (rules,
//!   FTS5/lexical, fuzzy) are the default. A model-backed implementation is an
//!   optional enhancement layered behind the *same* capability trait, adopted
//!   per capability only where an eval shows it beats static. Removing the model
//!   is switching the active implementation back to static — no consumer change.
//! - **The layer produces only disposable, derived artifacts.** Any index it
//!   builds (e.g. a future vector store) is a cache computed from canonical data
//!   and can be dropped with zero canonical data loss.
//!
//! This module defines the capability **contracts** and shared request/result
//! types only. Implementations (static baselines), the selection registry, the
//! embedding model, and the eval harness are added in later slices of the
//! interpretative-layer epic.

mod capabilities;
mod eval;
mod lexical_similarity;
mod registry;
mod rule_classifier;
mod types;

pub use capabilities::{Classifier, Matcher, SemanticSearch, SimilarityProvider};
pub use eval::{evaluate_classifier, ClassifierEvalReport, LabeledSample};
pub use lexical_similarity::{LexicalSimilarity, LEXICAL_SIMILARITY_ID};
pub use registry::{
    build_classifier, build_similarity, InterpretationSelection, EMBEDDING_STRATEGY,
    STATIC_STRATEGY,
};
pub use rule_classifier::{CategoryRule, RuleClassifier, RULE_CLASSIFIER_ID};
pub use types::{
    Classification, ClassificationRequest, InterpretationError, ScoredItem, SearchScope, TextItem,
};
