use super::*;

pub(super) const RESEARCH_SCHEMA_VERSION: i64 = 1;
pub(super) const SETTINGS_SCHEMA_VERSION: i64 = 1;

pub(super) const SETTINGS_KEYS: &[&str] = &[
    "theme",
    "accent_palette",
    "locale",
    "poll_interval_seconds",
    "youtube_transcription_provider",
    "youtube_transcription_model",
    "youtube_transcription_timeout_seconds",
    "general_analysis_provider",
    "general_analysis_model",
    "general_analysis_timeout_seconds",
    "ai_analysis_mode",
    "log_level",
    "log_max_files",
    "log_max_file_bytes",
    "shortcut_bindings",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPayload {
    pub file_name: String,
    pub media_type: String,
    pub contents: String,
    pub summary: ImportExportSummary,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExportSummary {
    pub companies: usize,
    pub watchlists: usize,
    pub memberships: usize,
    pub notebook_entries: usize,
    pub research_questions: usize,
    pub evidence_links: usize,
    pub ai_research_briefs: usize,
    pub ai_research_brief_citations: usize,
    pub research_reminders: usize,
    pub ai_research_digests: usize,
    pub ai_research_digest_citations: usize,
    pub settings: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub valid: bool,
    pub summary: ImportApplySummary,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportApplySummary {
    pub companies_created: usize,
    pub companies_merged: usize,
    pub watchlists_created: usize,
    pub watchlists_merged: usize,
    pub memberships_created: usize,
    pub notebook_entries_created: usize,
    pub notebook_entries_skipped: usize,
    pub research_questions_created: usize,
    pub research_questions_merged: usize,
    pub evidence_links_created: usize,
    pub evidence_links_skipped: usize,
    pub ai_research_briefs_created: usize,
    pub ai_research_briefs_skipped: usize,
    pub ai_research_brief_citations_created: usize,
    pub ai_research_brief_citations_skipped: usize,
    pub research_reminders_created: usize,
    pub research_reminders_skipped: usize,
    pub ai_research_digests_created: usize,
    pub ai_research_digests_skipped: usize,
    pub ai_research_digest_citations_created: usize,
    pub ai_research_digest_citations_skipped: usize,
    pub settings_updated: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportApplyResult {
    pub summary: ImportApplySummary,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResearchExportDocument {
    pub(super) schema_version: i64,
    pub(super) exported_at: String,
    pub(super) app_version: String,
    pub(super) sections: Vec<String>,
    pub(super) companies: Vec<ExportCompany>,
    pub(super) watchlists: Vec<ExportWatchlist>,
    pub(super) memberships: Vec<ExportMembership>,
    pub(super) notebook_entries: Vec<ExportNotebookEntry>,
    #[serde(default)]
    pub(super) research_questions: Vec<ExportResearchQuestion>,
    #[serde(default)]
    pub(super) evidence_links: Vec<ExportEvidenceLink>,
    #[serde(default)]
    pub(super) ai_research_briefs: Vec<ExportAiResearchBrief>,
    #[serde(default)]
    pub(super) ai_research_brief_citations: Vec<ExportAiResearchBriefCitation>,
    #[serde(default)]
    pub(super) research_reminders: Vec<ExportResearchReminder>,
    #[serde(default)]
    pub(super) ai_research_digests: Vec<ExportAiResearchDigest>,
    #[serde(default)]
    pub(super) ai_research_digest_citations: Vec<ExportAiResearchDigestCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportCompany {
    pub(super) id: String,
    pub(super) exchange: String,
    pub(super) ticker: String,
    pub(super) qualified_ticker: String,
    pub(super) display_name: String,
    pub(super) isin: Option<String>,
    pub(super) cik: Option<String>,
    pub(super) lei: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportWatchlist {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportMembership {
    pub(super) watchlist_id: String,
    pub(super) company_qualified_ticker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportNotebookEntry {
    pub(super) id: String,
    pub(super) company_qualified_ticker: String,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) body_format: String,
    pub(super) tags: Vec<String>,
    pub(super) kind: String,
    pub(super) claim_status: Option<String>,
    pub(super) event_date: Option<String>,
    pub(super) follow_up_after: Option<String>,
    pub(super) follow_up_date: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) origins: Vec<ExportNotebookOrigin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportNotebookOrigin {
    pub(super) source_type: String,
    pub(super) source_id: Option<String>,
    pub(super) source_url: Option<String>,
    pub(super) label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportResearchQuestion {
    pub(super) id: String,
    pub(super) scope_type: String,
    pub(super) scope_id: String,
    pub(super) scope_company_qualified_ticker: Option<String>,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) status: String,
    pub(super) closed_at: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportEvidenceLink {
    pub(super) id: String,
    pub(super) from_type: String,
    pub(super) from_id: String,
    pub(super) to_type: String,
    pub(super) to_id: String,
    pub(super) relation_type: String,
    pub(super) created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportAiResearchBrief {
    pub(super) id: String,
    pub(super) job_id: String,
    pub(super) scope_type: String,
    pub(super) scope_id: String,
    pub(super) scope_company_qualified_ticker: Option<String>,
    pub(super) provider_id: String,
    pub(super) model: String,
    pub(super) prompt_version: String,
    pub(super) evidence_collector_version: String,
    pub(super) renderer_version: String,
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) content_markdown: String,
    pub(super) language: Option<String>,
    pub(super) generated_at: String,
    pub(super) created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportAiResearchBriefCitation {
    pub(super) id: String,
    pub(super) brief_id: String,
    pub(super) citation_key: String,
    pub(super) evidence_type: String,
    pub(super) evidence_id: String,
    pub(super) label: String,
    pub(super) snippet: Option<String>,
    pub(super) created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportResearchReminder {
    pub(super) id: String,
    pub(super) scope_type: String,
    pub(super) scope_id: String,
    pub(super) scope_company_qualified_ticker: Option<String>,
    pub(super) company_qualified_ticker: Option<String>,
    pub(super) reminder_kind: String,
    pub(super) source_type: Option<String>,
    pub(super) source_id: Option<String>,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) due_at: Option<String>,
    pub(super) status: String,
    pub(super) snoozed_until: Option<String>,
    pub(super) completed_at: Option<String>,
    pub(super) dismissed_at: Option<String>,
    pub(super) created_at: String,
    pub(super) updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportAiResearchDigest {
    pub(super) id: String,
    pub(super) job_id: String,
    pub(super) scope_type: String,
    pub(super) scope_id: String,
    pub(super) scope_company_qualified_ticker: Option<String>,
    pub(super) provider_id: String,
    pub(super) model: String,
    pub(super) prompt_version: String,
    pub(super) evidence_collector_version: String,
    pub(super) renderer_version: String,
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) content_markdown: String,
    pub(super) language: Option<String>,
    pub(super) generated_at: String,
    pub(super) created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportAiResearchDigestCitation {
    pub(super) id: String,
    pub(super) digest_id: String,
    pub(super) citation_key: String,
    pub(super) evidence_type: String,
    pub(super) evidence_id: String,
    pub(super) label: String,
    pub(super) snippet: Option<String>,
    pub(super) created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SettingsExportDocument {
    pub(super) schema_version: i64,
    pub(super) exported_at: String,
    pub(super) app_version: String,
    pub(super) settings: ExportSettings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportSettings {
    pub(super) theme: Option<String>,
    pub(super) accent_palette: Option<String>,
    pub(super) locale: Option<String>,
    pub(super) poll_interval_seconds: Option<i64>,
    pub(super) youtube_transcription_provider: Option<String>,
    pub(super) youtube_transcription_model: Option<String>,
    pub(super) youtube_transcription_timeout_seconds: Option<i64>,
    pub(super) general_analysis_provider: Option<String>,
    pub(super) general_analysis_model: Option<String>,
    pub(super) general_analysis_timeout_seconds: Option<i64>,
    pub(super) ai_analysis_mode: Option<String>,
    pub(super) log_level: Option<String>,
    pub(super) log_max_files: Option<i64>,
    pub(super) log_max_file_bytes: Option<i64>,
    pub(super) shortcut_bindings: Option<HashMap<String, ShortcutBindingSetting>>,
}
