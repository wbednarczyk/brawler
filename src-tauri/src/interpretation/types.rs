//! Shared request/result types and the error type for the interpretative layer
//! capability contracts (ADR 0035). These types are deliberately
//! implementation-neutral: a static (rule-based) implementation and a future
//! model-backed implementation of the same capability return the same shapes.

use thiserror::Error;

/// Error returned by any interpretative-layer capability implementation.
#[derive(Debug, Error)]
pub enum InterpretationError {
    /// The backing implementation is not available (e.g. an optional model
    /// implementation is disabled or its resources are missing). Callers should
    /// be able to fall back to the static baseline for the same capability.
    #[error("interpretation backend unavailable: {0}")]
    Unavailable(String),
    /// The request was malformed or empty (e.g. blank text to classify).
    #[error("invalid interpretation request: {0}")]
    InvalidRequest(String),
    /// The backing implementation failed while producing a result.
    #[error("interpretation backend error: {0}")]
    Backend(String),
}

/// A request to classify a single piece of text into one category.
///
/// The taxonomy/rules a classifier uses are configured into the implementation
/// (e.g. the ESPI `signal_categories` registry); this request carries only the
/// text and an optional constraint to a subset of category keys.
#[derive(Debug, Clone)]
pub struct ClassificationRequest {
    pub text: String,
    /// Optional subset of category keys to consider. Empty means "the
    /// implementation's full taxonomy".
    pub candidate_categories: Vec<String>,
}

/// The outcome of a classification.
#[derive(Debug, Clone)]
pub struct Classification {
    /// The chosen category key, or `None` for the **unknown** outcome — the text
    /// could not be confidently classified. Unknown is a first-class result: an
    /// implementation must never guess a wrong category in place of `None`.
    pub category: Option<String>,
    /// Confidence in `0.0..=1.0` for the chosen category (`0.0` when unknown).
    pub confidence: f32,
    /// Optional short, implementation-provided rationale (e.g. the matched rule).
    pub rationale: Option<String>,
}
