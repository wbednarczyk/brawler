import { callCommand } from "./tauri";
import type {
  EvidenceLink,
  EvidenceLinkListInput,
  NewResearchQuestion,
  NewEvidenceLink,
  ResearchEvidenceInput,
  ResearchQuestion,
  ResearchQuestionListInput,
  ResearchQuestionUpdate,
  ResearchTimelineResult,
  ResearchReviewCheckpoint,
  ResearchReviewCheckpointInput,
} from "./researchTypes";

export function listResearchEvidence(input: ResearchEvidenceInput) {
  return callCommand<ResearchTimelineResult>("list_research_evidence", { input });
}

export function listCompanyTimeline(companyId: string) {
  return callCommand<ResearchTimelineResult>("list_company_timeline", { companyId });
}

export function listWatchlistTimeline(watchlistId: string) {
  return callCommand<ResearchTimelineResult>("list_watchlist_timeline", { watchlistId });
}

export function markResearchScopeReviewed(input: ResearchReviewCheckpointInput) {
  return callCommand<ResearchReviewCheckpoint>("mark_research_scope_reviewed", { input });
}

export function listResearchReviewState(input: ResearchReviewCheckpointInput) {
  return callCommand<ResearchReviewCheckpoint | null>("list_research_review_state", { input });
}

export function listResearchQuestions(input: ResearchQuestionListInput) {
  return callCommand<ResearchQuestion[]>("list_research_questions", { input });
}

export function createResearchQuestion(input: NewResearchQuestion) {
  return callCommand<ResearchQuestion>("create_research_question", { input });
}

export function updateResearchQuestion(input: ResearchQuestionUpdate) {
  return callCommand<ResearchQuestion>("update_research_question", { input });
}

export function createEvidenceLink(input: NewEvidenceLink) {
  return callCommand<EvidenceLink>("create_evidence_link", { input });
}

export function listEvidenceLinks(input: EvidenceLinkListInput) {
  return callCommand<EvidenceLink[]>("list_evidence_links", { input });
}

export function deleteEvidenceLink(id: string) {
  return callCommand<void>("delete_evidence_link", { id });
}
