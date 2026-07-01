pub mod ai_analysis;
pub mod autopilot;
pub mod backfill;
pub mod claim_extraction;
pub mod content_embedding;
pub mod event_derivation;
pub mod feed_cleanup;
pub mod handlers;
pub mod kpi_extraction;
pub mod queue;
pub mod report_extraction;
pub mod research_briefs;
pub mod research_digests;
pub mod scheduler;
pub mod signal_classification;
pub mod source_refresh;
pub mod structured_extraction;
pub mod transcript_runner;

use crate::app_state::AppState;
use crate::providers::analysis::{registry, AiAnalysisProvider};

/// Build an AI analysis provider **wrapped in its per-provider concurrency gate**
/// (ADR 0059). Every job that calls a provider builds it through here, so the shared
/// per-provider semaphore (limit = `ai_provider_concurrency` setting, one instance
/// per provider id in `AppState`) bounds total concurrent calls to that provider
/// across the autopilot + ai lanes. Replaces bare `registry::build_analysis_provider`
/// at the job call sites.
pub fn build_gated_analysis_provider(
    state: &AppState,
    provider_id: &str,
    api_key: Option<String>,
    model: &str,
    timeout_seconds: i64,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    let provider = registry::build_analysis_provider(provider_id, api_key, model, timeout_seconds)?;
    let limit = state.queue_config().ai_provider_concurrency.max(1) as usize;
    let semaphore = state.provider_semaphore(provider_id, limit);
    Ok(registry::gate_analysis_provider(provider, semaphore))
}
