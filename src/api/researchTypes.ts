// ============================================================================
// Research-workspace API DTOs (ADR 0048)
//
// Every shape below is GENERATED from the Rust source via ts-rs: change the
// struct in src-tauri (src/storage/types.rs, research_briefs.rs,
// research_reminders.rs, research_digests.rs) and run `make types`. The
// string-literal unions are generated from the marker enums in
// src-tauri/src/api_ts_unions.rs. Only the three pure type aliases at the
// bottom are hand-written (they mirror another generated type 1:1).
// ============================================================================

// --- String-literal unions (generated from api_ts_unions.rs) ---
export type { ResearchEvidenceType } from "./generated/ResearchEvidenceType";
export type { ResearchEvidenceSourceDomain } from "./generated/ResearchEvidenceSourceDomain";
export type { ResearchTrustCategory } from "./generated/ResearchTrustCategory";
export type { ResearchReviewScopeType } from "./generated/ResearchReviewScopeType";
export type { EvidenceRelationType } from "./generated/EvidenceRelationType";
export type { ResearchQuestionStatus } from "./generated/ResearchQuestionStatus";
export type { ResearchBriefJobStatus } from "./generated/ResearchBriefJobStatus";
export type { ResearchReminderKind } from "./generated/ResearchReminderKind";
export type { ResearchReminderStatus } from "./generated/ResearchReminderStatus";

// --- Timeline / evidence ---
export type { ResearchEvidenceReviewState } from "./generated/ResearchEvidenceReviewState";
export type { ResearchEvidenceItem } from "./generated/ResearchEvidenceItem";
export type { ResearchEvidenceInput } from "./generated/ResearchEvidenceInput";
export type { ResearchTimelineSummary } from "./generated/ResearchTimelineSummary";
export type { ResearchCompanyTimelineSummary } from "./generated/ResearchCompanyTimelineSummary";
export type { ResearchTimelineResult } from "./generated/ResearchTimelineResult";

// --- Review checkpoints ---
export type { ResearchReviewCheckpointInput } from "./generated/ResearchReviewCheckpointInput";
export type { ResearchReviewCheckpoint } from "./generated/ResearchReviewCheckpoint";

// --- Evidence links ---
export type { NewEvidenceLink } from "./generated/NewEvidenceLink";
export type { EvidenceLink } from "./generated/EvidenceLink";
export type { EvidenceLinkListInput } from "./generated/EvidenceLinkListInput";

// --- Research questions ---
export type { ResearchQuestion } from "./generated/ResearchQuestion";
export type { ResearchQuestionListInput } from "./generated/ResearchQuestionListInput";
export type { NewResearchQuestion } from "./generated/NewResearchQuestion";
export type { ResearchQuestionUpdate } from "./generated/ResearchQuestionUpdate";

// --- AI research briefs ---
export type { ResearchBriefScopeInput } from "./generated/ResearchBriefScopeInput";
export type { ResearchBriefCitation } from "./generated/ResearchBriefCitation";
export type { ResearchBrief } from "./generated/ResearchBrief";
export type { ResearchBriefJob } from "./generated/ResearchBriefJob";

// --- Research reminders ---
export type { ResearchReminder } from "./generated/ResearchReminder";
export type { ResearchReminderListInput } from "./generated/ResearchReminderListInput";
export type { NewResearchReminder } from "./generated/NewResearchReminder";
export type { ResearchReminderUpdate } from "./generated/ResearchReminderUpdate";

// --- Research digests ---
export type { ResearchDigestCitation } from "./generated/ResearchDigestCitation";
export type { ResearchDigest } from "./generated/ResearchDigest";
export type { ResearchDigestJob } from "./generated/ResearchDigestJob";

// --- Hand-written aliases (each mirrors a generated type 1:1) ---
import type { ResearchReviewScopeType } from "./generated/ResearchReviewScopeType";
import type { ResearchBriefScopeInput } from "./generated/ResearchBriefScopeInput";
import type { ResearchBriefJobStatus } from "./generated/ResearchBriefJobStatus";

export type ResearchQuestionScopeType = ResearchReviewScopeType;
export type ResearchDigestScopeInput = ResearchBriefScopeInput;
export type ResearchDigestJobStatus = ResearchBriefJobStatus;
