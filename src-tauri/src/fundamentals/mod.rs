//! Fundamentals domain core (ADR 0046): the metric expression engine, the
//! shared derived-metrics service, and the quality-framework rule engine.
//!
//! Package-by-feature domain core (ADR 0039): no ports here — the engine and
//! metrics service each have one implementation and are reused as plain modules.

pub mod expr;
pub mod metrics;
pub mod scorecard;
pub mod templates;

pub use scorecard::{CriterionOutcome, Verdict, VerdictCounts, ENGINE_VERSION};
