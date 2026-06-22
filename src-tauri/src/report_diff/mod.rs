//! Report-over-report diff (v0.47.0, ADR 0052): deterministic, pure-Rust,
//! dual-format extraction of financial-statement report documents into ordered
//! text sections, plus the on-demand deterministic section diff over them.
//!
//! Scope: structured financial statements only (consolidated SSF / standalone
//! JSF). The narrative management report (MD&A) diff and the AI delta summary are
//! deferred (ADR 0052). No AI, no network — fully local and reproducible.

pub mod classify;
pub mod diff;
pub mod extraction;

pub use classify::{classify_statement, period_label, period_sort_key, StatementType};
pub use diff::{diff_sections, SectionDiff, SectionDiffStatus, SectionsDiff};
pub use extraction::{
    extract_report, ExtractionOutcome, ExtractionState, Section, SourceFormat, EXTRACTOR_VERSION,
};
