use super::*;

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub applied_migrations: i64,
    pub companies: i64,
    pub source_adapters: i64,
    pub settings: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    pub id: String,
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: Option<String>,
    pub cik: Option<String>,
    pub lei: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "CreateCompanyInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewCompany {
    pub exchange: String,
    pub ticker: String,
    pub display_name: String,
    pub isin: Option<String>,
    pub cik: Option<String>,
    pub lei: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct Watchlist {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub company_count: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistMembership {
    pub watchlist_id: String,
    pub watchlist_name: String,
    pub company_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "CreateWatchlistInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewWatchlist {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "RenameWatchlistInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistUpdate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "WatchlistMembershipInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistCompanyInput {
    pub watchlist_id: String,
    pub company_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    pub id: String,
    pub company: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub source: String,
    pub time: String,
    pub title: String,
    pub unread: bool,
    pub saved: bool,
    pub source_url: String,
    pub language: String,
    pub published_at: String,
    pub fetched_at: String,
    pub attribution: String,
    pub summary: String,
    pub body_text: String,
    pub attachments: Vec<FeedItemAttachment>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FeedItemAttachment {
    pub id: String,
    pub label: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "UpdateFeedItemStateInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct FeedItemStateInput {
    pub id: String,
    #[cfg_attr(feature = "ts-export", ts(type = "boolean"))]
    pub read: Option<bool>,
    #[cfg_attr(feature = "ts-export", ts(type = "boolean"))]
    pub saved: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SourceIngestionResult {
    pub adapter_id: String,
    pub items_fetched: usize,
    pub items_created: usize,
    pub items_matched: usize,
    pub items_unmatched: usize,
    pub detail_items_attempted: usize,
    pub detail_items_stored: usize,
    pub detail_items_failed: usize,
    pub fetched_at: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FeedPruneResult {
    pub retention_days: i64,
    pub items_deleted: usize,
    pub pruned_at: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FeedDeleteResult {
    pub items_deleted: usize,
    pub deleted_at: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchedSourceItem {
    pub id: String,
    pub adapter_id: String,
    pub company_name: String,
    pub title: String,
    pub source_url: String,
    pub published_at: String,
    pub fetched_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchEvidenceInput {
    pub company_id: Option<String>,
    pub watchlist_id: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<Vec<crate::api_ts_unions::ResearchEvidenceType>>")
    )]
    pub evidence_types: Option<Vec<String>>,
    pub changed_since_review_only: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchTimelineResult {
    pub items: Vec<ResearchEvidenceItem>,
    pub summary: ResearchTimelineSummary,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchTimelineSummary {
    pub total: usize,
    pub changed_since_review: usize,
    pub last_reviewed_at: Option<String>,
    pub member_company_count: usize,
    pub companies_with_changed_evidence: usize,
    pub company_summaries: Vec<ResearchCompanyTimelineSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchCompanyTimelineSummary {
    pub company_id: String,
    pub total: usize,
    pub changed_since_review: usize,
    pub last_reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchEvidenceItem {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub evidence_type: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceSourceDomain")
    )]
    pub source_domain: String,
    pub source_id: String,
    pub company_id: String,
    pub occurred_at: String,
    pub title: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub attribution: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchTrustCategory")
    )]
    pub trust_category: String,
    pub review_state: ResearchEvidenceReviewState,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchEvidenceReviewState {
    pub changed_since_company_review: bool,
    pub changed_since_watchlist_review: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReviewCheckpointInput {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub reviewed_at: Option<String>,
    pub cascade_to_companies: Option<bool>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReviewCheckpoint {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub reviewed_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct NewEvidenceLink {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub from_type: String,
    pub from_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub to_type: String,
    pub to_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::EvidenceRelationType")
    )]
    pub relation_type: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLink {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub from_type: String,
    pub from_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub to_type: String,
    pub to_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::EvidenceRelationType")
    )]
    pub relation_type: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchQuestionListInput {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchReviewScopeType>")
    )]
    pub scope_type: Option<String>,
    pub scope_id: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchQuestionStatus>")
    )]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewResearchQuestion {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub title: String,
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchQuestionUpdate {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchQuestionStatus>")
    )]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchQuestion {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub title: String,
    pub body: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchQuestionStatus")
    )]
    pub status: String,
    pub closed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLinkListInput {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub endpoint_type: String,
    pub endpoint_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct NotebookEntry {
    pub id: String,
    pub company_id: String,
    pub title: String,
    pub body: String,
    pub body_format: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub origins: Vec<NotebookOrigin>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct NotebookOrigin {
    pub id: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "CreateNotebookEntryInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewNotebookEntry {
    pub company_id: String,
    pub title: String,
    pub body: String,
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub body_format: Option<String>,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
    pub origins: Vec<NewNotebookOrigin>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "UpdateNotebookEntryInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NotebookEntryUpdate {
    pub id: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct NewNotebookOrigin {
    pub source_type: String,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SourceAdapter {
    pub id: String,
    pub display_name: String,
    pub source_type: String,
    pub fetch_mode: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"required\" | \"optional\" | \"developer\"")
    )]
    pub visibility: String,
    /// Feed-pipeline role (ADR 0069 D2, plan v0.55 T3). `witness` sources
    /// reconcile against the primary instead of ingesting into the feed.
    #[cfg_attr(feature = "ts-export", ts(type = "\"primary\" | \"witness\""))]
    pub role: String,
    pub user_configurable: bool,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"healthy\" | \"attention\" | \"notRefreshed\" | \"off\"")
    )]
    pub health_status: String,
    pub enabled: bool,
    pub default_poll_interval_seconds: i64,
    pub source_url: String,
    pub rate_limit_policy: String,
    pub policy_note: String,
    pub last_attempt_at: Option<String>,
    pub last_trigger: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error_at: Option<String>,
    pub last_error: Option<String>,
    pub last_items_fetched: Option<i64>,
    pub last_items_created: Option<i64>,
    pub last_items_matched: Option<i64>,
    pub last_items_unmatched: Option<i64>,
    pub last_detail_items_attempted: Option<i64>,
    pub last_detail_items_stored: Option<i64>,
    pub last_detail_items_failed: Option<i64>,
    pub last_detail_warning: Option<String>,
    pub markets: Vec<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyRegistryEntry {
    pub source_adapter_id: String,
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: Option<String>,
    pub source_url: String,
    pub fetched_at: String,
    pub tracked: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "LookupCompanyInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLookupInput {
    pub exchange: String,
    pub ticker: Option<String>,
    pub display_name: Option<String>,
    pub isin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLookupResult {
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: String,
    pub source: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyRegistryRefreshResult {
    pub adapter_id: String,
    pub entries_fetched: usize,
    pub entries_upserted: usize,
    pub entries_deactivated: usize,
    pub fetched_at: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyEvent {
    pub id: String,
    pub company_id: String,
    pub company: String,
    pub company_name: String,
    pub event_type: String,
    pub title: String,
    pub event_date: String,
    pub event_time: Option<String>,
    pub status: String,
    pub source_type: String,
    pub source_adapter_id: Option<String>,
    pub source_event_key: Option<String>,
    pub source_url: Option<String>,
    pub attribution: Option<String>,
    pub fetched_at: Option<String>,
    pub manual: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "ListCompanyEventsInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyEventListInput {
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub mode: Option<String>,
    pub company_id: Option<String>,
    pub watchlist_id: Option<String>,
    pub event_type: Option<String>,
    pub status: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// A typed ESPI/EBI classification (ADR 0034). Canonical output of
/// classification, separate from the raw `feed_item` and any derived event.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanySignal {
    pub id: String,
    pub company_id: String,
    pub company: String,
    pub company_name: String,
    pub feed_item_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "\"insider_transaction\" | \"dividend\" | \"profit_warning\" | \"significant_contract\" | \"own_shares\" | \"guidance_change\" | \"general_meeting\" | \"auditor_opinion\" | \"short_position_change\" | \"other\""
        )
    )]
    pub category: String,
    pub category_display_name: String,
    pub confidence: f64,
    #[cfg_attr(feature = "ts-export", ts(type = "\"rule\" | \"ai\""))]
    pub classified_by: String,
    #[cfg_attr(feature = "ts-export", ts(type = "\"confirmed\" | \"proposed\""))]
    pub status: String,
    pub signal_date: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub derived_event_id: Option<String>,
    pub title: String,
    pub source_url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "ListCompanySignalsInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct CompanySignalListInput {
    pub company_id: Option<String>,
    pub watchlist_id: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanySignalActionInput {
    pub id: String,
}

/// One seeded signal category exposed to the AI fallback prompt (key + label).
#[derive(Debug, Clone)]
pub struct SignalCategorySummary {
    pub key: String,
    pub display_name: String,
}

/// An official-report filing that has no signal yet — a candidate for the AI
/// fallback. `body_text` may be empty when only the title is available.
#[derive(Debug, Clone)]
pub struct UnclassifiedFiling {
    pub feed_item_id: String,
    pub company_id: String,
    pub title: String,
    pub body_text: String,
    pub signal_date: Option<String>,
}

/// Internal input for persisting an AI-proposed signal (used by the AI fallback
/// job). Not a command DTO; the AI job builds it from the classification result.
#[derive(Debug, Clone)]
pub struct ProposedSignalInput {
    pub feed_item_id: String,
    pub company_id: String,
    pub category: String,
    pub confidence: f64,
    pub signal_date: Option<String>,
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "CreateCompanyEventInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewCompanyEvent {
    pub company_id: String,
    pub event_type: String,
    pub title: String,
    pub event_date: String,
    pub event_time: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub status: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "string"))]
    pub source_type: Option<String>,
    pub source_adapter_id: Option<String>,
    pub source_event_key: Option<String>,
    pub source_url: Option<String>,
    pub attribution: Option<String>,
    pub fetched_at: Option<String>,
}
