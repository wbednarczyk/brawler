//! Tier-4 OCR-markdown extraction substrate (ADR 0077 §4).
//!
//! The last-resort layer reads a report through Mistral OCR (provider side,
//! `providers::analysis::mistral`) and parses the stitched markdown
//! deterministically through a confirmed per-company [`OcrExtractionProfile`].
//! This module owns only the pure model + parser; the pipeline/job hook and the
//! tier-4 caller gate land in T4.2/T4.3.

pub mod parser;
pub mod profile;

pub use parser::parse_ocr_markdown;
pub use profile::{OcrExtractionProfile, ValueColumnLayout};
