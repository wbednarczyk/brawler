//! Ownership-structure extraction prototype (v0.56 T1, ADR 0072).
//!
//! Pure, deterministic, no-network parsing of the mandatory "shareholders
//! holding ≥5%" table out of an already-stored periodic report, over the
//! ordered text [`Section`](crate::report_diff::extraction::Section)s produced
//! by the report-diff extractor (ADR 0052). This is the T1 spike: its accuracy
//! is measured by the `#[ignore]` real-data harness
//! (`storage::tests::real_data_ownership`) against a hand-labeled ground-truth
//! set before the milestone commits to a parse tier split (ADR 0072
//! consequence; docs/testing.md real-data-first rule).
//!
//! **Where heading matching lives (design note).** The shareholders heading is
//! matched *module-locally* by scanning `Section` lines here — the report-diff
//! `split_sections`/`known_heading_regex` is left untouched. Two reasons: (1)
//! extending `known_heading_regex` would reflow existing sections and risk the
//! `report_diff` goldens; (2) several real headings ("Struktura akcjonariatu",
//! "Znaczne pakiety akcji") are mixed-case and are *not* detected as split
//! points anyway, so they only survive as body lines — a line scan finds them
//! where a heading-title match would miss them.
//!
//! **Input is text, not HTML (design note).** The parser consumes `Section`s,
//! which carry text only. For ESEF `.xhtml` the report-diff `html_to_text`
//! runs first and emits one table cell per line (each `</td>`/`</th>` becomes a
//! newline), so the parser reconstructs rows from that linearized cell stream
//! rather than re-parsing HTML. This keeps a single deterministic text entry
//! for both formats and matches how the harness feeds it
//! (`extract_report` → `parse_shareholders`). Native HTML-table parsing
//! (colspan, empty-cell alignment) is a T3 upgrade path.

pub mod classify;
pub mod espi_notification;
mod parse;

#[cfg(test)]
mod tests;

pub use espi_notification::{
    parse_major_holding_notification, EspiHoldingParse, ParsedEspiHolding,
};
pub use parse::{
    parse_shareholders, AggregateRow, OwnershipParseOutcome, OwnershipParseState, OwnershipRow,
};
