//! TypeScript marker enums for API string-literal unions (ADR 0048).
//!
//! Several DTO fields are persisted as `String` in Rust but exposed to the
//! frontend as closed string-literal unions (the values are pinned in `const`
//! arrays / SQL `CASE`s elsewhere in the crate). To keep the generated
//! `src/api/generated/*` bindings faithful to that contract — named unions, not
//! a widened `string` — each such union is declared here once as an enum whose
//! `#[serde(rename_all = ...)]` reproduces the exact wire values, and the
//! `String` field references it with `#[ts(as = "crate::api_ts_unions::Foo")]`.
//!
//! This module exists ONLY for type generation: it is compiled behind the
//! `ts-export` feature and never used in runtime logic. Adding/removing a wire
//! value means editing the enum here (the single source for the union) and
//! rerunning `make types`. The variant set mirrors the FRONTEND contract (which
//! may include values the Rust write-path does not yet emit, e.g. `cancelled`).

use serde::Serialize;
use ts_rs::TS;

macro_rules! ts_union {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Serialize, TS)]
        #[ts(export, export_to = "../../src/api/generated/")]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),*
        }
    };
}

ts_union!(ResearchEvidenceType {
    FeedItem,
    NotebookEntry,
    Claim,
    TranscriptSegment,
    CompanyEvent,
    AiAnalysis,
    ResearchQuestion,
    CompanySignal,
    DecisionEntry,
    Reminder,
    AiBrief,
    Digest,
});

ts_union!(ResearchEvidenceSourceDomain {
    Feed,
    Notebooks,
    Transcripts,
    Events,
    AiAnalysis,
    Research,
});

ts_union!(ResearchTrustCategory {
    OfficialReport,
    CompanyPublication,
    PublicMedia,
    MarketCalendar,
    Transcript,
    UserNote,
    AiGenerated,
    Unknown,
});

ts_union!(ResearchReviewScopeType { Company, Watchlist });

ts_union!(EvidenceRelationType {
    OriginatesFrom,
    Cites,
    Supports,
    Contradicts,
    Updates,
    FollowsUp,
    Answers,
    Related,
});

ts_union!(ResearchQuestionStatus {
    Open,
    Answered,
    Closed,
});

ts_union!(ResearchBriefJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
});

ts_union!(ResearchReminderKind {
    ClaimFollowUp,
    EventReview,
    QuestionReview,
    ManualResearch,
    DigestReview,
    SignalReview,
});

ts_union!(ResearchReminderStatus {
    Open,
    Completed,
    Dismissed,
});

// --- Quality frameworks (ADR 0046) ---

// Adds `insufficient_evidence` (ADR 0075): the agent-assessed qualitative
// verdict for "not enough app-held evidence to judge", distinct from the
// quantitative `unavailable` ("metric could not be computed"). Stored as a raw
// `String` on the agent rows, so the union is the only thing pinning the value.
ts_union!(CriterionVerdict {
    Pass,
    Partial,
    Fail,
    Unavailable,
    InsufficientEvidence,
});

// --- Management claims (ADR 0040) ---

ts_union!(ClaimStatus {
    Pending,
    Delivered,
    PartiallyDelivered,
    Missed,
    Revised,
});

ts_union!(ClaimSourceEvidenceType {
    ReportDocument,
    TranscriptSegment,
    FeedItem,
    Manual,
});

ts_union!(ClaimTargetComparator {
    Gte,
    Lte,
    Gt,
    Lt,
    Approx,
    Eq,
});

// --- Report season (ADR 0041) ---

ts_union!(ReportPreparationStatus {
    Upcoming,
    Prepared,
    Processed,
});

// --- Search (ADR 0032) ---

ts_union!(SearchContentType {
    Company,
    Watchlist,
    FeedItem,
    NotebookEntry,
    TranscriptSegment,
    Event,
    ResearchBrief,
    Digest,
});

// --- Claim extraction (ADR 0040) ---

ts_union!(ClaimExtractionSourceType {
    ReportDocument,
    Transcript,
});
