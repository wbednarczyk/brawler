export type ResearchEvidenceType =
  | "feed_item"
  | "notebook_entry"
  | "claim"
  | "transcript_segment"
  | "company_event"
  | "ai_analysis"
  | "research_question"
  | "company_signal"
  | "reminder"
  | "ai_brief"
  | "digest";

export type ResearchEvidenceSourceDomain =
  | "feed"
  | "notebooks"
  | "transcripts"
  | "events"
  | "ai_analysis"
  | "research";

export type ResearchTrustCategory =
  | "official_report"
  | "company_publication"
  | "public_media"
  | "market_calendar"
  | "transcript"
  | "user_note"
  | "ai_generated"
  | "unknown";

export type ResearchEvidenceReviewState = {
  changedSinceCompanyReview: boolean;
  changedSinceWatchlistReview: boolean;
};

export type ResearchEvidenceItem = {
  id: string;
  evidenceType: ResearchEvidenceType;
  sourceDomain: ResearchEvidenceSourceDomain;
  sourceId: string;
  companyId: string;
  occurredAt: string;
  title: string;
  summary: string | null;
  sourceUrl: string | null;
  attribution: string | null;
  trustCategory: ResearchTrustCategory;
  reviewState: ResearchEvidenceReviewState;
};

export type ResearchEvidenceInput = {
  companyId?: string | null;
  watchlistId?: string | null;
  evidenceTypes?: ResearchEvidenceType[] | null;
  changedSinceReviewOnly?: boolean | null;
  limit?: number | null;
};

export type ResearchTimelineSummary = {
  total: number;
  changedSinceReview: number;
  lastReviewedAt: string | null;
  memberCompanyCount: number;
  companiesWithChangedEvidence: number;
  companySummaries: ResearchCompanyTimelineSummary[];
};

export type ResearchCompanyTimelineSummary = {
  companyId: string;
  total: number;
  changedSinceReview: number;
  lastReviewedAt: string | null;
};

export type ResearchTimelineResult = {
  items: ResearchEvidenceItem[];
  summary: ResearchTimelineSummary;
};

export type ResearchReviewScopeType = "company" | "watchlist";

export type ResearchReviewCheckpointInput = {
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  reviewedAt?: string | null;
  cascadeToCompanies?: boolean | null;
};

export type ResearchReviewCheckpoint = {
  id: string;
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  reviewedAt: string;
  createdAt: string;
  updatedAt: string;
};

export type EvidenceRelationType =
  | "originates_from"
  | "cites"
  | "supports"
  | "contradicts"
  | "updates"
  | "follows_up"
  | "answers"
  | "related";

export type NewEvidenceLink = {
  fromType: ResearchEvidenceType;
  fromId: string;
  toType: ResearchEvidenceType;
  toId: string;
  relationType: EvidenceRelationType;
};

export type EvidenceLink = NewEvidenceLink & {
  id: string;
  createdAt: string;
};

export type ResearchQuestionStatus = "open" | "answered" | "closed";
export type ResearchQuestionScopeType = "company" | "watchlist";

export type ResearchQuestion = {
  id: string;
  scopeType: ResearchQuestionScopeType;
  scopeId: string;
  title: string;
  body: string;
  status: ResearchQuestionStatus;
  closedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ResearchQuestionListInput = {
  scopeType?: ResearchQuestionScopeType | null;
  scopeId?: string | null;
  status?: ResearchQuestionStatus | null;
};

export type NewResearchQuestion = {
  scopeType: ResearchQuestionScopeType;
  scopeId: string;
  title: string;
  body?: string | null;
};

export type ResearchQuestionUpdate = {
  id: string;
  title?: string | null;
  body?: string | null;
  status?: ResearchQuestionStatus | null;
};

export type EvidenceLinkListInput = {
  endpointType: ResearchEvidenceType;
  endpointId: string;
};

export type ResearchBriefScopeInput = {
  scopeType: ResearchReviewScopeType;
  scopeId: string;
};

export type ResearchBriefJobStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

export type ResearchBriefCitation = {
  id: string;
  briefId: string;
  citationKey: string;
  evidenceType: ResearchEvidenceType;
  evidenceId: string;
  label: string;
  snippet: string | null;
  createdAt: string;
};

export type ResearchBrief = {
  id: string;
  jobId: string;
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  providerId: string;
  model: string;
  promptVersion: string;
  evidenceCollectorVersion: string;
  rendererVersion: string;
  title: string;
  summary: string;
  contentMarkdown: string;
  language: string | null;
  generatedAt: string;
  createdAt: string;
  citations: ResearchBriefCitation[];
};

export type ResearchBriefJob = {
  id: string;
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  providerId: string;
  model: string;
  promptVersion: string;
  evidenceCollectorVersion: string;
  rendererVersion: string;
  status: ResearchBriefJobStatus;
  errorCode: string | null;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  brief: ResearchBrief | null;
};

export type ResearchReminderKind =
  | "claim_follow_up"
  | "event_review"
  | "question_review"
  | "manual_research"
  | "digest_review"
  | "signal_review";

export type ResearchReminderStatus = "open" | "completed" | "dismissed";

export type ResearchReminder = {
  id: string;
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  companyId: string | null;
  reminderKind: ResearchReminderKind;
  sourceType: ResearchEvidenceType | null;
  sourceId: string | null;
  title: string;
  body: string;
  dueAt: string | null;
  status: ResearchReminderStatus;
  snoozedUntil: string | null;
  completedAt: string | null;
  dismissedAt: string | null;
  createdAt: string;
  updatedAt: string;
};

export type ResearchReminderListInput = {
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  status?: ResearchReminderStatus | null;
};

export type NewResearchReminder = {
  scopeType: ResearchReviewScopeType;
  scopeId: string;
  companyId?: string | null;
  reminderKind: ResearchReminderKind;
  sourceType?: ResearchEvidenceType | null;
  sourceId?: string | null;
  title: string;
  body?: string | null;
  dueAt?: string | null;
};

export type ResearchReminderUpdate = {
  id: string;
  status?: ResearchReminderStatus | null;
  dueAt?: string | null;
  snoozedUntil?: string | null;
};

export type ResearchDigestScopeInput = ResearchBriefScopeInput;
export type ResearchDigestJobStatus = ResearchBriefJobStatus;
export type ResearchDigestCitation = {
  id: string;
  digestId: string;
  citationKey: string;
  evidenceType: ResearchEvidenceType;
  evidenceId: string;
  label: string;
  snippet: string | null;
  createdAt: string;
};
export type ResearchDigest = Omit<ResearchBrief, "citations"> & {
  citations: ResearchDigestCitation[];
};
export type ResearchDigestJob = Omit<ResearchBriefJob, "brief"> & {
  digest: ResearchDigest | null;
};
