import { invoke } from "@tauri-apps/api/core";
import type { TaggedFactCoverageCounts } from "./generated/TaggedFactCoverageCounts";
import type { UncrosswalkedConceptRow } from "./generated/UncrosswalkedConceptRow";
import type { PromotedConcept } from "./generated/PromotedConcept";

export type { TaggedFactCoverageCounts } from "./generated/TaggedFactCoverageCounts";
export type { UncrosswalkedConceptRow } from "./generated/UncrosswalkedConceptRow";
export type { PromotedConcept } from "./generated/PromotedConcept";

// The Coverage panel's compact "what did the program read from the report"
// line (ADR 0100, epic #398): everything Layer 1 captured for a company,
// split into what reached Fundamentals and the reasons the rest did not.
export async function getReportTaggedFactCoverage(
  companyId: string,
): Promise<TaggedFactCoverageCounts> {
  return invoke("get_report_tagged_fact_coverage", { companyId });
}

// "Positions the program doesn't know yet" (ADR 0100 decision 10): captured
// concepts with no crosswalk entry, at this company, ranked by how many
// companies across the corpus report them.
export async function listUncrosswalkedConcepts(
  companyId: string,
): Promise<UncrosswalkedConceptRow[]> {
  return invoke("list_uncrosswalked_concepts", { companyId });
}

// "Show in Fundamentals" (ADR 0100 decision 10) — the owner's own authority
// to promote a captured position into that company's Fundamentals. Never
// reachable by an agent (MCP registry: excluded).
export async function promoteUncrosswalkedConcept(
  companyId: string,
  conceptLocalName: string,
): Promise<PromotedConcept> {
  return invoke("promote_uncrosswalked_concept", { companyId, conceptLocalName });
}
