use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::source_adapters::bankier_calendar::{
    BankierCalendarEventItem, ADAPTER_ID as BANKIER_CALENDAR_ADAPTER_ID,
    ATTRIBUTION as BANKIER_CALENDAR_ATTRIBUTION, SOURCE_URL as BANKIER_CALENDAR_SOURCE_URL,
};
use crate::source_adapters::bankier_company::{
    BankierCompanyAttachment, BankierCompanyIdentifiers, BankierCompanyItem, BankierCompanyTarget,
    ADAPTER_ID as BANKIER_COMPANY_ADAPTER_ID, ATTRIBUTION as BANKIER_COMPANY_ATTRIBUTION,
    DISPLAY_NAME as BANKIER_COMPANY_DISPLAY_NAME, SOURCE_URL as BANKIER_COMPANY_SOURCE_URL,
};
use crate::source_adapters::bankier_rss::{
    BankierRssItem, ADAPTER_ID as BANKIER_RSS_ADAPTER_ID, ATTRIBUTION as BANKIER_RSS_ATTRIBUTION,
    DISPLAY_NAME as BANKIER_RSS_DISPLAY_NAME, SOURCE_URL as BANKIER_RSS_SOURCE_URL,
};
use crate::source_adapters::gpw_company_registry::{
    GpwCompanyRegistryEntry, ADAPTER_ID as GPW_REGISTRY_ADAPTER_ID,
    SOURCE_URL as GPW_REGISTRY_SOURCE_URL,
};
use crate::source_adapters::gpw_espi_ebi::{
    GpwReportAttachment, GpwReportListing, ADAPTER_ID, DISPLAY_NAME,
};
use crate::source_adapters::gpw_market_events::{
    GpwMarketEventItem, ADAPTER_ID as GPW_MARKET_EVENTS_ADAPTER_ID,
    ATTRIBUTION as GPW_MARKET_EVENTS_ATTRIBUTION, SOURCE_URL as GPW_MARKET_EVENTS_SOURCE_URL,
};
use crate::source_adapters::newconnect_company_directory::{
    ADAPTER_ID as NEWCONNECT_DIRECTORY_ADAPTER_ID, SOURCE_URL as NEWCONNECT_DIRECTORY_SOURCE_URL,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

mod ai_analysis;
mod companies;
mod diagnostics;
mod error;
mod events;
mod feed;
mod feed_matching;
mod financials;
mod import_export;
mod kpi_extraction;
mod licensing;
mod metrics;
mod migrations;
mod notebooks;
mod registry;
mod report_documents;
mod research;
mod research_briefs;
mod research_digests;
mod research_reminders;
mod settings;
mod sources;
mod transcripts;
mod types;
mod watchlists;

pub use ai_analysis::{
    AiAnalysisJob, AiAnalysisResult, AiAnalysisSourceReference, CompletedAiAnalysis,
    NewAiAnalysisJob, NewAiAnalysisSourceReference,
};
pub use diagnostics::{DiagnosticEvent, DiagnosticScope, NewDiagnosticEvent};
pub use error::{StorageError, StorageResult};
pub use financials::{
    FinancialFact, FinancialPeriod, KpiDefinition, KpiRelevance, ListFinancialFactsInput,
    ListFinancialPeriodsInput, ListKpiDefinitionsInput, NewFinancialFact, NewFinancialPeriod,
    NewKpiDefinition, NewKpiRelevance, UpdateFinancialFact, UpdateFinancialPeriod,
    UpdateKpiRelevance,
};
pub use import_export::{ExportPayload, ImportApplyResult, ImportPreview};
pub use kpi_extraction::{
    CompletedKpiExtraction, ConfirmKpiProposalInput, KpiExtractionJob, KpiExtractionProposal,
    NewKpiExtractionJob, NewKpiProposal,
};
pub use licensing::{LicenseMetadataUpdate, StoredLicenseMetadata};
pub use metrics::{
    LocalMetricsSnapshot, MetricKind, MetricLabel, MetricSample, MetricUnit, RuntimeMetricCounters,
};
pub use migrations::{open_database, open_in_memory_database};
pub use report_documents::{CaptureReportDocumentInput, ReportDocument};
pub use research_briefs::{
    CompletedResearchBrief, NewResearchBriefCitation, NewResearchBriefJob, ResearchBrief,
    ResearchBriefCitation, ResearchBriefEvidenceContext, ResearchBriefJob, ResearchBriefScopeInput,
    RESEARCH_BRIEF_COLLECTOR_VERSION, RESEARCH_BRIEF_PROMPT_VERSION,
    RESEARCH_BRIEF_RENDERER_VERSION,
};
pub use research_digests::{
    completed_digest_from_provider_output, CompletedResearchDigest, NewResearchDigestJob,
    ResearchDigest, ResearchDigestCitation, ResearchDigestEvidenceContext, ResearchDigestJob,
    ResearchDigestScopeInput, RESEARCH_DIGEST_COLLECTOR_VERSION, RESEARCH_DIGEST_PROMPT_VERSION,
    RESEARCH_DIGEST_RENDERER_VERSION,
};
pub use research_reminders::{
    NewResearchReminder, ResearchReminder, ResearchReminderListInput, ResearchReminderUpdate,
};
pub use settings::{
    AiProviderSettings, LogSettings, SettingsUpdate, ShortcutBindingSetting, UserSettings,
};
pub use transcripts::{
    CreateNoteFromTranscriptSelectionInput, NewTranscriptJob, NewTranscriptSegment,
    ResolveTranscriptJobCompanyInput, TranscriptJob, TranscriptJobListInput, TranscriptNoteDraft,
    TranscriptSegment, UpdateTranscriptJobInput,
};
pub use types::*;

const PORTAL_ANALIZ_SOURCE_URL: &str = "https://portalanaliz.pl/";
const BANKIER_FIRMA_RSS_SOURCE_URL: &str = "https://www.bankier.pl/rss/firma.xml";
const BANKIER_WIADOMOSCI_RSS_SOURCE_URL: &str = "https://www.bankier.pl/rss/wiadomosci.xml";
const STREFA_REPORT_CALENDAR_SOURCE_URL: &str = "https://strefainwestorow.pl/dane/raporty";
const MONEY_CALENDAR_SOURCE_URL: &str = "https://www.money.pl/gielda/raporty/";

#[derive(Clone)]
pub struct AppState {
    connection: Arc<Mutex<Connection>>,
    runtime_metrics: Arc<RuntimeMetricCounters>,
    data_dir: PathBuf,
}

impl AppState {
    pub fn new(connection: Connection) -> Self {
        Self::with_data_dir(connection, std::env::temp_dir().join("brawler-test-data"))
    }

    pub fn with_data_dir(connection: Connection, data_dir: PathBuf) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection)),
            runtime_metrics: Arc::new(RuntimeMetricCounters::default()),
            data_dir,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn increment_runtime_counter(&self, name: &'static str, labels: &[(&'static str, &str)]) {
        self.runtime_metrics.increment(name, labels);
    }

    pub fn add_runtime_counter_value(
        &self,
        name: &'static str,
        labels: &[(&'static str, &str)],
        value: f64,
    ) {
        self.runtime_metrics.add_counter_value(name, labels, value);
    }

    pub fn observe_runtime_duration_seconds(
        &self,
        name: &'static str,
        labels: &[(&'static str, &str)],
        duration_seconds: f64,
    ) {
        self.runtime_metrics
            .observe_duration_seconds(name, labels, duration_seconds);
    }

    pub fn local_metrics_snapshot(
        &self,
        app_data_dir: &std::path::Path,
    ) -> StorageResult<LocalMetricsSnapshot> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        metrics::collect_local_metrics_snapshot(&connection, &self.runtime_metrics, app_data_dir)
    }

    pub fn database_status(&self) -> StorageResult<DatabaseStatus> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        migrations::database_status(&connection)
    }

    pub fn list_companies(&self) -> StorageResult<Vec<Company>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::list_companies(&connection)
    }

    pub fn create_company(&self, input: NewCompany) -> StorageResult<Company> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::create_company(&connection, input)
    }

    pub fn lookup_company(
        &self,
        input: CompanyLookupInput,
    ) -> StorageResult<Option<CompanyLookupResult>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::lookup_company(&connection, input)
    }

    pub fn company_directories_need_bootstrap_refresh(&self) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::company_directories_need_bootstrap_refresh(&connection)
    }

    pub fn company_directories_are_stale(&self, stale_after_seconds: i64) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::company_directories_are_stale(&connection, stale_after_seconds)
    }

    pub fn refresh_gpw_company_registry(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        companies::refresh_company_directory(
            &mut connection,
            GPW_REGISTRY_ADAPTER_ID,
            entries,
            fetched_at,
        )
    }

    pub fn refresh_newconnect_company_directory(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        companies::refresh_company_directory(
            &mut connection,
            NEWCONNECT_DIRECTORY_ADAPTER_ID,
            entries,
            fetched_at,
        )
    }

    pub fn delete_company(&self, company_id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::delete_company(&connection, company_id)
    }

    pub fn export_research_data(&self) -> StorageResult<ExportPayload> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        import_export::export_research_data(&connection)
    }

    pub fn preview_research_import(&self, contents: &str) -> StorageResult<ImportPreview> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        import_export::preview_research_import(&connection, contents)
    }

    pub fn apply_research_import(&self, contents: &str) -> StorageResult<ImportApplyResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        import_export::apply_research_import(&mut connection, contents)
    }

    pub fn export_settings_data(&self) -> StorageResult<ExportPayload> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        import_export::export_settings_data(&connection)
    }

    pub fn preview_settings_import(&self, contents: &str) -> StorageResult<ImportPreview> {
        import_export::preview_settings_import(contents)
    }

    pub fn apply_settings_import(&self, contents: &str) -> StorageResult<ImportApplyResult> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        import_export::apply_settings_import(&connection, contents)
    }

    pub fn list_watchlists(&self) -> StorageResult<Vec<Watchlist>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::list_watchlists(&connection)
    }

    pub fn list_watchlist_memberships(&self) -> StorageResult<Vec<WatchlistMembership>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::list_watchlist_memberships(&connection)
    }

    pub fn create_watchlist(&self, input: NewWatchlist) -> StorageResult<Watchlist> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::create_watchlist(&connection, input)
    }

    pub fn rename_watchlist(&self, input: WatchlistUpdate) -> StorageResult<Watchlist> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::rename_watchlist(&connection, input)
    }

    pub fn delete_watchlist(&self, watchlist_id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::delete_watchlist(&connection, watchlist_id)
    }

    pub fn add_company_to_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::add_company_to_watchlist(&connection, input)
    }

    pub fn remove_company_from_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        watchlists::remove_company_from_watchlist(&connection, input)
    }

    pub fn list_feed_items(&self) -> StorageResult<Vec<FeedItem>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        feed::list_feed_items(&connection)
    }

    pub fn list_unmatched_source_items(
        &self,
        adapter_id: &str,
    ) -> StorageResult<Vec<UnmatchedSourceItem>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        feed::list_unmatched_source_items(&connection, adapter_id)
    }

    pub fn list_research_evidence(
        &self,
        input: ResearchEvidenceInput,
    ) -> StorageResult<ResearchTimelineResult> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::list_research_evidence(&connection, input)
    }

    pub fn mark_research_scope_reviewed(
        &self,
        input: ResearchReviewCheckpointInput,
    ) -> StorageResult<ResearchReviewCheckpoint> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::mark_research_scope_reviewed(&connection, input)
    }

    pub fn list_research_review_state(
        &self,
        input: ResearchReviewCheckpointInput,
    ) -> StorageResult<Option<ResearchReviewCheckpoint>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::list_research_review_state(&connection, input)
    }

    pub fn list_research_questions(
        &self,
        input: ResearchQuestionListInput,
    ) -> StorageResult<Vec<ResearchQuestion>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::list_research_questions(&connection, input)
    }

    pub fn create_research_question(
        &self,
        input: NewResearchQuestion,
    ) -> StorageResult<ResearchQuestion> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::create_research_question(&connection, input)
    }

    pub fn update_research_question(
        &self,
        input: ResearchQuestionUpdate,
    ) -> StorageResult<ResearchQuestion> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::update_research_question(&connection, input)
    }

    pub fn delete_research_question(&self, id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::delete_research_question(&connection, id)
    }

    pub fn create_evidence_link(&self, input: NewEvidenceLink) -> StorageResult<EvidenceLink> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::create_evidence_link(&connection, input)
    }

    pub fn list_evidence_links(
        &self,
        input: EvidenceLinkListInput,
    ) -> StorageResult<Vec<EvidenceLink>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::list_evidence_links(&connection, input)
    }

    pub fn delete_evidence_link(&self, id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research::delete_evidence_link(&connection, id)
    }

    pub fn list_research_reminders(
        &self,
        input: ResearchReminderListInput,
    ) -> StorageResult<Vec<ResearchReminder>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_reminders::list_research_reminders(&connection, input)
    }

    pub fn create_research_reminder(
        &self,
        input: NewResearchReminder,
    ) -> StorageResult<ResearchReminder> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_reminders::create_research_reminder(&connection, input)
    }

    pub fn update_research_reminder(
        &self,
        input: ResearchReminderUpdate,
    ) -> StorageResult<ResearchReminder> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_reminders::update_research_reminder(&connection, input)
    }

    pub fn delete_research_reminder(&self, id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_reminders::delete_research_reminder(&connection, id)
    }

    pub fn create_research_brief_job(
        &self,
        input: NewResearchBriefJob,
    ) -> StorageResult<ResearchBriefJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::create_research_brief_job(&connection, input)
    }

    pub fn list_research_brief_jobs(
        &self,
        input: ResearchBriefScopeInput,
    ) -> StorageResult<Vec<ResearchBriefJob>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::list_research_brief_jobs(&connection, input)
    }

    pub fn get_research_brief_job(&self, job_id: &str) -> StorageResult<ResearchBriefJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::get_research_brief_job(&connection, job_id)
    }

    pub fn collect_research_brief_evidence(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchBriefEvidenceContext> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::collect_research_brief_evidence(&connection, job_id)
    }

    pub fn mark_research_brief_job_running(&self, job_id: &str) -> StorageResult<ResearchBriefJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::mark_research_brief_job_running(&connection, job_id)
    }

    pub fn complete_research_brief_job(
        &self,
        input: CompletedResearchBrief,
    ) -> StorageResult<ResearchBriefJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::complete_research_brief_job(&connection, input)
    }

    pub fn mark_research_brief_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ResearchBriefJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_briefs::mark_research_brief_job_failed(&connection, job_id, error_code, error)
    }

    pub fn create_research_digest_job(
        &self,
        input: NewResearchDigestJob,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::create_research_digest_job(&connection, input)
    }

    pub fn list_research_digest_jobs(
        &self,
        input: ResearchDigestScopeInput,
    ) -> StorageResult<Vec<ResearchDigestJob>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::list_research_digest_jobs(&connection, input)
    }

    pub fn get_research_digest_job(&self, job_id: &str) -> StorageResult<ResearchDigestJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::get_research_digest_job(&connection, job_id)
    }

    pub fn collect_research_digest_evidence(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchDigestEvidenceContext> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::collect_research_digest_evidence(&connection, job_id)
    }

    pub fn mark_research_digest_job_running(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::mark_research_digest_job_running(&connection, job_id)
    }

    pub fn complete_research_digest_job(
        &self,
        input: CompletedResearchDigest,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::complete_research_digest_job(&connection, input)
    }

    pub fn mark_research_digest_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        research_digests::mark_research_digest_job_failed(&connection, job_id, error_code, error)
    }

    pub fn ingest_gpw_report_listings(
        &self,
        listings: &[GpwReportListing],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        sources::ingest_gpw_report_listings(&mut connection, listings)
    }

    pub fn ingest_bankier_rss_items(
        &self,
        items: &[BankierRssItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        sources::ingest_bankier_rss_items(&mut connection, items)
    }

    pub fn list_bankier_company_targets(&self) -> StorageResult<Vec<BankierCompanyTarget>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        sources::list_bankier_company_targets(&connection)
    }

    pub fn upsert_bankier_company_identifiers(
        &self,
        company_id: &str,
        identifiers: &BankierCompanyIdentifiers,
    ) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        sources::upsert_bankier_company_identifiers(&connection, company_id, identifiers)
    }

    pub fn list_bankier_company_detail_cached_urls(&self) -> StorageResult<Vec<String>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        sources::list_bankier_company_detail_cached_urls(&connection)
    }

    pub fn ingest_bankier_company_items(
        &self,
        items: &[BankierCompanyItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        sources::ingest_bankier_company_items(&mut connection, items)
    }

    pub fn ingest_gpw_market_event_items(
        &self,
        items: &[GpwMarketEventItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        events::ingest_gpw_market_event_items(&mut connection, items)
    }

    pub fn ingest_bankier_calendar_event_items(
        &self,
        items: &[BankierCalendarEventItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        events::ingest_bankier_calendar_event_items(&mut connection, items)
    }

    pub fn tracks_gpw_listing_company(&self, ticker: &str, isin: &str) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        Ok(feed_matching::find_company_for_gpw_listing(&connection, ticker, isin)?.is_some())
    }

    pub fn update_feed_item_state(&self, input: FeedItemStateInput) -> StorageResult<FeedItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        feed::update_feed_item_state(&connection, input)
    }

    pub fn get_feed_item(&self, feed_item_id: &str) -> StorageResult<FeedItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        feed::get_feed_item(&connection, feed_item_id)
    }

    pub fn prune_old_feed_items(&self, retention_days: i64) -> StorageResult<FeedPruneResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        feed::prune_old_feed_items(&mut connection, retention_days)
    }

    pub fn delete_unsaved_feed_items(&self) -> StorageResult<FeedDeleteResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        feed::delete_unsaved_feed_items(&mut connection)
    }

    pub fn list_notebook_entries(&self, company_id: &str) -> StorageResult<Vec<NotebookEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        notebooks::list_notebook_entries(&connection, company_id)
    }

    pub fn create_notebook_entry(&self, input: NewNotebookEntry) -> StorageResult<NotebookEntry> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        notebooks::create_notebook_entry(&connection, input)
    }

    pub fn create_note_from_transcript_selection(
        &self,
        input: CreateNoteFromTranscriptSelectionInput,
    ) -> StorageResult<NotebookEntry> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::create_note_from_transcript_selection(&connection, input)
    }

    pub fn update_notebook_entry(
        &self,
        input: NotebookEntryUpdate,
    ) -> StorageResult<NotebookEntry> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        notebooks::update_notebook_entry(&connection, input)
    }

    pub fn delete_notebook_entry(&self, notebook_entry_id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        notebooks::delete_notebook_entry(&connection, notebook_entry_id)
    }

    pub fn list_company_events(
        &self,
        input: CompanyEventListInput,
    ) -> StorageResult<Vec<CompanyEvent>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        events::list_company_events(&connection, input)
    }

    pub fn create_company_event(&self, input: NewCompanyEvent) -> StorageResult<CompanyEvent> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        events::create_company_event(&connection, input)
    }

    pub fn list_transcript_jobs(
        &self,
        input: TranscriptJobListInput,
    ) -> StorageResult<Vec<TranscriptJob>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::list_transcript_jobs(&connection, input)
    }

    pub fn delete_transcript_job(&self, job_id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::delete_transcript_job(&connection, job_id)
    }

    pub fn create_transcript_job(&self, input: NewTranscriptJob) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::create_transcript_job(&connection, input)
    }

    pub fn update_transcript_job(
        &self,
        input: UpdateTranscriptJobInput,
    ) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::update_transcript_job(&connection, input)
    }

    pub fn list_transcript_segments(
        &self,
        transcript_job_id: &str,
    ) -> StorageResult<Vec<TranscriptSegment>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::list_transcript_segments(&connection, transcript_job_id)
    }

    pub fn create_transcript_segment(
        &self,
        input: NewTranscriptSegment,
    ) -> StorageResult<TranscriptSegment> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::create_transcript_segment(&connection, input)
    }

    pub fn resolve_transcript_job_company(
        &self,
        input: ResolveTranscriptJobCompanyInput,
    ) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::resolve_transcript_job_company(&connection, input)
    }

    pub fn get_transcript_job(&self, job_id: &str) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::get_transcript_job(&connection, job_id)
    }

    pub fn mark_transcript_job_running(&self, job_id: &str) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::mark_transcript_job_running(&connection, job_id)
    }

    pub fn mark_transcript_job_completed(&self, job_id: &str) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::mark_transcript_job_completed(&connection, job_id)
    }

    pub fn mark_transcript_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<TranscriptJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        transcripts::mark_transcript_job_failed(&connection, job_id, error_code, error)
    }

    pub fn list_source_adapters(&self) -> StorageResult<Vec<SourceAdapter>> {
        self.list_source_adapters_with_developer(false)
    }

    pub fn list_source_adapters_with_developer(
        &self,
        include_developer_only: bool,
    ) -> StorageResult<Vec<SourceAdapter>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::list_source_adapters(&connection, include_developer_only)
    }

    pub fn set_source_adapter_enabled(
        &self,
        adapter_id: &str,
        enabled: bool,
    ) -> StorageResult<SourceAdapter> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::set_source_adapter_enabled(&connection, adapter_id, enabled)
    }

    pub fn source_adapter_enabled(&self, adapter_id: &str) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::source_adapter_enabled(&connection, adapter_id)
    }

    pub fn list_company_registry_entries(&self) -> StorageResult<Vec<CompanyRegistryEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::list_company_registry_entries(&connection)
    }

    pub fn record_source_adapter_error(&self, adapter_id: &str, error: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::record_source_adapter_error(&connection, adapter_id, error)
    }

    pub fn record_source_adapter_attempt(
        &self,
        adapter_id: &str,
        trigger: &str,
    ) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::record_source_adapter_attempt(&connection, adapter_id, trigger)
    }

    pub fn record_source_adapter_state(
        &self,
        adapter_id: &str,
        key: &str,
        value: &str,
    ) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        sources::set_source_adapter_state(&connection, adapter_id, key, value)
    }

    pub fn get_settings(&self) -> StorageResult<UserSettings> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        settings::get_settings(&connection)
    }

    pub fn update_settings(&self, input: SettingsUpdate) -> StorageResult<UserSettings> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        settings::update_settings(&connection, input)
    }

    pub fn set_developer_mode_enabled(&self, enabled: bool) -> StorageResult<UserSettings> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        settings::set_developer_mode_enabled(&connection, enabled)
    }

    pub fn get_license_metadata(&self) -> StorageResult<Option<StoredLicenseMetadata>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        licensing::get_license_metadata(&connection)
    }

    pub fn upsert_license_metadata(
        &self,
        input: LicenseMetadataUpdate,
    ) -> StorageResult<StoredLicenseMetadata> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        licensing::upsert_license_metadata(&connection, input)
    }

    pub fn clear_license_metadata(&self) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        licensing::clear_license_metadata(&connection)
    }

    pub fn record_diagnostic_event(
        &self,
        input: NewDiagnosticEvent,
    ) -> StorageResult<Option<DiagnosticEvent>> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        diagnostics::record_diagnostic_event(&mut connection, input)
    }

    pub fn list_diagnostic_events(&self, limit: i64) -> StorageResult<Vec<DiagnosticEvent>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        diagnostics::list_diagnostic_events(&connection, limit)
    }

    pub fn clear_diagnostic_events(&self) -> StorageResult<usize> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        diagnostics::clear_diagnostic_events(&connection)
    }

    pub fn create_ai_analysis_job(&self, input: NewAiAnalysisJob) -> StorageResult<AiAnalysisJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        ai_analysis::create_ai_analysis_job(&connection, input)
    }

    pub fn list_ai_analysis_jobs(&self, feed_item_id: &str) -> StorageResult<Vec<AiAnalysisJob>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        ai_analysis::list_ai_analysis_jobs(&connection, feed_item_id)
    }

    pub fn get_ai_analysis_job(&self, job_id: &str) -> StorageResult<AiAnalysisJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        ai_analysis::get_ai_analysis_job(&connection, job_id)
    }

    pub fn mark_ai_analysis_job_running(&self, job_id: &str) -> StorageResult<AiAnalysisJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        ai_analysis::mark_ai_analysis_job_running(&connection, job_id)
    }

    pub fn complete_ai_analysis_job(
        &self,
        input: CompletedAiAnalysis,
    ) -> StorageResult<AiAnalysisJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        ai_analysis::complete_ai_analysis_job(&connection, input)
    }

    pub fn mark_ai_analysis_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<AiAnalysisJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        ai_analysis::mark_ai_analysis_job_failed(&connection, job_id, error_code, error)
    }

    pub fn list_kpi_definitions(
        &self,
        input: ListKpiDefinitionsInput,
    ) -> StorageResult<Vec<KpiDefinition>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::list_kpi_definitions(&connection, input)
    }

    pub fn create_kpi_definition(&self, input: NewKpiDefinition) -> StorageResult<KpiDefinition> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::create_kpi_definition(&connection, input)
    }

    pub fn list_financial_periods(
        &self,
        input: ListFinancialPeriodsInput,
    ) -> StorageResult<Vec<FinancialPeriod>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::list_financial_periods(&connection, input)
    }

    pub fn create_financial_period(
        &self,
        input: NewFinancialPeriod,
    ) -> StorageResult<FinancialPeriod> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::create_financial_period(&connection, input)
    }

    pub fn update_financial_period(
        &self,
        input: UpdateFinancialPeriod,
    ) -> StorageResult<FinancialPeriod> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::update_financial_period(&connection, input)
    }

    pub fn delete_financial_period(&self, id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::delete_financial_period(&connection, id)
    }

    pub fn list_kpi_relevance(&self, company_id: &str) -> StorageResult<Vec<KpiRelevance>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::list_kpi_relevance(&connection, company_id)
    }

    pub fn create_kpi_relevance(&self, input: NewKpiRelevance) -> StorageResult<KpiRelevance> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::create_kpi_relevance(&connection, input)
    }

    pub fn update_kpi_relevance(&self, input: UpdateKpiRelevance) -> StorageResult<KpiRelevance> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::update_kpi_relevance(&connection, input)
    }

    pub fn delete_kpi_relevance(&self, id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::delete_kpi_relevance(&connection, id)
    }

    pub fn list_financial_facts(
        &self,
        input: ListFinancialFactsInput,
    ) -> StorageResult<Vec<FinancialFact>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::list_financial_facts(&connection, input)
    }

    pub fn create_financial_fact(&self, input: NewFinancialFact) -> StorageResult<FinancialFact> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::create_financial_fact(&connection, input)
    }

    pub fn create_kpi_extraction_job(
        &self,
        input: NewKpiExtractionJob,
    ) -> StorageResult<KpiExtractionJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::create_kpi_extraction_job(&connection, input)
    }

    pub fn get_kpi_extraction_job(&self, job_id: &str) -> StorageResult<KpiExtractionJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::get_kpi_extraction_job(&connection, job_id)
    }

    pub fn list_kpi_extraction_jobs_by_document(
        &self,
        report_document_id: &str,
    ) -> StorageResult<Vec<KpiExtractionJob>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::list_kpi_extraction_jobs_by_document(&connection, report_document_id)
    }

    pub fn mark_kpi_extraction_job_running(&self, job_id: &str) -> StorageResult<KpiExtractionJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::mark_kpi_extraction_job_running(&connection, job_id)
    }

    pub fn mark_kpi_extraction_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<KpiExtractionJob> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::mark_kpi_extraction_job_failed(&connection, job_id, error_code, error)
    }

    pub fn complete_kpi_extraction_job(
        &self,
        input: CompletedKpiExtraction,
    ) -> StorageResult<KpiExtractionJob> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::complete_kpi_extraction_job(&mut connection, input)
    }

    pub fn confirm_kpi_proposal(
        &self,
        input: ConfirmKpiProposalInput,
    ) -> StorageResult<FinancialFact> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::confirm_kpi_proposal(&connection, input)
    }

    pub fn reject_kpi_proposal(&self, proposal_id: &str) -> StorageResult<KpiExtractionProposal> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        kpi_extraction::reject_kpi_proposal(&connection, proposal_id)
    }

    pub fn update_financial_fact(
        &self,
        input: UpdateFinancialFact,
    ) -> StorageResult<FinancialFact> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::update_financial_fact(&connection, input)
    }

    pub fn delete_financial_fact(&self, id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        financials::delete_financial_fact(&connection, id)
    }

    pub fn create_or_find_pending_report_document(
        &self,
        input: CaptureReportDocumentInput,
    ) -> StorageResult<ReportDocument> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        report_documents::create_or_find_pending(&connection, input)
    }

    pub fn mark_report_document_fetched(
        &self,
        id: &str,
        local_path: Option<&str>,
        content_type: Option<&str>,
        content_hash: Option<&str>,
        byte_size: Option<i64>,
    ) -> StorageResult<ReportDocument> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        report_documents::mark_fetched(
            &connection,
            id,
            local_path,
            content_type,
            content_hash,
            byte_size,
        )
    }

    pub fn mark_report_document_failed(
        &self,
        id: &str,
        error: &str,
    ) -> StorageResult<ReportDocument> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        report_documents::mark_failed(&connection, id, error)
    }

    pub fn get_report_document(&self, id: &str) -> StorageResult<ReportDocument> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        report_documents::get(&connection, id)
    }

    pub fn list_report_documents_by_company(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<ReportDocument>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        report_documents::list_by_company(&connection, company_id)
    }
}

pub(super) fn feed_item_id(dedupe_key: &str) -> String {
    format!("feed_{}", slug_part(dedupe_key))
}

pub(super) fn feed_item_attachment_id(feed_item_id: &str, url: &str) -> String {
    format!(
        "feed_attachment_{}_{}",
        slug_part(feed_item_id),
        slug_part(url)
    )
}

pub(super) fn validate_allowed_company_event_value(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidCompanyEventValue {
            key,
            value: value.to_owned(),
        })
    }
}

pub(super) fn company_id(exchange: &str, ticker: &str) -> String {
    format!("company_{}_{}", slug_part(exchange), slug_part(ticker))
}

pub(super) fn company_registry_entry_id(exchange: &str, ticker: &str) -> String {
    format!(
        "company_registry_{}_{}",
        slug_part(exchange),
        slug_part(ticker)
    )
}

pub(super) fn company_event_id(
    company_id: &str,
    event_type: &str,
    event_date: &str,
    title: &str,
) -> String {
    format!(
        "event_{}_{}_{}_{}",
        slug_part(company_id),
        slug_part(event_type),
        slug_part(event_date),
        slug_part(title)
    )
}

pub(super) fn company_event_source_id(source_adapter_id: &str, source_event_key: &str) -> String {
    format!(
        "event_{}_{}",
        slug_part(source_adapter_id),
        slug_part(source_event_key)
    )
}

pub(super) fn watchlist_id(name: &str) -> String {
    format!("watchlist_{}", slug_part(name))
}

pub(super) fn slug_part(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if character == '_' || character == '-' {
                Some('_')
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn empty_string_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|inner| {
        let trimmed = inner.trim().to_owned();

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(super) fn normalize_lookup_value(value: &str) -> String {
    value.trim().to_uppercase()
}

pub(super) fn normalize_name_lookup(value: &str) -> String {
    value.trim().to_uppercase()
}

#[cfg(test)]
mod tests;
