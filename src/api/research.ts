import { callCommand } from "./tauri";
import type {
  EvidenceLink,
  EvidenceLinkListInput,
  NewResearchQuestion,
  NewEvidenceLink,
  ResearchEvidenceInput,
  NewResearchReminder,
  ResearchBriefJob,
  ResearchBriefScopeInput,
  ResearchDigestJob,
  ResearchDigestScopeInput,
  ResearchQuestion,
  ResearchQuestionListInput,
  ResearchQuestionUpdate,
  ResearchReminder,
  ResearchReminderListInput,
  ResearchReminderUpdate,
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

export function deleteResearchQuestion(id: string) {
  return callCommand<void>("delete_research_question", { id });
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

export function startResearchBrief(input: ResearchBriefScopeInput) {
  return callCommand<ResearchBriefJob>("start_research_brief", { input });
}

export function listResearchBriefs(input: ResearchBriefScopeInput) {
  return callCommand<ResearchBriefJob[]>("list_research_briefs", { input });
}

export function listResearchReminders(input: ResearchReminderListInput) {
  return callCommand<ResearchReminder[]>("list_research_reminders", { input });
}

export function createResearchReminder(input: NewResearchReminder) {
  return callCommand<ResearchReminder>("create_research_reminder", { input });
}

export function updateResearchReminder(input: ResearchReminderUpdate) {
  return callCommand<ResearchReminder>("update_research_reminder", { input });
}

export function deleteResearchReminder(id: string) {
  return callCommand<void>("delete_research_reminder", { id });
}

export function startResearchDigest(input: ResearchDigestScopeInput) {
  return callCommand<ResearchDigestJob>("start_research_digest", { input });
}

export function listResearchDigests(input: ResearchDigestScopeInput) {
  return callCommand<ResearchDigestJob[]>("list_research_digests", { input });
}
