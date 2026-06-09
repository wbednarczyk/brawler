import { callCommand } from "./tauri";
import type {
  EvidenceLink,
  NewEvidenceLink,
  ResearchEvidenceInput,
  ResearchEvidenceItem,
  ResearchReviewCheckpoint,
  ResearchReviewCheckpointInput,
} from "./researchTypes";

export function listResearchEvidence(input: ResearchEvidenceInput) {
  return callCommand<ResearchEvidenceItem[]>("list_research_evidence", { input });
}

export function listCompanyTimeline(companyId: string) {
  return callCommand<ResearchEvidenceItem[]>("list_company_timeline", { companyId });
}

export function listWatchlistTimeline(watchlistId: string) {
  return callCommand<ResearchEvidenceItem[]>("list_watchlist_timeline", { watchlistId });
}

export function markResearchScopeReviewed(input: ResearchReviewCheckpointInput) {
  return callCommand<ResearchReviewCheckpoint>("mark_research_scope_reviewed", { input });
}

export function listResearchReviewState(input: ResearchReviewCheckpointInput) {
  return callCommand<ResearchReviewCheckpoint | null>("list_research_review_state", { input });
}

export function createEvidenceLink(input: NewEvidenceLink) {
  return callCommand<EvidenceLink>("create_evidence_link", { input });
}

export function deleteEvidenceLink(id: string) {
  return callCommand<void>("delete_evidence_link", { id });
}
