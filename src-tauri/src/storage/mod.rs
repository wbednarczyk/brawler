use std::sync::{Arc, Mutex};

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
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

mod ai_analysis;
mod companies;
mod diagnostics;
mod error;
mod events;
mod feed;
mod metrics;
mod migrations;
mod notebooks;
mod registry;
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
pub use metrics::{
    LocalMetricsSnapshot, MetricKind, MetricLabel, MetricSample, MetricUnit, RuntimeMetricCounters,
};
pub use migrations::{open_database, open_in_memory_database};
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
}

impl AppState {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection)),
            runtime_metrics: Arc::new(RuntimeMetricCounters::default()),
        }
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

    pub fn gpw_company_registry_needs_bootstrap_refresh(&self) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::gpw_company_registry_needs_bootstrap_refresh(&connection)
    }

    pub fn gpw_company_registry_is_stale(&self, stale_after_seconds: i64) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::gpw_company_registry_is_stale(&connection, stale_after_seconds)
    }

    pub fn refresh_gpw_company_registry(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        companies::refresh_gpw_company_registry(&mut connection, entries, fetched_at)
    }

    pub fn delete_company(&self, company_id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        companies::delete_company(&connection, company_id)
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

        Ok(sources::find_company_for_gpw_listing(&connection, ticker, isin)?.is_some())
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
        let connection = self.connection.lock().expect("database mutex poisoned");

        registry::list_source_adapters(&connection)
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
