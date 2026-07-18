use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::source_adapters::bankier_calendar::{
    BankierCalendarEventItem, ADAPTER_ID as BANKIER_CALENDAR_ADAPTER_ID,
    ATTRIBUTION as BANKIER_CALENDAR_ATTRIBUTION,
};
use crate::source_adapters::bankier_company::{
    BankierCompanyAttachment, BankierCompanyIdentifiers, BankierCompanyItem, BankierCompanyTarget,
    ADAPTER_ID as BANKIER_COMPANY_ADAPTER_ID, ATTRIBUTION as BANKIER_COMPANY_ATTRIBUTION,
    DISPLAY_NAME as BANKIER_COMPANY_DISPLAY_NAME,
};
use crate::source_adapters::bankier_rss::{
    BankierRssItem, ADAPTER_ID as BANKIER_RSS_ADAPTER_ID, ATTRIBUTION as BANKIER_RSS_ATTRIBUTION,
    DISPLAY_NAME as BANKIER_RSS_DISPLAY_NAME,
};
use crate::source_adapters::gpw_company_registry::GpwCompanyRegistryEntry;
use crate::source_adapters::gpw_espi_ebi::{
    GpwReportAttachment, GpwReportListing, ADAPTER_ID, DISPLAY_NAME,
};
use crate::source_adapters::gpw_market_events::{
    GpwMarketEventItem, ADAPTER_ID as GPW_MARKET_EVENTS_ADAPTER_ID,
    ATTRIBUTION as GPW_MARKET_EVENTS_ATTRIBUTION,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use self::database::{Database, DbGuard};

mod ai_analysis;
mod attention;
mod autopilot;
mod backup;
mod claim_extraction;
mod cockpit_layouts;
mod companies;
mod database;
mod decision_journal;
mod diagnostics;
mod error;
mod events;
mod feed;
mod feed_matching;
mod financials;
mod fundamentals_provenance;
mod history_sweeps;
mod import_export;
mod ingestion;
mod insider;
mod jobs;
mod kpi_extraction;
mod licensing;
mod management_claims;
mod management_holdings;
mod market_data;
mod metrics;
mod migrations;
mod morning_briefings;
mod notebooks;
mod ownership;
mod pool;
mod quality_frameworks;
mod queue_config;
mod reconciliation;
mod red_flags;
mod registry;
mod report_documents;
mod report_expectations;
mod report_season;
mod report_sections;
mod research;
mod research_briefs;
mod research_digests;
mod research_reminders;
mod search;
mod settings;
mod short_positions;
mod signals;
mod sources;
mod transcripts;
mod types;
mod watchlists;

pub use ai_analysis::{
    AiAnalysisJob, AiAnalysisResult, AiAnalysisSourceReference, CompletedAiAnalysis,
    NewAiAnalysisJob, NewAiAnalysisSourceReference,
};
pub use attention::{
    AlertRule, AlertRuleUpdate, AttentionEvent, AttentionEventListInput, AttentionStore,
    NewAlertRule,
};
pub use autopilot::{
    is_valid_mode as is_valid_autopilot_mode, AutopilotRun, AutopilotStore, CompanyAutopilot,
    ListAutopilotRunsInput, MODE_ASSIST, MODE_AUTOPILOT, MODE_OFF,
};
pub use backup::{BackupEntry, BackupStatus};
pub use claim_extraction::{
    ClaimExtractionJob, ClaimExtractionProposal, CompletedClaimExtraction,
    ConfirmClaimProposalInput, NewClaimExtractionJob, NewClaimProposal,
};
pub use diagnostics::{DiagnosticEvent, DiagnosticScope, NewDiagnosticEvent};
pub use reconciliation::{ReconciliationResult, ReconciliationStore};
// `AppState` is defined in this module.
pub use ai_analysis::AiAnalysisStore;
pub use claim_extraction::ClaimExtractionStore;
pub use cockpit_layouts::{CockpitLayout, CockpitLayoutStore, NewCockpitLayout};
pub use decision_journal::{
    DecisionEntry, DecisionEntryListInput, DecisionJournalStore, NewDecisionEntry,
    DECISION_ENTRY_KINDS,
};
pub use diagnostics::DiagnosticsStore;
pub use error::{StorageError, StorageResult};
pub use events::EventStore;
pub use feed::FeedStore;
pub use financials::FinancialsStore;
pub use financials::{
    FinancialFact, FinancialPeriod, KpiDefinition, KpiRelevance, ListFinancialFactsInput,
    ListFinancialPeriodsInput, ListKpiDefinitionsInput, NewFinancialFact, NewFinancialPeriod,
    NewKpiDefinition, NewKpiRelevance, PeriodFactCoverage, UpdateFinancialFact,
    UpdateFinancialPeriod, UpdateKpiRelevance,
};
pub use fundamentals_provenance::{FactProvenance, FundamentalsProvenanceStore, NewFactProvenance};
pub use history_sweeps::{HistorySweep, HistorySweepOutcome, HistorySweepStore};
pub use import_export::ImportExportStore;
pub use import_export::{ExportPayload, ImportApplyResult, ImportPreview};
pub use insider::{
    AttachmentConflict, AttachmentMergeOutcome, AttachmentPendingFiling, InsiderOverviewSource,
    InsiderStore, InsiderTransactionRow,
};
pub use jobs::{ClaimedJob, JobQueueCounts, JobQueueStore, JobStatusRow};
pub use kpi_extraction::KpiExtractionStore;
pub use kpi_extraction::{
    CompletedKpiExtraction, ConfirmKpiProposalInput, ConfirmedKpiFact, KpiExtractionJob,
    KpiExtractionProposal, NewKpiExtractionJob, NewKpiProposal, PendingKpiProposal,
    PeriodPendingProposals, StructuredFactCommit, StructuredFactInput,
};
pub use licensing::LicensingStore;
pub use licensing::{LicenseMetadataUpdate, StoredLicenseMetadata};
pub use management_claims::ManagementClaimStore;
pub use management_claims::{
    ClaimToVerify, ClaimsToVerify, ManagementClaim, ManagementClaimUpdate, NewManagementClaim,
    SetClaimVerdictInput, VerifyingFactCandidate,
};
pub use management_holdings::{
    DocumentNeedingManagementExtraction, ManagementHoldingRow, ManagementHoldingsResidual,
    ManagementHoldingsStore, NewManagementHolding, SkinInTheGameMatch,
};
pub use metrics::{
    LocalMetricsSnapshot, MetricKind, MetricLabel, MetricSample, MetricUnit, RuntimeMetricCounters,
};
pub use migrations::{open_database, open_in_memory_database};
pub use morning_briefings::{
    briefing_evidence_items, build_briefing_narrative, compose_briefing, BriefingSources,
    CompletedNarrative, ComposedBriefing, ComposedBriefingItem, MorningBriefing,
    MorningBriefingItem, MorningBriefingStore,
};
pub use notebooks::NotebookStore;
pub use ownership::{
    compare_witness, DocumentNeedingOwnershipExtraction, HolderDictionaryEntry,
    HolderTypeProposalRow, NewHolderTypeProposal, NewOwnershipOcrProposal, NewOwnershipStake,
    OcrHolderRow, OwnershipCurrentState, OwnershipExtractionResidual, OwnershipOcrProposalRecord,
    OwnershipStakeRow, OwnershipStore, OwnershipWitnessResult, ResidualNeedingOcr,
    WitnessComparison, WitnessDivergence, WitnessHolder, OCR_STATE_NO_TABLE, OCR_STATE_PROPOSED,
    OCR_STATE_REJECTED,
};
pub use pool::open_pool;
pub use quality_frameworks::QualityFrameworkStore;
pub use quality_frameworks::{
    AssessedFrameworkRef, CloneFrameworkInput, CriterionResult, EvaluateFrameworkInput,
    FrameworkCriterion, FrameworkEvaluation, ListFrameworkEvaluationsInput, MetricKeyInfo,
    NewFrameworkCriterion, NewQualityFramework, PersistQualitativeAssessmentInput,
    QualitativeCriterionResult, QualitativeVerdictChange, QualityFramework,
    UpdateFrameworkCriterion, UpdateQualityFramework, ValidateCriterionResult,
};
pub use queue_config::QueueConfig;
pub use red_flags::{AcknowledgeRedFlagInput, RedFlag, RedFlagStore, RedFlagsInput, RedFlagsView};
pub use registry::SourceRegistryStore;
pub use report_documents::ReportDocumentStore;
pub use report_documents::{
    CaptureReportDocumentInput, ReclassifyReportDocumentsSummary, ReportDocument,
};
pub use report_expectations::{
    evaluate_metric_expectation, ExpectationMetric, ExpectationReview, ExpectationReviewInput,
    ListReportExpectationsInput, MetricExpectationOutcome, MetricExpectationReview,
    NewExpectationMetric, NewReportExpectation, RecordExpectationResolutionInput,
    ReportExpectation, ReportExpectationStore, UpdateReportExpectation, EXPECTATION_COMPARATORS,
};
pub use report_season::ReportSeasonStore;
pub use report_season::{
    CalendarFreshness, MarkReportPreparedInput, MarkReportProcessedInput, PreReportCard,
    PreReportCardInput, PreReportKpi, ReportPreparation, ReportSeasonEntry, ReportSeasonInput,
    ReportSeasonResult,
};
pub use report_sections::{ReportSectionStore, StoredExtraction, StoredSection};
pub use research::ResearchStore;
pub use research::{citation_resolves, supplied_evidence_refs};
pub use research_briefs::ResearchBriefStore;
pub use research_briefs::{
    CompletedResearchBrief, NewResearchBriefCitation, NewResearchBriefJob, ResearchBrief,
    ResearchBriefCitation, ResearchBriefEvidenceContext, ResearchBriefJob, ResearchBriefScopeInput,
    RESEARCH_BRIEF_COLLECTOR_VERSION, RESEARCH_BRIEF_PROMPT_VERSION,
    RESEARCH_BRIEF_RENDERER_VERSION,
};
pub use research_digests::ResearchDigestStore;
pub use research_digests::{
    completed_digest_from_provider_output, CompletedResearchDigest, NewResearchDigestJob,
    ResearchDigest, ResearchDigestCitation, ResearchDigestEvidenceContext, ResearchDigestJob,
    ResearchDigestScopeInput, RESEARCH_DIGEST_COLLECTOR_VERSION, RESEARCH_DIGEST_PROMPT_VERSION,
    RESEARCH_DIGEST_RENDERER_VERSION,
};
pub use research_reminders::ResearchReminderStore;
pub use research_reminders::{
    NewResearchReminder, ResearchReminder, ResearchReminderListInput, ResearchReminderUpdate,
};
pub use search::SearchMatch;
pub use settings::SettingsStore;
pub(crate) use settings::MCP_PORT_DEFAULT;
pub use settings::{
    AiProviderSettings, CapabilityProviderEntry, LogSettings, SettingsUpdate,
    ShortcutBindingSetting, UserSettings,
};
pub use short_positions::{
    ShortPositionEventRow, ShortPositionExit, ShortPositionRow, ShortPositionsInput,
    ShortPositionsView,
};
pub use signals::SignalNeedingDate;
pub use signals::SignalStore;
pub use sources::{BackfillMarketStatus, SourcesStore};
pub use transcripts::TranscriptStore;
pub use transcripts::{
    CreateNoteFromTranscriptSelectionInput, NewTranscriptJob, NewTranscriptSegment,
    ResolveTranscriptJobCompanyInput, TranscriptJob, TranscriptJobListInput, TranscriptNoteDraft,
    TranscriptSegment, UpdateTranscriptJobInput,
};
pub use types::*;
pub use watchlists::WatchlistStore;

/// Live progress/diagnostics for an on-track history backfill (ADR 0036). Held in shared
/// memory (not persisted): backfill is an explicit, app-open-only action, and idempotent
/// re-runs mean a lost in-flight status is never harmful.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct BackfillProgress {
    pub company_id: String,
    /// `running` | `completed` | `failed`.
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"running\" | \"completed\" | \"failed\"")
    )]
    pub status: String,
    pub pages_fetched: usize,
    pub items_ingested: usize,
    pub documents_stored: usize,
    pub detail_errors: usize,
    /// True when the page cap ended the fetch before the configured backfill
    /// cutoff was reached (ADR 0077 §3) — older filings may be missing. Surfaced
    /// as an explicit warning in the coverage panel, never silently dropped.
    pub truncated: bool,
    /// The chained history sweep's id, when a completed backfill auto-chained one
    /// (ADR 0077 §3). The sweep row is created **eagerly** at enqueue time, so this
    /// id is known before the command returns — the coverage panel polls THIS sweep
    /// specifically (never "the latest sweep", which could be a stale/other one) so
    /// its status line and AI-budget footer settle on the sweep the backfill
    /// started, never a false-settle. `None` when nothing was chained (a chain
    /// failure is best-effort, or the backfill itself failed).
    pub chained_sweep_id: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub updated_at: String,
}

/// Next-due snapshot published by the Rust-side source scheduler (ADR 0055, AV5)
/// for the UI to render "next refresh at …". Times are epoch milliseconds so they
/// map directly onto the frontend's existing display. Not persisted: the schedule
/// is in-memory and app-open-only; the scheduler owns the cadence (the frontend no
/// longer decides *when* to refresh — a webview timer is throttled when hidden).
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerStatus {
    /// Per source adapter id → epoch-ms timestamp of its next scheduled refresh.
    pub source_next_due_ms: HashMap<String, i64>,
    /// Epoch-ms of the next company-registry refresh check, if scheduled.
    pub registry_next_due_ms: Option<i64>,
}

/// RAII guard proving the holder is the **sole** worker refreshing a given source
/// adapter (ADR 0059, per-source serialization = exactly one). Acquired via
/// [`AppState::try_acquire_source`]; the source id is released from the in-flight
/// set on `Drop`, so a panic or early return can never leak the lock.
pub struct SourceRefreshGuard {
    adapter_id: String,
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl Drop for SourceRefreshGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = self.in_flight.lock() {
            set.remove(&self.adapter_id);
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    db: Database,
    runtime_metrics: Arc<RuntimeMetricCounters>,
    data_dir: PathBuf,
    backfill_progress: Arc<Mutex<HashMap<String, BackfillProgress>>>,
    scheduler_status: Arc<Mutex<SchedulerStatus>>,
    /// Source adapter ids currently being refreshed by a worker (ADR 0059). The
    /// per-source lock lets multiple source-lane workers run *different* sources
    /// concurrently while guaranteeing at most one touches the *same* source —
    /// politeness (no parallel hammering) with no duplicate work.
    sources_in_flight: Arc<Mutex<HashSet<String>>>,
    /// One concurrency semaphore per AI provider id (ADR 0059). Shared across the
    /// `autopilot` and `ai` lanes so the limit bounds total concurrent calls to a
    /// provider (its cost/rate ceiling), not per-lane. Created lazily on first use.
    provider_semaphores: Arc<Mutex<HashMap<String, Arc<tokio::sync::Semaphore>>>>,
    /// Cross-call failover-cooldown state for capability pools (ADR 0061 decision
    /// 5), shared across the `autopilot` and `ai` lanes so a provider that just
    /// failed stays deprioritized everywhere until its cooldown window elapses.
    provider_cooldowns: crate::providers::analysis::pool::ProviderCooldowns,
}

impl AppState {
    pub fn new(connection: Connection) -> Self {
        Self::with_data_dir(connection, std::env::temp_dir().join("brawler-test-data"))
    }

    pub fn with_data_dir(connection: Connection, data_dir: PathBuf) -> Self {
        let state = Self {
            db: Database::from_connection(connection),
            runtime_metrics: Arc::new(RuntimeMetricCounters::default()),
            data_dir,
            backfill_progress: Arc::new(Mutex::new(HashMap::new())),
            scheduler_status: Arc::new(Mutex::new(SchedulerStatus::default())),
            sources_in_flight: Arc::new(Mutex::new(HashSet::new())),
            provider_semaphores: Arc::new(Mutex::new(HashMap::new())),
            provider_cooldowns: Default::default(),
        };
        state.seed_app_data();
        state
    }

    pub(super) fn with_pool(pool: database::SqlitePool, data_dir: PathBuf) -> Self {
        let state = Self {
            db: Database::from_pool(pool),
            runtime_metrics: Arc::new(RuntimeMetricCounters::default()),
            data_dir,
            backfill_progress: Arc::new(Mutex::new(HashMap::new())),
            scheduler_status: Arc::new(Mutex::new(SchedulerStatus::default())),
            sources_in_flight: Arc::new(Mutex::new(HashSet::new())),
            provider_semaphores: Arc::new(Mutex::new(HashMap::new())),
            provider_cooldowns: Default::default(),
        };
        state.seed_app_data();
        state
    }

    /// Try to claim the per-source refresh lock for `adapter_id` (ADR 0059).
    /// Returns `Some(guard)` when no other worker holds it — the caller is now the
    /// sole refresher and the lock releases when the guard drops. Returns `None`
    /// when the source is already being refreshed; the caller should requeue with
    /// a short backoff and free its worker rather than run duplicate/parallel work.
    pub fn try_acquire_source(&self, adapter_id: &str) -> Option<SourceRefreshGuard> {
        let mut set = self.sources_in_flight.lock().ok()?;
        if set.contains(adapter_id) {
            return None;
        }
        set.insert(adapter_id.to_owned());
        Some(SourceRefreshGuard {
            adapter_id: adapter_id.to_owned(),
            in_flight: Arc::clone(&self.sources_in_flight),
        })
    }

    /// Resolved worker-pool + per-provider concurrency tuning (ADR 0059), read from
    /// settings with tolerant defaults. Read once when the lanes are spawned.
    pub fn queue_config(&self) -> QueueConfig {
        match self.checkout() {
            Ok(connection) => queue_config::read_queue_config(&connection),
            Err(_) => QueueConfig::default(),
        }
    }

    /// The shared concurrency semaphore for one AI provider (ADR 0059), created
    /// lazily with `limit` permits the first time the provider is seen. Shared
    /// across the autopilot + ai lanes, so `limit` bounds total concurrent calls to
    /// that provider — the real AI cost/rate ceiling, independent of thread count.
    /// The limit is fixed at creation (like `db_max_connections`, applied at start);
    /// a later change takes effect on restart.
    pub fn provider_semaphore(
        &self,
        provider_id: &str,
        limit: usize,
    ) -> Arc<tokio::sync::Semaphore> {
        let mut map = self
            .provider_semaphores
            .lock()
            .expect("provider semaphore map poisoned");
        map.entry(provider_id.to_owned())
            .or_insert_with(|| Arc::new(tokio::sync::Semaphore::new(limit.max(1))))
            .clone()
    }

    /// The shared cross-call cooldown state for capability failover pools (ADR
    /// 0061 decision 5). Cheap to clone (an `Arc` around the shared map).
    pub fn provider_cooldowns(&self) -> crate::providers::analysis::pool::ProviderCooldowns {
        self.provider_cooldowns.clone()
    }

    /// Idempotent startup seeding that cannot be expressed as a pure SQL migration
    /// (it derives from Rust constants). Seeds app framework templates (ADR 0046).
    /// Non-fatal: a failure leaves the templates unseeded rather than crashing startup.
    fn seed_app_data(&self) {
        if let Ok(connection) = self.checkout() {
            let _ = quality_frameworks::seed_templates(&connection);
        }
    }

    /// Replace the stored backfill progress for a company.
    pub fn set_backfill_progress(&self, progress: BackfillProgress) {
        let mut guard = self
            .backfill_progress
            .lock()
            .expect("backfill progress mutex poisoned");
        guard.insert(progress.company_id.clone(), progress);
    }

    /// Read the latest backfill progress for a company, if any run has been recorded.
    pub fn get_backfill_progress(&self, company_id: &str) -> Option<BackfillProgress> {
        let guard = self
            .backfill_progress
            .lock()
            .expect("backfill progress mutex poisoned");
        guard.get(company_id).cloned()
    }

    /// Publish the scheduler's next-due snapshot (Rust-side scheduler, ADR 0055).
    pub fn set_scheduler_status(&self, status: SchedulerStatus) {
        let mut guard = self
            .scheduler_status
            .lock()
            .expect("scheduler status mutex poisoned");
        *guard = status;
    }

    /// Read the latest scheduler next-due snapshot for the UI.
    pub fn get_scheduler_status(&self) -> SchedulerStatus {
        self.scheduler_status
            .lock()
            .expect("scheduler status mutex poisoned")
            .clone()
    }

    /// Check out a connection for a single storage operation.
    fn checkout(&self) -> StorageResult<DbGuard<'_>> {
        self.db.checkout()
    }

    /// Test-only raw connection access, for seeding legacy DB shapes the public
    /// creation surface can no longer produce (e.g. pre-0066 period labels).
    #[cfg(test)]
    pub(crate) fn checkout_for_tests(&self) -> StorageResult<DbGuard<'_>> {
        self.checkout()
    }

    /// Watchlist operations as a focused domain store (Architecture v2 / ADR 0050).
    /// Commands that only touch watchlists can depend on this store instead of the
    /// whole `AppState` facade.
    pub fn watchlists(&self) -> watchlists::WatchlistStore {
        watchlists::WatchlistStore::new(self.db.clone())
    }

    /// Research cockpit layout persistence as a focused domain store
    /// (Architecture v2 / ADR 0050; cockpit decision 3A in ADR 0053).
    pub fn cockpit_layouts(&self) -> cockpit_layouts::CockpitLayoutStore {
        cockpit_layouts::CockpitLayoutStore::new(self.db.clone())
    }

    /// Feed operations as a focused domain store (Architecture v2 / ADR 0050).
    pub fn feed(&self) -> feed::FeedStore {
        feed::FeedStore::new(self.db.clone())
    }

    /// Durable job-queue operations as a focused domain store (Architecture v2 /
    /// ADR 0050). The in-process worker (`crate::jobs::queue`) drives it.
    pub fn jobs(&self) -> jobs::JobQueueStore {
        jobs::JobQueueStore::new(self.db.clone())
    }

    /// ai_analysis domain store (Architecture v2 / ADR 0050).
    pub fn ai_analysis(&self) -> ai_analysis::AiAnalysisStore {
        ai_analysis::AiAnalysisStore::new(self.db.clone())
    }

    /// claim_extraction domain store (Architecture v2 / ADR 0050).
    pub fn claim_extraction(&self) -> claim_extraction::ClaimExtractionStore {
        claim_extraction::ClaimExtractionStore::new(self.db.clone())
    }

    /// Autonomous report pipeline domain store (North Star, v0.49.0 / ADR 0055):
    /// per-company trust-ladder mode + autopilot run records.
    pub fn autopilot(&self) -> autopilot::AutopilotStore {
        autopilot::AutopilotStore::new(self.db.clone())
    }

    /// Decision journal domain store (ADR 0071): immutable record of the
    /// user's own judgments; the ADR 0043 workbench extends it later.
    pub fn decision_journal(&self) -> decision_journal::DecisionJournalStore {
        decision_journal::DecisionJournalStore::new(self.db.clone())
    }

    /// diagnostics domain store (Architecture v2 / ADR 0050).
    pub fn diagnostics(&self) -> diagnostics::DiagnosticsStore {
        diagnostics::DiagnosticsStore::new(self.db.clone())
    }

    /// events domain store (Architecture v2 / ADR 0050).
    pub fn events(&self) -> events::EventStore {
        events::EventStore::new(self.db.clone())
    }

    /// financials domain store (Architecture v2 / ADR 0050).
    pub fn financials(&self) -> financials::FinancialsStore {
        financials::FinancialsStore::new(self.db.clone())
    }

    /// Structured-first extraction provenance + per-company profiles (ADR 0061).
    pub fn fundamentals_provenance(&self) -> fundamentals_provenance::FundamentalsProvenanceStore {
        fundamentals_provenance::FundamentalsProvenanceStore::new(self.db.clone())
    }

    /// History sweep records (ADR 0077 §3): the durable backfill/manual
    /// extraction-sweep counterpart to the refresh-time detection sweep.
    pub fn history_sweeps(&self) -> history_sweeps::HistorySweepStore {
        history_sweeps::HistorySweepStore::new(self.db.clone())
    }

    /// import_export domain store (Architecture v2 / ADR 0050).
    pub fn import_export(&self) -> import_export::ImportExportStore {
        import_export::ImportExportStore::new(self.db.clone())
    }

    /// kpi_extraction domain store (Architecture v2 / ADR 0050).
    pub fn kpi_extraction(&self) -> kpi_extraction::KpiExtractionStore {
        kpi_extraction::KpiExtractionStore::new(self.db.clone())
    }

    /// licensing domain store (Architecture v2 / ADR 0050).
    pub fn licensing(&self) -> licensing::LicensingStore {
        licensing::LicensingStore::new(self.db.clone())
    }

    /// management_claims domain store (Architecture v2 / ADR 0050).
    pub fn management_claims(&self) -> management_claims::ManagementClaimStore {
        management_claims::ManagementClaimStore::new(self.db.clone())
    }

    /// notebooks domain store (Architecture v2 / ADR 0050).
    pub fn notebooks(&self) -> notebooks::NotebookStore {
        notebooks::NotebookStore::new(self.db.clone())
    }

    /// quality_frameworks domain store (Architecture v2 / ADR 0050).
    pub fn quality_frameworks(&self) -> quality_frameworks::QualityFrameworkStore {
        quality_frameworks::QualityFrameworkStore::new(self.db.clone())
    }

    /// registry domain store (Architecture v2 / ADR 0050).
    pub fn source_registry(&self) -> registry::SourceRegistryStore {
        registry::SourceRegistryStore::new(self.db.clone())
    }

    /// report_documents domain store (Architecture v2 / ADR 0050).
    pub fn report_documents(&self) -> report_documents::ReportDocumentStore {
        report_documents::ReportDocumentStore::new(self.db.clone())
    }

    pub fn report_sections(&self) -> report_sections::ReportSectionStore {
        report_sections::ReportSectionStore::new(self.db.clone())
    }

    /// Pre-report expectations domain store (ADR 0071): stance + per-metric
    /// expectations for a report occurrence, frozen once the period's facts land.
    pub fn report_expectations(&self) -> report_expectations::ReportExpectationStore {
        report_expectations::ReportExpectationStore::new(self.db.clone())
    }

    /// report_season domain store (Architecture v2 / ADR 0050).
    pub fn report_season(&self) -> report_season::ReportSeasonStore {
        report_season::ReportSeasonStore::new(self.db.clone())
    }

    /// research domain store (Architecture v2 / ADR 0050).
    pub fn research(&self) -> research::ResearchStore {
        research::ResearchStore::new(self.db.clone())
    }

    /// research_briefs domain store (Architecture v2 / ADR 0050).
    pub fn research_briefs(&self) -> research_briefs::ResearchBriefStore {
        research_briefs::ResearchBriefStore::new(self.db.clone())
    }

    /// research_digests domain store (Architecture v2 / ADR 0050).
    pub fn research_digests(&self) -> research_digests::ResearchDigestStore {
        research_digests::ResearchDigestStore::new(self.db.clone())
    }

    /// research_reminders domain store (Architecture v2 / ADR 0050).
    pub fn research_reminders(&self) -> research_reminders::ResearchReminderStore {
        research_reminders::ResearchReminderStore::new(self.db.clone())
    }

    /// settings domain store (Architecture v2 / ADR 0050).
    pub fn settings(&self) -> settings::SettingsStore {
        settings::SettingsStore::new(self.db.clone())
    }

    /// signals domain store (Architecture v2 / ADR 0050).
    pub fn signals(&self) -> signals::SignalStore {
        signals::SignalStore::new(self.db.clone())
    }

    /// KNF short-selling domain store (ADR 0069 decision 3).
    pub fn short_positions(&self) -> short_positions::ShortPositionStore {
        short_positions::ShortPositionStore::new(self.db.clone())
    }

    /// Source-reconciliation domain store (ADR 0069 decision 2, plan v0.55 T3).
    pub fn reconciliation(&self) -> reconciliation::ReconciliationStore {
        reconciliation::ReconciliationStore::new(self.db.clone())
    }

    /// Ownership-stakes domain store (ADR 0072, plan v0.56 T2).
    pub fn ownership(&self) -> ownership::OwnershipStore {
        ownership::OwnershipStore::new(self.db.clone())
    }

    /// Parsed insider-transaction (MAR art. 19) domain store (ADR 0083, plan v0.57 T4).
    pub fn insider(&self) -> insider::InsiderStore {
        insider::InsiderStore::new(self.db.clone())
    }

    /// Parsed management-holdings domain store + founder stamping (ADR 0083, plan v0.57 T5).
    pub fn management_holdings(&self) -> management_holdings::ManagementHoldingsStore {
        management_holdings::ManagementHoldingsStore::new(self.db.clone())
    }

    /// Company red-flags domain store (ADR 0083, plan v0.57 T7).
    pub fn red_flags(&self) -> red_flags::RedFlagStore {
        red_flags::RedFlagStore::new(self.db.clone())
    }

    /// Attention (alert rules + attention events) domain store (ADR 0068).
    pub fn attention(&self) -> attention::AttentionStore {
        attention::AttentionStore::new(self.db.clone())
    }

    /// Morning-briefing domain store (ADR 0068 decision 4).
    pub fn morning_briefings(&self) -> morning_briefings::MorningBriefingStore {
        morning_briefings::MorningBriefingStore::new(self.db.clone())
    }

    /// sources domain store (Architecture v2 / ADR 0050).
    pub fn sources(&self) -> sources::SourcesStore {
        sources::SourcesStore::new(self.db.clone())
    }

    /// transcripts domain store (Architecture v2 / ADR 0050).
    pub fn transcripts(&self) -> transcripts::TranscriptStore {
        transcripts::TranscriptStore::new(self.db.clone())
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
        let connection = self.checkout()?;

        metrics::collect_local_metrics_snapshot(&connection, &self.runtime_metrics, app_data_dir)
    }

    pub fn database_status(&self) -> StorageResult<DatabaseStatus> {
        let connection = self.checkout()?;

        migrations::database_status(&connection)
    }

    pub fn backup_status(&self) -> StorageResult<BackupStatus> {
        backup::collect_status(&self.data_dir)
    }

    pub fn create_backup(&self) -> StorageResult<BackupStatus> {
        let connection = self.checkout()?;

        backup::create_rotating_backup(&connection, &self.data_dir)
    }

    pub fn request_restore(&self, file_name: &str) -> StorageResult<()> {
        backup::request_restore(&self.data_dir, file_name)
    }

    /// Company operations as a focused domain store (Architecture v2 / ADR 0050).
    pub fn companies(&self) -> companies::CompanyStore {
        companies::CompanyStore::new(self.db.clone())
    }

    /// Market-data (`daily_quotes`) domain store (ADR 0067 / ADR 0082).
    pub fn market_data(&self) -> market_data::MarketDataStore {
        market_data::MarketDataStore::new(self.db.clone())
    }

    pub fn list_companies(&self) -> StorageResult<Vec<Company>> {
        self.companies().list_companies()
    }

    pub fn search(
        &self,
        query: &str,
        content_types: &[String],
        company_id: Option<&str>,
        limit: i64,
    ) -> StorageResult<Vec<SearchMatch>> {
        let connection = self.checkout()?;

        search::run_search(&connection, query, content_types, company_id, limit)
    }

    pub fn create_company(&self, input: NewCompany) -> StorageResult<Company> {
        self.companies().create_company(input)
    }

    pub fn get_company_ir_reports_url(&self, company_id: &str) -> StorageResult<Option<String>> {
        self.companies().get_company_ir_reports_url(company_id)
    }

    pub fn set_company_ir_reports_url(
        &self,
        company_id: &str,
        url: Option<&str>,
    ) -> StorageResult<Option<String>> {
        self.companies().set_company_ir_reports_url(company_id, url)
    }

    pub fn get_company_sector(
        &self,
        company_id: &str,
    ) -> StorageResult<(Option<String>, Option<String>)> {
        self.companies().get_company_sector(company_id)
    }

    pub fn set_company_sector(
        &self,
        company_id: &str,
        sector: Option<&str>,
    ) -> StorageResult<Option<String>> {
        self.companies().set_company_sector(company_id, sector)
    }

    pub fn list_company_sectors(&self) -> StorageResult<Vec<String>> {
        self.companies().list_company_sectors()
    }

    pub fn latest_shares_outstanding(
        &self,
        company_id: &str,
    ) -> StorageResult<Option<(String, String)>> {
        self.companies().latest_shares_outstanding(company_id)
    }

    pub fn lookup_company(
        &self,
        input: CompanyLookupInput,
    ) -> StorageResult<Option<CompanyLookupResult>> {
        self.companies().lookup_company(input)
    }

    pub fn company_directories_need_bootstrap_refresh(&self) -> StorageResult<bool> {
        self.companies()
            .company_directories_need_bootstrap_refresh()
    }

    pub fn company_directories_are_stale(&self, stale_after_seconds: i64) -> StorageResult<bool> {
        self.companies()
            .company_directories_are_stale(stale_after_seconds)
    }

    pub fn refresh_gpw_company_registry(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        self.companies()
            .refresh_gpw_company_registry(entries, fetched_at)
    }

    pub fn refresh_newconnect_company_directory(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        self.companies()
            .refresh_newconnect_company_directory(entries, fetched_at)
    }

    pub fn delete_company(&self, company_id: &str) -> StorageResult<()> {
        self.companies().delete_company(company_id)
    }

    pub fn export_research_data(&self) -> StorageResult<ExportPayload> {
        self.import_export().export_research_data()
    }

    pub fn preview_research_import(&self, contents: &str) -> StorageResult<ImportPreview> {
        self.import_export().preview_research_import(contents)
    }

    pub fn apply_research_import(&self, contents: &str) -> StorageResult<ImportApplyResult> {
        self.import_export().apply_research_import(contents)
    }

    pub fn export_settings_data(&self) -> StorageResult<ExportPayload> {
        self.import_export().export_settings_data()
    }

    pub fn preview_settings_import(&self, contents: &str) -> StorageResult<ImportPreview> {
        import_export::preview_settings_import(contents)
    }

    pub fn apply_settings_import(&self, contents: &str) -> StorageResult<ImportApplyResult> {
        self.import_export().apply_settings_import(contents)
    }

    // Watchlist operations delegate to `WatchlistStore` (Architecture v2 / ADR
    // 0050). These thin pass-throughs keep existing call sites working while the
    // facade is dissolved incrementally; new code should prefer `watchlists()`.
    pub fn list_watchlists(&self) -> StorageResult<Vec<Watchlist>> {
        self.watchlists().list_watchlists()
    }

    pub fn list_watchlist_memberships(&self) -> StorageResult<Vec<WatchlistMembership>> {
        self.watchlists().list_watchlist_memberships()
    }

    pub fn create_watchlist(&self, input: NewWatchlist) -> StorageResult<Watchlist> {
        self.watchlists().create_watchlist(input)
    }

    pub fn rename_watchlist(&self, input: WatchlistUpdate) -> StorageResult<Watchlist> {
        self.watchlists().rename_watchlist(input)
    }

    pub fn delete_watchlist(&self, watchlist_id: &str) -> StorageResult<()> {
        self.watchlists().delete_watchlist(watchlist_id)
    }

    pub fn add_company_to_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        self.watchlists().add_company_to_watchlist(input)
    }

    pub fn remove_company_from_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        self.watchlists().remove_company_from_watchlist(input)
    }

    // Feed operations delegate to `FeedStore` (Architecture v2 / ADR 0050).
    pub fn list_feed_items(&self) -> StorageResult<Vec<FeedItem>> {
        self.feed().list_feed_items()
    }

    pub fn list_unmatched_source_items(
        &self,
        adapter_id: &str,
    ) -> StorageResult<Vec<UnmatchedSourceItem>> {
        self.feed().list_unmatched_source_items(adapter_id)
    }

    pub fn list_research_evidence(
        &self,
        input: ResearchEvidenceInput,
    ) -> StorageResult<ResearchTimelineResult> {
        self.research().list_research_evidence(input)
    }

    pub fn mark_research_scope_reviewed(
        &self,
        input: ResearchReviewCheckpointInput,
    ) -> StorageResult<ResearchReviewCheckpoint> {
        self.research().mark_research_scope_reviewed(input)
    }

    pub fn list_research_review_state(
        &self,
        input: ResearchReviewCheckpointInput,
    ) -> StorageResult<Option<ResearchReviewCheckpoint>> {
        self.research().list_research_review_state(input)
    }

    pub fn list_research_questions(
        &self,
        input: ResearchQuestionListInput,
    ) -> StorageResult<Vec<ResearchQuestion>> {
        self.research().list_research_questions(input)
    }

    pub fn create_research_question(
        &self,
        input: NewResearchQuestion,
    ) -> StorageResult<ResearchQuestion> {
        self.research().create_research_question(input)
    }

    pub fn update_research_question(
        &self,
        input: ResearchQuestionUpdate,
    ) -> StorageResult<ResearchQuestion> {
        self.research().update_research_question(input)
    }

    pub fn delete_research_question(&self, id: &str) -> StorageResult<()> {
        self.research().delete_research_question(id)
    }

    pub fn create_evidence_link(&self, input: NewEvidenceLink) -> StorageResult<EvidenceLink> {
        self.research().create_evidence_link(input)
    }

    pub fn list_evidence_links(
        &self,
        input: EvidenceLinkListInput,
    ) -> StorageResult<Vec<EvidenceLink>> {
        self.research().list_evidence_links(input)
    }

    pub fn delete_evidence_link(&self, id: &str) -> StorageResult<()> {
        self.research().delete_evidence_link(id)
    }

    pub fn list_research_reminders(
        &self,
        input: ResearchReminderListInput,
    ) -> StorageResult<Vec<ResearchReminder>> {
        self.research_reminders().list_research_reminders(input)
    }

    pub fn create_research_reminder(
        &self,
        input: NewResearchReminder,
    ) -> StorageResult<ResearchReminder> {
        self.research_reminders().create_research_reminder(input)
    }

    pub fn update_research_reminder(
        &self,
        input: ResearchReminderUpdate,
    ) -> StorageResult<ResearchReminder> {
        self.research_reminders().update_research_reminder(input)
    }

    pub fn delete_research_reminder(&self, id: &str) -> StorageResult<()> {
        self.research_reminders().delete_research_reminder(id)
    }

    pub fn create_research_brief_job(
        &self,
        input: NewResearchBriefJob,
    ) -> StorageResult<ResearchBriefJob> {
        self.research_briefs().create_research_brief_job(input)
    }

    pub fn list_research_brief_jobs(
        &self,
        input: ResearchBriefScopeInput,
    ) -> StorageResult<Vec<ResearchBriefJob>> {
        self.research_briefs().list_research_brief_jobs(input)
    }

    pub fn get_research_brief_job(&self, job_id: &str) -> StorageResult<ResearchBriefJob> {
        self.research_briefs().get_research_brief_job(job_id)
    }

    pub fn collect_research_brief_evidence(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchBriefEvidenceContext> {
        self.research_briefs()
            .collect_research_brief_evidence(job_id)
    }

    pub fn mark_research_brief_job_running(&self, job_id: &str) -> StorageResult<ResearchBriefJob> {
        self.research_briefs()
            .mark_research_brief_job_running(job_id)
    }

    pub fn complete_research_brief_job(
        &self,
        input: CompletedResearchBrief,
    ) -> StorageResult<ResearchBriefJob> {
        self.research_briefs().complete_research_brief_job(input)
    }

    pub fn mark_research_brief_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ResearchBriefJob> {
        self.research_briefs()
            .mark_research_brief_job_failed(job_id, error_code, error)
    }

    pub fn create_research_digest_job(
        &self,
        input: NewResearchDigestJob,
    ) -> StorageResult<ResearchDigestJob> {
        self.research_digests().create_research_digest_job(input)
    }

    pub fn list_research_digest_jobs(
        &self,
        input: ResearchDigestScopeInput,
    ) -> StorageResult<Vec<ResearchDigestJob>> {
        self.research_digests().list_research_digest_jobs(input)
    }

    pub fn get_research_digest_job(&self, job_id: &str) -> StorageResult<ResearchDigestJob> {
        self.research_digests().get_research_digest_job(job_id)
    }

    pub fn collect_research_digest_evidence(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchDigestEvidenceContext> {
        self.research_digests()
            .collect_research_digest_evidence(job_id)
    }

    pub fn mark_research_digest_job_running(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchDigestJob> {
        self.research_digests()
            .mark_research_digest_job_running(job_id)
    }

    pub fn complete_research_digest_job(
        &self,
        input: CompletedResearchDigest,
    ) -> StorageResult<ResearchDigestJob> {
        self.research_digests().complete_research_digest_job(input)
    }

    pub fn mark_research_digest_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ResearchDigestJob> {
        self.research_digests()
            .mark_research_digest_job_failed(job_id, error_code, error)
    }

    pub fn ingest_gpw_report_listings(
        &self,
        listings: &[GpwReportListing],
    ) -> StorageResult<SourceIngestionResult> {
        self.sources().ingest_gpw_report_listings(listings)
    }

    pub fn ingest_bankier_rss_items(
        &self,
        items: &[BankierRssItem],
    ) -> StorageResult<SourceIngestionResult> {
        self.sources().ingest_bankier_rss_items(items)
    }

    pub fn list_bankier_company_targets(&self) -> StorageResult<Vec<BankierCompanyTarget>> {
        self.sources().list_bankier_company_targets()
    }

    pub fn backfill_market_status(
        &self,
        company_id: &str,
    ) -> StorageResult<sources::BackfillMarketStatus> {
        self.sources().backfill_market_status(company_id)
    }

    /// Delete `espi_attachment` report documents mis-associated onto the wrong
    /// company by tag-listing ingestion (T-A3, card 45fcece). Idempotent startup
    /// self-heal; returns the number of rows removed.
    pub fn repair_misassociated_report_documents(&self) -> StorageResult<usize> {
        self.sources().repair_misassociated_report_documents()
    }

    pub fn upsert_bankier_company_identifiers(
        &self,
        company_id: &str,
        identifiers: &BankierCompanyIdentifiers,
    ) -> StorageResult<()> {
        self.sources()
            .upsert_bankier_company_identifiers(company_id, identifiers)
    }

    pub fn list_bankier_company_detail_cached_urls(&self) -> StorageResult<Vec<String>> {
        self.sources().list_bankier_company_detail_cached_urls()
    }

    pub fn ingest_bankier_company_items(
        &self,
        items: &[BankierCompanyItem],
    ) -> StorageResult<SourceIngestionResult> {
        self.sources().ingest_bankier_company_items(items)
    }

    pub fn ingest_gpw_market_event_items(
        &self,
        items: &[GpwMarketEventItem],
    ) -> StorageResult<SourceIngestionResult> {
        self.events().ingest_gpw_market_event_items(items)
    }

    pub fn ingest_bankier_calendar_event_items(
        &self,
        items: &[BankierCalendarEventItem],
    ) -> StorageResult<SourceIngestionResult> {
        self.events().ingest_bankier_calendar_event_items(items)
    }

    pub fn tracks_gpw_listing_company(&self, ticker: &str, isin: &str) -> StorageResult<bool> {
        let connection = self.checkout()?;

        Ok(feed_matching::find_company_for_gpw_listing(&connection, ticker, isin)?.is_some())
    }

    pub fn update_feed_item_state(&self, input: FeedItemStateInput) -> StorageResult<FeedItem> {
        self.feed().update_feed_item_state(input)
    }

    pub fn get_feed_item(&self, feed_item_id: &str) -> StorageResult<FeedItem> {
        self.feed().get_feed_item(feed_item_id)
    }

    pub fn prune_old_feed_items(&self, retention_days: i64) -> StorageResult<FeedPruneResult> {
        self.feed().prune_old_feed_items(retention_days)
    }

    pub fn delete_unsaved_feed_items(&self) -> StorageResult<FeedDeleteResult> {
        self.feed().delete_unsaved_feed_items()
    }

    pub fn list_notebook_entries(&self, company_id: &str) -> StorageResult<Vec<NotebookEntry>> {
        self.notebooks().list_notebook_entries(company_id)
    }

    pub fn create_notebook_entry(&self, input: NewNotebookEntry) -> StorageResult<NotebookEntry> {
        self.notebooks().create_notebook_entry(input)
    }

    pub fn create_note_from_transcript_selection(
        &self,
        input: CreateNoteFromTranscriptSelectionInput,
    ) -> StorageResult<NotebookEntry> {
        self.transcripts()
            .create_note_from_transcript_selection(input)
    }

    pub fn update_notebook_entry(
        &self,
        input: NotebookEntryUpdate,
    ) -> StorageResult<NotebookEntry> {
        self.notebooks().update_notebook_entry(input)
    }

    pub fn delete_notebook_entry(&self, notebook_entry_id: &str) -> StorageResult<()> {
        self.notebooks().delete_notebook_entry(notebook_entry_id)
    }

    pub fn list_management_claims(&self, company_id: &str) -> StorageResult<Vec<ManagementClaim>> {
        self.management_claims().list_management_claims(company_id)
    }

    pub fn create_management_claim(
        &self,
        input: NewManagementClaim,
    ) -> StorageResult<ManagementClaim> {
        self.management_claims().create_management_claim(input)
    }

    pub fn update_management_claim(
        &self,
        input: ManagementClaimUpdate,
    ) -> StorageResult<ManagementClaim> {
        self.management_claims().update_management_claim(input)
    }

    pub fn set_claim_verdict(&self, input: SetClaimVerdictInput) -> StorageResult<ManagementClaim> {
        self.management_claims().set_claim_verdict(input)
    }

    pub fn delete_management_claim(&self, claim_id: &str) -> StorageResult<()> {
        self.management_claims().delete_management_claim(claim_id)
    }

    pub fn list_claims_to_verify(&self, company_id: &str) -> StorageResult<ClaimsToVerify> {
        self.management_claims().list_claims_to_verify(company_id)
    }

    // ---- Quality frameworks (ADR 0046) -----------------------------------

    pub fn list_quality_frameworks(&self) -> StorageResult<Vec<QualityFramework>> {
        self.quality_frameworks().list_quality_frameworks()
    }

    pub fn get_quality_framework(&self, id: &str) -> StorageResult<QualityFramework> {
        self.quality_frameworks().get_quality_framework(id)
    }

    pub fn create_quality_framework(
        &self,
        input: NewQualityFramework,
    ) -> StorageResult<QualityFramework> {
        self.quality_frameworks().create_quality_framework(input)
    }

    pub fn update_quality_framework(
        &self,
        input: UpdateQualityFramework,
    ) -> StorageResult<QualityFramework> {
        self.quality_frameworks().update_quality_framework(input)
    }

    pub fn delete_quality_framework(&self, id: &str) -> StorageResult<()> {
        self.quality_frameworks().delete_quality_framework(id)
    }

    pub fn clone_framework(&self, input: CloneFrameworkInput) -> StorageResult<QualityFramework> {
        self.quality_frameworks().clone_framework(input)
    }

    pub fn reset_framework_to_template(&self, id: &str) -> StorageResult<QualityFramework> {
        self.quality_frameworks().reset_framework_to_template(id)
    }

    pub fn create_framework_criterion(
        &self,
        input: NewFrameworkCriterion,
    ) -> StorageResult<FrameworkCriterion> {
        self.quality_frameworks().create_framework_criterion(input)
    }

    pub fn update_framework_criterion(
        &self,
        input: UpdateFrameworkCriterion,
    ) -> StorageResult<FrameworkCriterion> {
        self.quality_frameworks().update_framework_criterion(input)
    }

    pub fn delete_framework_criterion(&self, id: &str) -> StorageResult<()> {
        self.quality_frameworks().delete_framework_criterion(id)
    }

    pub fn validate_criterion_expression(&self, expression: &str) -> ValidateCriterionResult {
        quality_frameworks::validate_criterion_expression(expression)
    }

    pub fn evaluate_framework(
        &self,
        input: EvaluateFrameworkInput,
    ) -> StorageResult<FrameworkEvaluation> {
        self.quality_frameworks().evaluate_framework(input)
    }

    pub fn persist_qualitative_assessment(
        &self,
        input: PersistQualitativeAssessmentInput,
    ) -> StorageResult<FrameworkEvaluation> {
        self.quality_frameworks()
            .persist_qualitative_assessment(input)
    }

    pub fn list_framework_evaluations(
        &self,
        input: ListFrameworkEvaluationsInput,
    ) -> StorageResult<Vec<FrameworkEvaluation>> {
        self.quality_frameworks().list_framework_evaluations(input)
    }

    pub fn get_framework_evaluation(&self, id: &str) -> StorageResult<FrameworkEvaluation> {
        self.quality_frameworks().get_framework_evaluation(id)
    }

    pub fn get_qualitative_assessment(
        &self,
        framework_id: &str,
        company_id: &str,
    ) -> StorageResult<Vec<CriterionResult>> {
        self.quality_frameworks()
            .get_qualitative_assessment(framework_id, company_id)
    }

    pub fn qualitative_verdict_changes(
        &self,
        framework_id: &str,
        company_id: &str,
    ) -> StorageResult<Vec<QualitativeVerdictChange>> {
        self.quality_frameworks()
            .qualitative_verdict_changes(framework_id, company_id)
    }

    pub fn frameworks_with_qualitative_assessments(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<AssessedFrameworkRef>> {
        self.quality_frameworks()
            .frameworks_with_qualitative_assessments(company_id)
    }

    pub fn delete_framework_evaluation(&self, id: &str) -> StorageResult<()> {
        self.quality_frameworks().delete_framework_evaluation(id)
    }

    pub fn list_available_metric_keys(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<MetricKeyInfo>> {
        self.quality_frameworks()
            .list_available_metric_keys(company_id)
    }

    pub fn list_report_season(
        &self,
        input: ReportSeasonInput,
    ) -> StorageResult<ReportSeasonResult> {
        self.report_season().list_report_season(input)
    }

    pub fn get_pre_report_card(&self, input: PreReportCardInput) -> StorageResult<PreReportCard> {
        self.report_season().get_pre_report_card(input)
    }

    pub fn mark_report_prepared(
        &self,
        input: MarkReportPreparedInput,
    ) -> StorageResult<ReportPreparation> {
        self.report_season().mark_report_prepared(input)
    }

    pub fn mark_report_processed(
        &self,
        input: MarkReportProcessedInput,
    ) -> StorageResult<ReportPreparation> {
        self.report_season().mark_report_processed(input)
    }

    pub fn create_claim_extraction_job(
        &self,
        input: NewClaimExtractionJob,
    ) -> StorageResult<ClaimExtractionJob> {
        self.claim_extraction().create_claim_extraction_job(input)
    }

    pub fn get_claim_extraction_job(&self, job_id: &str) -> StorageResult<ClaimExtractionJob> {
        self.claim_extraction().get_claim_extraction_job(job_id)
    }

    pub fn list_claim_extraction_jobs_by_source(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> StorageResult<Vec<ClaimExtractionJob>> {
        self.claim_extraction()
            .list_claim_extraction_jobs_by_source(source_type, source_id)
    }

    pub fn mark_claim_extraction_job_running(
        &self,
        job_id: &str,
    ) -> StorageResult<ClaimExtractionJob> {
        self.claim_extraction()
            .mark_claim_extraction_job_running(job_id)
    }

    pub fn mark_claim_extraction_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ClaimExtractionJob> {
        self.claim_extraction()
            .mark_claim_extraction_job_failed(job_id, error_code, error)
    }

    pub fn complete_claim_extraction_job(
        &self,
        input: CompletedClaimExtraction,
    ) -> StorageResult<ClaimExtractionJob> {
        self.claim_extraction().complete_claim_extraction_job(input)
    }

    pub fn confirm_claim_proposal(
        &self,
        input: ConfirmClaimProposalInput,
    ) -> StorageResult<ManagementClaim> {
        self.claim_extraction().confirm_claim_proposal(input)
    }

    pub fn reject_claim_proposal(&self, proposal_id: &str) -> StorageResult<ClaimExtractionJob> {
        self.claim_extraction().reject_claim_proposal(proposal_id)
    }

    pub fn list_company_events(
        &self,
        input: CompanyEventListInput,
    ) -> StorageResult<Vec<CompanyEvent>> {
        self.events().list_company_events(input)
    }

    pub fn create_company_event(&self, input: NewCompanyEvent) -> StorageResult<CompanyEvent> {
        self.events().create_company_event(input)
    }

    pub fn list_company_signals(
        &self,
        input: CompanySignalListInput,
    ) -> StorageResult<Vec<CompanySignal>> {
        self.signals().list_company_signals(input)
    }

    pub fn propose_company_signal(&self, input: ProposedSignalInput) -> StorageResult<bool> {
        self.signals().propose_company_signal(input)
    }

    pub fn list_signal_categories_for_ai(&self) -> StorageResult<Vec<SignalCategorySummary>> {
        self.signals().list_signal_categories_for_ai()
    }

    pub fn list_unclassified_official_filings(
        &self,
        source_adapter_id: &str,
        limit: i64,
    ) -> StorageResult<Vec<UnclassifiedFiling>> {
        self.signals()
            .list_unclassified_official_filings(source_adapter_id, limit)
    }

    pub fn confirm_company_signal(&self, signal_id: &str) -> StorageResult<CompanySignal> {
        self.signals().confirm_company_signal(signal_id)
    }

    pub fn reject_company_signal(&self, signal_id: &str) -> StorageResult<()> {
        self.signals().reject_company_signal(signal_id)
    }

    /// Confirm (`confirm = true`) or reject a `proposed` derived calendar event (ADR 0036).
    pub fn confirm_derived_event(&self, event_id: &str, confirm: bool) -> StorageResult<()> {
        self.signals().confirm_derived_event(event_id, confirm)
    }

    /// Confirmed dividend / general-meeting signals still lacking a derived event — candidates
    /// for the opt-in AI date-extraction fallback (ADR 0036).
    pub fn list_signals_needing_event_date(
        &self,
        limit: i64,
    ) -> StorageResult<Vec<signals::SignalNeedingDate>> {
        self.signals().list_signals_needing_event_date(limit)
    }

    /// Derive a `proposed` event from an AI-extracted date for one signal (ADR 0036).
    pub fn derive_event_from_extracted_date(
        &self,
        signal_id: &str,
        event_date: &str,
    ) -> StorageResult<bool> {
        self.signals()
            .derive_event_from_extracted_date(signal_id, event_date)
    }

    pub fn list_transcript_jobs(
        &self,
        input: TranscriptJobListInput,
    ) -> StorageResult<Vec<TranscriptJob>> {
        self.transcripts().list_transcript_jobs(input)
    }

    pub fn delete_transcript_job(&self, job_id: &str) -> StorageResult<()> {
        self.transcripts().delete_transcript_job(job_id)
    }

    pub fn create_transcript_job(&self, input: NewTranscriptJob) -> StorageResult<TranscriptJob> {
        self.transcripts().create_transcript_job(input)
    }

    pub fn update_transcript_job(
        &self,
        input: UpdateTranscriptJobInput,
    ) -> StorageResult<TranscriptJob> {
        self.transcripts().update_transcript_job(input)
    }

    pub fn list_transcript_segments(
        &self,
        transcript_job_id: &str,
    ) -> StorageResult<Vec<TranscriptSegment>> {
        self.transcripts()
            .list_transcript_segments(transcript_job_id)
    }

    pub fn create_transcript_segment(
        &self,
        input: NewTranscriptSegment,
    ) -> StorageResult<TranscriptSegment> {
        self.transcripts().create_transcript_segment(input)
    }

    pub fn resolve_transcript_job_company(
        &self,
        input: ResolveTranscriptJobCompanyInput,
    ) -> StorageResult<TranscriptJob> {
        self.transcripts().resolve_transcript_job_company(input)
    }

    pub fn get_transcript_job(&self, job_id: &str) -> StorageResult<TranscriptJob> {
        self.transcripts().get_transcript_job(job_id)
    }

    pub fn mark_transcript_job_running(&self, job_id: &str) -> StorageResult<TranscriptJob> {
        self.transcripts().mark_transcript_job_running(job_id)
    }

    pub fn mark_transcript_job_completed(&self, job_id: &str) -> StorageResult<TranscriptJob> {
        self.transcripts().mark_transcript_job_completed(job_id)
    }

    pub fn mark_transcript_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<TranscriptJob> {
        self.transcripts()
            .mark_transcript_job_failed(job_id, error_code, error)
    }

    pub fn list_source_adapters(&self) -> StorageResult<Vec<SourceAdapter>> {
        self.list_source_adapters_with_developer(false)
    }

    pub fn list_source_adapters_with_developer(
        &self,
        include_developer_only: bool,
    ) -> StorageResult<Vec<SourceAdapter>> {
        self.source_registry()
            .list_source_adapters_with_developer(include_developer_only)
    }

    pub fn set_source_adapter_enabled(
        &self,
        adapter_id: &str,
        enabled: bool,
    ) -> StorageResult<SourceAdapter> {
        self.source_registry()
            .set_source_adapter_enabled(adapter_id, enabled)
    }

    pub fn source_adapter_enabled(&self, adapter_id: &str) -> StorageResult<bool> {
        self.source_registry().source_adapter_enabled(adapter_id)
    }

    pub fn list_company_registry_entries(&self) -> StorageResult<Vec<CompanyRegistryEntry>> {
        self.source_registry().list_company_registry_entries()
    }

    pub fn record_source_adapter_error(&self, adapter_id: &str, error: &str) -> StorageResult<()> {
        self.source_registry()
            .record_source_adapter_error(adapter_id, error)
    }

    pub fn record_source_adapter_attempt(
        &self,
        adapter_id: &str,
        trigger: &str,
    ) -> StorageResult<()> {
        self.source_registry()
            .record_source_adapter_attempt(adapter_id, trigger)
    }

    pub fn record_source_adapter_state(
        &self,
        adapter_id: &str,
        key: &str,
        value: &str,
    ) -> StorageResult<()> {
        self.sources()
            .record_source_adapter_state(adapter_id, key, value)
    }

    pub fn get_source_adapter_state(
        &self,
        adapter_id: &str,
        key: &str,
    ) -> StorageResult<Option<String>> {
        self.sources().get_source_adapter_state(adapter_id, key)
    }

    pub fn get_settings(&self) -> StorageResult<UserSettings> {
        self.settings().get_settings()
    }

    pub fn update_settings(&self, input: SettingsUpdate) -> StorageResult<UserSettings> {
        self.settings().update_settings(input)
    }

    pub fn set_developer_mode_enabled(&self, enabled: bool) -> StorageResult<UserSettings> {
        self.settings().set_developer_mode_enabled(enabled)
    }

    /// Read the legacy similarity-strategy setting, mapping the retired
    /// `embedding` value to `static` (ADR 0080): the embedding model is gone,
    /// so `static` is the only strategy — but an old database may still hold
    /// the `embedding` row and must never error or resurrect the old behavior.
    pub fn get_similarity_strategy(&self) -> StorageResult<String> {
        self.settings().get_similarity_strategy()
    }

    pub fn get_license_metadata(&self) -> StorageResult<Option<StoredLicenseMetadata>> {
        self.licensing().get_license_metadata()
    }

    pub fn upsert_license_metadata(
        &self,
        input: LicenseMetadataUpdate,
    ) -> StorageResult<StoredLicenseMetadata> {
        self.licensing().upsert_license_metadata(input)
    }

    pub fn clear_license_metadata(&self) -> StorageResult<()> {
        self.licensing().clear_license_metadata()
    }

    pub fn record_diagnostic_event(
        &self,
        input: NewDiagnosticEvent,
    ) -> StorageResult<Option<DiagnosticEvent>> {
        self.diagnostics().record_diagnostic_event(input)
    }

    pub fn list_diagnostic_events(&self, limit: i64) -> StorageResult<Vec<DiagnosticEvent>> {
        self.diagnostics().list_diagnostic_events(limit)
    }

    pub fn clear_diagnostic_events(&self) -> StorageResult<usize> {
        self.diagnostics().clear_diagnostic_events()
    }

    pub fn create_ai_analysis_job(&self, input: NewAiAnalysisJob) -> StorageResult<AiAnalysisJob> {
        self.ai_analysis().create_ai_analysis_job(input)
    }

    pub fn list_ai_analysis_jobs(&self, feed_item_id: &str) -> StorageResult<Vec<AiAnalysisJob>> {
        self.ai_analysis().list_ai_analysis_jobs(feed_item_id)
    }

    pub fn get_ai_analysis_job(&self, job_id: &str) -> StorageResult<AiAnalysisJob> {
        self.ai_analysis().get_ai_analysis_job(job_id)
    }

    pub fn mark_ai_analysis_job_running(&self, job_id: &str) -> StorageResult<AiAnalysisJob> {
        self.ai_analysis().mark_ai_analysis_job_running(job_id)
    }

    pub fn complete_ai_analysis_job(
        &self,
        input: CompletedAiAnalysis,
    ) -> StorageResult<AiAnalysisJob> {
        self.ai_analysis().complete_ai_analysis_job(input)
    }

    pub fn mark_ai_analysis_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<AiAnalysisJob> {
        self.ai_analysis()
            .mark_ai_analysis_job_failed(job_id, error_code, error)
    }

    pub fn list_kpi_definitions(
        &self,
        input: ListKpiDefinitionsInput,
    ) -> StorageResult<Vec<KpiDefinition>> {
        self.financials().list_kpi_definitions(input)
    }

    pub fn create_kpi_definition(&self, input: NewKpiDefinition) -> StorageResult<KpiDefinition> {
        self.financials().create_kpi_definition(input)
    }

    pub fn list_financial_periods(
        &self,
        input: ListFinancialPeriodsInput,
    ) -> StorageResult<Vec<FinancialPeriod>> {
        self.financials().list_financial_periods(input)
    }

    pub fn create_financial_period(
        &self,
        input: NewFinancialPeriod,
    ) -> StorageResult<FinancialPeriod> {
        self.financials().create_financial_period(input)
    }

    pub fn update_financial_period(
        &self,
        input: UpdateFinancialPeriod,
    ) -> StorageResult<FinancialPeriod> {
        self.financials().update_financial_period(input)
    }

    pub fn delete_financial_period(&self, id: &str) -> StorageResult<()> {
        self.financials().delete_financial_period(id)
    }

    pub fn list_kpi_relevance(&self, company_id: &str) -> StorageResult<Vec<KpiRelevance>> {
        self.financials().list_kpi_relevance(company_id)
    }

    pub fn create_kpi_relevance(&self, input: NewKpiRelevance) -> StorageResult<KpiRelevance> {
        self.financials().create_kpi_relevance(input)
    }

    pub fn update_kpi_relevance(&self, input: UpdateKpiRelevance) -> StorageResult<KpiRelevance> {
        self.financials().update_kpi_relevance(input)
    }

    pub fn delete_kpi_relevance(&self, id: &str) -> StorageResult<()> {
        self.financials().delete_kpi_relevance(id)
    }

    pub fn list_financial_facts(
        &self,
        input: ListFinancialFactsInput,
    ) -> StorageResult<Vec<FinancialFact>> {
        self.financials().list_financial_facts(input)
    }

    pub fn create_financial_fact(&self, input: NewFinancialFact) -> StorageResult<FinancialFact> {
        self.financials().create_financial_fact(input)
    }

    pub fn create_kpi_extraction_job(
        &self,
        input: NewKpiExtractionJob,
    ) -> StorageResult<KpiExtractionJob> {
        self.kpi_extraction().create_kpi_extraction_job(input)
    }

    pub fn get_kpi_extraction_job(&self, job_id: &str) -> StorageResult<KpiExtractionJob> {
        self.kpi_extraction().get_kpi_extraction_job(job_id)
    }

    pub fn list_kpi_extraction_jobs_by_document(
        &self,
        report_document_id: &str,
    ) -> StorageResult<Vec<KpiExtractionJob>> {
        self.kpi_extraction()
            .list_kpi_extraction_jobs_by_document(report_document_id)
    }

    pub fn mark_kpi_extraction_job_running(&self, job_id: &str) -> StorageResult<KpiExtractionJob> {
        self.kpi_extraction()
            .mark_kpi_extraction_job_running(job_id)
    }

    pub fn mark_kpi_extraction_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<KpiExtractionJob> {
        self.kpi_extraction()
            .mark_kpi_extraction_job_failed(job_id, error_code, error)
    }

    pub fn complete_kpi_extraction_job(
        &self,
        input: CompletedKpiExtraction,
    ) -> StorageResult<KpiExtractionJob> {
        self.kpi_extraction().complete_kpi_extraction_job(input)
    }

    pub fn confirm_kpi_proposal(
        &self,
        input: ConfirmKpiProposalInput,
    ) -> StorageResult<ConfirmedKpiFact> {
        self.kpi_extraction().confirm_kpi_proposal(input)
    }

    pub fn reject_kpi_proposal(&self, proposal_id: &str) -> StorageResult<KpiExtractionProposal> {
        self.kpi_extraction().reject_kpi_proposal(proposal_id)
    }

    /// Auto-confirm a KPI proposal as an `auto_unreviewed` fact (autopilot path,
    /// ADR 0055). Used by the extract stage of an autopilot run.
    pub fn autopilot_auto_confirm_proposal(
        &self,
        proposal_id: &str,
    ) -> StorageResult<FinancialFact> {
        self.kpi_extraction().auto_confirm_kpi_proposal(proposal_id)
    }

    pub fn update_financial_fact(
        &self,
        input: UpdateFinancialFact,
    ) -> StorageResult<FinancialFact> {
        self.financials().update_financial_fact(input)
    }

    pub fn delete_financial_fact(&self, id: &str) -> StorageResult<()> {
        self.financials().delete_financial_fact(id)
    }

    pub fn create_or_find_pending_report_document(
        &self,
        input: CaptureReportDocumentInput,
    ) -> StorageResult<ReportDocument> {
        self.report_documents()
            .create_or_find_pending_report_document(input)
    }

    pub fn mark_report_document_fetched(
        &self,
        id: &str,
        local_path: Option<&str>,
        content_type: Option<&str>,
        content_hash: Option<&str>,
        byte_size: Option<i64>,
    ) -> StorageResult<ReportDocument> {
        self.report_documents().mark_report_document_fetched(
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
        self.report_documents()
            .mark_report_document_failed(id, error)
    }

    pub fn mark_report_document_metadata_only(&self, id: &str) -> StorageResult<ReportDocument> {
        self.report_documents()
            .mark_report_document_metadata_only(id)
    }

    pub fn list_pending_attachment_documents(&self) -> StorageResult<Vec<ReportDocument>> {
        self.report_documents().list_pending_attachment_documents()
    }

    pub fn get_report_document(&self, id: &str) -> StorageResult<ReportDocument> {
        self.report_documents().get_report_document(id)
    }

    pub fn list_report_documents_by_company(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<ReportDocument>> {
        self.report_documents()
            .list_report_documents_by_company(company_id)
    }

    pub fn list_report_documents_by_origin(
        &self,
        origin_ref: &str,
    ) -> StorageResult<Vec<ReportDocument>> {
        self.report_documents()
            .list_report_documents_by_origin(origin_ref)
    }

    /// Tracked companies with no fetched periodic report document, paired with
    /// their autopilot mode — the selection behind the automatic backfill catch-up
    /// (v0.57, ADR 0077 amendment). See
    /// [`ReportDocumentsStore::companies_lacking_periodic_coverage`].
    pub fn companies_lacking_periodic_coverage(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<(String, String)>> {
        self.report_documents()
            .companies_lacking_periodic_coverage(company_id)
    }

    pub fn reclassify_report_documents(&self) -> StorageResult<ReclassifyReportDocumentsSummary> {
        self.report_documents().reclassify_report_documents()
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

#[cfg(test)]
mod feed_id_proptests {
    //! Invariant coverage of the feed-item id derivation (ADR 0049): an id-
    //! producing transform must have **stable identity** (same input → same id,
    //! deterministically), be total over arbitrary dedup-key text, and always
    //! carry its namespace prefix.
    use super::{feed_item_attachment_id, feed_item_id};
    use crate::transform_invariants::assert_deterministic_str;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn feed_item_id_is_deterministic_stable_and_prefixed(key in ".*") {
            assert_deterministic_str(feed_item_id, &key);
            let id = feed_item_id(&key);
            prop_assert!(id.starts_with("feed_"), "missing namespace: {id:?}");
        }

        #[test]
        fn feed_item_attachment_id_is_deterministic_and_prefixed(id in ".*", url in ".*") {
            let a = feed_item_attachment_id(&id, &url);
            let b = feed_item_attachment_id(&id, &url);
            prop_assert_eq!(&a, &b, "attachment id is not deterministic");
            prop_assert!(a.starts_with("feed_attachment_"), "missing namespace: {a:?}");
        }
    }
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
