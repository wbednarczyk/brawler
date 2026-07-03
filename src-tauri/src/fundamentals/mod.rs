//! Fundamentals domain core (ADR 0046): the metric expression engine, the
//! shared derived-metrics service, and the quality-framework rule engine.
//!
//! Package-by-feature domain core (ADR 0039): no ports here — the engine and
//! metrics service each have one implementation and are reused as plain modules.
//!
//! [`validation`] adds the deterministic "good" gate (ADR 0061): the objective,
//! human-free check that decides whether an extracted fact set may be
//! auto-accepted.

pub mod expr;
pub mod extraction;
pub mod metrics;
pub mod scorecard;
pub mod templates;
pub mod validation;

pub use scorecard::{CriterionOutcome, Verdict, VerdictCounts, ENGINE_VERSION};
