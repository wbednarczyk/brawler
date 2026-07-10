// ============================================================================
// Fundamentals-coverage DTOs — GENERATED from the Rust source via ts-rs (ADR 0048,
// ADR 0077 §2). The coverage read model unions a company's periods across three
// axes: the canonical report, extracted facts, and the review queue.
//
// Do not hand-edit these shapes: change the Rust struct (commands/fundamentals_coverage.rs)
// and run `make types`. `make types-check` fails if the committed bindings drift
// from the Rust source. This module is the stable import path; consumers import
// from here, not from ./generated directly.
// ============================================================================

export type { FundamentalsCoverage } from "./generated/FundamentalsCoverage";
export type { CoveragePeriodRow } from "./generated/CoveragePeriodRow";
export type { CoverageReportCell } from "./generated/CoverageReportCell";
export type { CoverageFactsCell } from "./generated/CoverageFactsCell";
export type { CoverageReviewCell } from "./generated/CoverageReviewCell";
