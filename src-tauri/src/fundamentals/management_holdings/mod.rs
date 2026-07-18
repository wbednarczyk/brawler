//! Management-holdings section extraction (v0.57 T5, ADR 0083 Decision 6, card
//! `9730f5f`).
//!
//! Pure, deterministic, no-network parsing of the mandatory "Zestawienie stanu
//! posiadania akcji … przez osoby zarządzające i nadzorujące" section out of an
//! already-stored periodic report, over the ordered text
//! [`Section`](crate::report_diff::extraction::Section)s produced by the
//! report-diff extractor (ADR 0052). Accuracy is measured by the `#[ignore]`
//! real-data harness (`storage::tests::real_data_management_holdings`) against
//! the 15-document hand-labeled ground truth (docs/testing.md real-data-first).
//!
//! **Design mirrors the ownership shareholders parser**
//! ([`super::ownership::parse_shareholders`]): a *deflated* (whitespace-removed,
//! lowercased) substrate for keyword matching (real text layers break words with
//! arbitrary spaces), a glyph guard FIRST (custom-font digits mapped into the
//! Unicode private-use area → [`MgmtHoldingsState::GlyphEncoded`] residual), and
//! one unified line-stream row assembler over every table shape the ground truth
//! taught (cell-stream one-value-per-line, positional multi-date-column,
//! role-inline, word-per-line, narrative-with-%, in-table organ subheaders).
//!
//! **What sets this apart from the ownership parser.** Rows are *natural persons*,
//! not entities: anchoring requires a holdings token co-occurring with an organ
//! phrase (board-composition / remuneration / diversity-policy sections are known
//! false anchors), and rows are keyed off a person-name detector rather than an
//! entity-name accumulator. Prose "nie posiadają akcji" statements are recorded as
//! an explicit [`MgmtHoldingsState::ZeroHoldingAggregate`] zero picture (zero
//! skin-in-the-game is signal). `pośrednio poprzez <vehicle>` / family-foundation
//! phrasing is captured into `indirect_via` — the founder-badge join bridge.
//!
//! **Conservative shares (never guess).** A person named in the table always gets
//! a row; `shares` is emitted only when the row's numeric cells carry exactly one
//! distinct readable value (all-equal multi-date columns, or a single-value
//! table). A genuinely ambiguous multi-column row (columns disagree and the
//! as-of column cannot be mapped deterministically) yields `shares = None`
//! (stated-but-unreadable) rather than a guessed figure — the STOP-on-guess rule
//! (plan v0.57 T5 tripwire). A dash / `nd.` / `nie dotyczy` cell is treated as
//! absent, never as `0`.

mod parse;

#[cfg(test)]
mod tests;

/// Exposed for the real-data corpus junk-rate guard (F-A2): the same person-
/// plausibility predicate the parser applies at emission, so the harness can
/// assert post-hoc that ZERO emitted rows across the whole live corpus are junk.
#[cfg(test)]
pub(crate) use parse::is_implausible_person;
pub use parse::{
    parse_management_holdings, MgmtHoldingRow, MgmtHoldingsOutcome, MgmtHoldingsState, MgmtRole,
    ZeroOrgan,
};
