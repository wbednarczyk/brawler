pub mod ai_analysis;
pub mod autopilot;
pub mod backfill;
pub mod claim_extraction;
pub mod event_derivation;
pub mod feed_cleanup;
pub mod handlers;
pub mod health_facts_backfill;
pub mod history_sweep;
pub mod insider_attachment;
pub mod kpi_extraction;
pub mod management_holdings_extraction;
pub mod morning_briefing;
pub mod ownership_classification;
pub mod ownership_extraction;
pub mod ownership_ocr_extraction;
pub mod qualitative_assessment;
pub mod queue;
pub mod quote_backfill;
pub mod quote_daily_pull;
pub mod report_extraction;
pub mod research_briefs;
pub mod research_digests;
pub mod scheduler;
pub mod signal_classification;
pub mod source_refresh;
pub mod structured_extraction;
pub mod transcript_runner;

use crate::app_state::AppState;
use crate::providers::analysis::{
    capabilities::{AiCapability, CapabilityKind},
    pool, registry, AiAnalysisProvider, DocumentSupport,
};

/// Build an AI analysis provider **wrapped in its per-provider concurrency gate**
/// (ADR 0059). Every job that calls a provider builds it through here, so the shared
/// per-provider semaphore (limit = `ai_provider_concurrency` setting, one instance
/// per provider id in `AppState`) bounds total concurrent calls to that provider
/// across the autopilot + ai lanes. Replaces bare `registry::build_analysis_provider`
/// at the job call sites.
///
/// Used for the **explicit-override** path only: a job whose `provider_id` is a
/// concrete provider a user picked. Capability-routed jobs (`provider_id ==
/// CAPABILITY_ROUTED_PROVIDER_ID`) go through [`build_capability_provider`] instead.
pub fn build_gated_analysis_provider(
    state: &AppState,
    provider_id: &str,
    api_key: Option<String>,
    model: &str,
    timeout_seconds: i64,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    // Only the OpenAI-compatible provider (ADR 0060) consults this setting; the
    // registry ignores it for every other provider id.
    let openai_compatible_base_url = state
        .get_settings()
        .map_err(|error| error.to_string())?
        .ai_providers
        .openai_compatible_base_url;
    let openai_compatible_base_url =
        Some(openai_compatible_base_url.trim()).filter(|value| !value.is_empty());
    let provider = registry::build_analysis_provider(
        provider_id,
        api_key,
        model,
        timeout_seconds,
        openai_compatible_base_url,
    )?;
    let limit = state.queue_config().ai_provider_concurrency.max(1) as usize;
    let semaphore = state.provider_semaphore(provider_id, limit);
    Ok(registry::gate_analysis_provider(provider, semaphore))
}

/// One resolved (provider, model) pool member for a capability (ADR 0060 as
/// amended), before it is credential-checked, built, and gated.
pub struct ResolvedCapabilityMember {
    pub provider_id: String,
    pub model: String,
}

/// Resolve a capability's ordered provider pool from settings (ADR 0060 as
/// amended, ADR 0061 decision 5): the capability's configured
/// `capability_providers` entry when non-empty; otherwise a single member built
/// from `general_analysis_provider` / `general_analysis_model` when a general
/// provider is configured; otherwise an empty list.
///
/// Never errors on emptiness — an empty result is a legitimate "nothing routed"
/// outcome, and callers differ in how they want to handle it (autopilot degrades
/// the run, the ESPI fallbacks surface a specific error, the KPI/claim enqueue
/// commands fall back to the test-sample provider to preserve their pre-pool
/// default).
pub fn resolve_capability_members(
    state: &AppState,
    capability: AiCapability,
) -> Result<Vec<ResolvedCapabilityMember>, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;

    if let Some(entries) = settings.capability_providers.get(capability.key()) {
        if !entries.is_empty() {
            return Ok(entries
                .iter()
                .map(|entry| ResolvedCapabilityMember {
                    provider_id: entry.provider.clone(),
                    model: entry.model.clone(),
                })
                .collect());
        }
    }

    let general_provider_id = settings
        .ai_providers
        .general_analysis_provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match general_provider_id {
        Some(provider_id) => Ok(vec![ResolvedCapabilityMember {
            provider_id: provider_id.to_owned(),
            model: settings.ai_providers.general_analysis_model.clone(),
        }]),
        None => Ok(Vec::new()),
    }
}

/// Whether a member resolved for a document-kind capability (KPI/claim
/// extraction) must be rejected because its built provider cannot accept the
/// **native** document bytes every document-kind call site sends
/// ([`crate::providers::analysis::AnalysisDocument::Native`]) — a settings
/// mistake, not a transient outage, so it is a hard build error rather than a
/// skip (ADR 0061 decision 5, Radicle 6ea2a8a).
///
/// `TextOnly` is rejected here too, not just `None`: a document-kind
/// capability always sends `AnalysisDocument::Native`, and a `TextOnly`
/// provider (e.g. `provider_openai`, `provider_openai_compatible`) returns a
/// non-availability error for every such call — it would silently poison
/// pool failover (a cooling `Native` member promotes the `TextOnly` one to
/// the front, which then hard-fails every call for the cooldown window)
/// instead of ever completing an extraction.
fn document_capability_needs_native_support(
    capability: AiCapability,
    provider_document_support: DocumentSupport,
) -> bool {
    capability.kind() == CapabilityKind::Document
        && provider_document_support != DocumentSupport::Native
}

/// Build a capability's failover pool (ADR 0061 decision 5): resolve the
/// capability's ordered members via [`resolve_capability_members`], gate and
/// wrap each buildable member in the shared per-app cooldown state
/// ([`AppState::provider_cooldowns`]), and return the pool as a single
/// [`AiAnalysisProvider`].
///
/// A member is **skipped** (logged, not failed) when it has no configured
/// credential, or — for the OpenAI-compatible provider — no configured base
/// URL: neither is worth a network round trip. Every other build failure,
/// including a document-kind capability routed at a provider without native
/// document support, is a **hard error**: it signals a real settings mistake
/// to fix, not a member to quietly drop.
///
/// If every member ends up skipped, the resulting error enumerates each
/// member's skip reason (`provider/model: <reason>`) rather than a bare
/// "unbuildable", and points the user at Settings → AI (Radicle R2 #8).
pub fn build_capability_provider(
    state: &AppState,
    capability: AiCapability,
    timeout_seconds: i64,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    let members = resolve_capability_members(state, capability)?;

    let openai_compatible_base_url = state
        .get_settings()
        .map_err(|error| error.to_string())?
        .ai_providers
        .openai_compatible_base_url;
    let openai_compatible_base_url =
        Some(openai_compatible_base_url.trim().to_owned()).filter(|value| !value.is_empty());

    let limit = state.queue_config().ai_provider_concurrency.max(1) as usize;
    let mut pool_members = Vec::new();
    let mut skip_reasons: Vec<String> = Vec::new();

    for member in members {
        let requires_credential =
            registry::analysis_provider_requires_credential(&member.provider_id);
        let api_key = registry::read_analysis_provider_api_key(&member.provider_id);
        if requires_credential && api_key.is_none() {
            log::warn!(
                "module=capability_pool stage=skip_member capability={} providerId={} model={} reason=missing_credential",
                capability.key(),
                member.provider_id,
                member.model
            );
            skip_reasons.push(format!(
                "{}/{}: missing credential",
                member.provider_id, member.model
            ));
            continue;
        }
        if member.provider_id == registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
            && openai_compatible_base_url.is_none()
        {
            log::warn!(
                "module=capability_pool stage=skip_member capability={} providerId={} model={} reason=missing_base_url",
                capability.key(),
                member.provider_id,
                member.model
            );
            skip_reasons.push(format!(
                "{}/{}: missing base URL",
                member.provider_id, member.model
            ));
            continue;
        }

        let provider = registry::build_analysis_provider(
            &member.provider_id,
            api_key,
            &member.model,
            timeout_seconds,
            openai_compatible_base_url.as_deref(),
        )?;

        if document_capability_needs_native_support(capability, provider.document_support()) {
            return Err(format!(
                "capability {} is routed to provider {} which does not support document input",
                capability.key(),
                member.provider_id
            ));
        }

        let semaphore = state.provider_semaphore(&member.provider_id, limit);
        let gated = registry::gate_analysis_provider(provider, semaphore);
        pool_members.push(pool::PoolMember {
            cooldown_key: format!("{}::{}", member.provider_id, member.model),
            provider: gated,
        });
    }

    if pool_members.is_empty() {
        let reasons = if skip_reasons.is_empty() {
            "no members are configured".to_owned()
        } else {
            skip_reasons.join("; ")
        };
        return Err(format!(
            "no usable AI provider for capability {}: every configured member is unconfigured or unbuildable ({reasons}). Fix this in Settings \u{2192} AI.",
            capability.key()
        ));
    }

    Ok(Box::new(pool::PooledAnalysisProvider::new(
        pool_members,
        state.provider_cooldowns(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::analysis::{
        TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    };
    use crate::storage::{
        open_in_memory_database, AppState, CapabilityProviderEntry, SettingsUpdate,
    };
    use std::collections::HashMap;

    #[test]
    fn resolver_prefers_capability_map_over_general() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                general_analysis_provider: Some(registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned()),
                general_analysis_model: Some("gemini-3.5-flash".to_owned()),
                capability_providers: Some(HashMap::from([(
                    AiCapability::FeedAnalysis.key().to_owned(),
                    vec![CapabilityProviderEntry {
                        provider: registry::ANTHROPIC_ANALYSIS_PROVIDER_ID.to_owned(),
                        model: "claude-sonnet-4-6".to_owned(),
                    }],
                )])),
                ..Default::default()
            })
            .expect("settings should update");

        let members = resolve_capability_members(&state, AiCapability::FeedAnalysis)
            .expect("resolution should succeed");

        assert_eq!(members.len(), 1);
        assert_eq!(
            members[0].provider_id,
            registry::ANTHROPIC_ANALYSIS_PROVIDER_ID
        );
        assert_eq!(members[0].model, "claude-sonnet-4-6");
    }

    #[test]
    fn resolver_falls_back_to_general_provider() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                general_analysis_provider: Some(registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned()),
                general_analysis_model: Some("gemini-3.5-flash".to_owned()),
                ..Default::default()
            })
            .expect("settings should update");

        let members = resolve_capability_members(&state, AiCapability::FeedAnalysis)
            .expect("resolution should succeed");

        assert_eq!(members.len(), 1);
        assert_eq!(
            members[0].provider_id,
            registry::GEMINI_ANALYSIS_PROVIDER_ID
        );
        assert_eq!(members[0].model, "gemini-3.5-flash");
    }

    #[test]
    fn qualitative_assessment_resolves_through_the_text_pool() {
        // ADR 0075: the new capability routes like any text capability — its
        // `capability_providers` override wins, else the general fallback.
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                general_analysis_provider: Some(registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned()),
                general_analysis_model: Some("gemini-3.5-flash".to_owned()),
                capability_providers: Some(HashMap::from([(
                    AiCapability::QualitativeAssessment.key().to_owned(),
                    vec![CapabilityProviderEntry {
                        provider: registry::ANTHROPIC_ANALYSIS_PROVIDER_ID.to_owned(),
                        model: "claude-sonnet-4-6".to_owned(),
                    }],
                )])),
                ..Default::default()
            })
            .expect("settings should update");

        let members = resolve_capability_members(&state, AiCapability::QualitativeAssessment)
            .expect("resolution should succeed");
        assert_eq!(members.len(), 1);
        assert_eq!(
            members[0].provider_id,
            registry::ANTHROPIC_ANALYSIS_PROVIDER_ID
        );
    }

    #[test]
    fn resolver_returns_empty_when_nothing_configured() {
        let connection = open_in_memory_database().expect("database should initialize");
        // The default seed pins `general_analysis_provider` to `provider_gemini`
        // (migration 0036) — clear it to simulate a genuinely unconfigured app.
        connection
            .execute(
                "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
                [],
            )
            .expect("clear the default general provider");
        let state = AppState::new(connection);

        let members = resolve_capability_members(&state, AiCapability::FeedAnalysis)
            .expect("resolution should succeed even when nothing is configured");

        assert!(members.is_empty());
    }

    #[test]
    fn capability_pool_rejects_document_capability_without_native_support() {
        // A document-kind capability always sends `AnalysisDocument::Native`
        // (jobs/kpi_extraction.rs), so only a `Native`-support provider may
        // serve it — `TextOnly` must be rejected exactly like `None`, not
        // silently accepted (Radicle 6ea2a8a).
        assert!(document_capability_needs_native_support(
            AiCapability::KpiExtraction,
            DocumentSupport::None
        ));
        assert!(document_capability_needs_native_support(
            AiCapability::KpiExtraction,
            DocumentSupport::TextOnly
        ));
        assert!(document_capability_needs_native_support(
            AiCapability::ClaimExtraction,
            DocumentSupport::None
        ));
        assert!(document_capability_needs_native_support(
            AiCapability::ClaimExtraction,
            DocumentSupport::TextOnly
        ));
        assert!(!document_capability_needs_native_support(
            AiCapability::KpiExtraction,
            DocumentSupport::Native
        ));
        // Text-kind capabilities never need document support at all.
        assert!(!document_capability_needs_native_support(
            AiCapability::FeedAnalysis,
            DocumentSupport::None
        ));
        assert!(!document_capability_needs_native_support(
            AiCapability::FeedAnalysis,
            DocumentSupport::TextOnly
        ));
    }

    #[test]
    fn capability_pool_rejects_vision_extraction_without_native_support() {
        // ADR 0077 T4.2: VisionExtraction is document-kind, so the build-layer
        // guard `build_capability_provider` applies rejects a non-native
        // provider for it exactly like the other document capabilities — and
        // accepts a Native provider (Mistral OCR).
        assert!(document_capability_needs_native_support(
            AiCapability::VisionExtraction,
            DocumentSupport::TextOnly
        ));
        assert!(document_capability_needs_native_support(
            AiCapability::VisionExtraction,
            DocumentSupport::None
        ));
        assert!(!document_capability_needs_native_support(
            AiCapability::VisionExtraction,
            DocumentSupport::Native
        ));
    }

    /// End-to-end pin (Radicle 6ea2a8a): a real `TextOnly` provider
    /// (OpenAI-compatible) routed at a document-kind capability must hard-fail
    /// `build_capability_provider` naming both the capability and the
    /// provider — not silently build a pool that then hard-fails every call.
    /// Seeds `capability_providers` directly, bypassing
    /// `validate_capability_providers` (which now rejects this combination at
    /// the settings layer), to exercise the build-layer guard in isolation —
    /// the same technique `skips_member_without_credential` uses.
    ///
    /// Sets the provider's dev-fallback env var so the member survives the
    /// earlier missing-credential skip and actually reaches the document
    /// check; safe here because `cargo nextest` (this repo's test runner)
    /// runs every test in its own process, so the mutation cannot leak into
    /// other tests.
    #[test]
    fn build_capability_provider_hard_fails_text_only_provider_for_document_capability() {
        crate::providers::credentials::scrub_provider_env_fallbacks();
        std::env::set_var("OPENAI_COMPATIBLE_API_KEY", "test-key");

        let connection = open_in_memory_database().expect("database should initialize");
        let capability_providers_json = serde_json::json!({
            AiCapability::KpiExtraction.key(): [
                {
                    "provider": registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
                    "model": "custom-model",
                },
            ]
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) VALUES ('capability_providers', ?1, 'json') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [capability_providers_json],
            )
            .expect("seed capability_providers bypassing validation");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                openai_compatible_base_url: Some("https://example.test/v1".to_owned()),
                ..Default::default()
            })
            .expect("base url should update");

        let result = build_capability_provider(&state, AiCapability::KpiExtraction, 90);
        std::env::remove_var("OPENAI_COMPATIBLE_API_KEY");
        let error = match result {
            Ok(_) => panic!("a text-only provider must hard-fail a document capability"),
            Err(error) => error,
        };

        assert!(
            error.contains(AiCapability::KpiExtraction.key()),
            "error must name the capability: {error}"
        );
        assert!(
            error.contains(registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID),
            "error must name the provider: {error}"
        );
    }

    /// R2 #8: when every member is skipped, the zero-members error must
    /// enumerate *each* member's skip reason (not just a generic "unbuildable"),
    /// so a user can fix settings without digging through logs.
    #[test]
    fn build_capability_provider_error_lists_all_skip_reasons() {
        // Give the OpenAI-compatible member a credential (via its dev-fallback
        // env var — safe under `cargo nextest`'s per-test process isolation)
        // so it survives the credential check and is skipped for the *other*
        // reason instead (missing base URL), while Gemini is skipped for
        // missing credential — exercising both skip-reason messages at once.
        // Scrub first: an exported GEMINI_API_KEY in the developer's shell
        // would otherwise give Gemini a dev-fallback credential (guardrail
        // 2026-07-11).
        crate::providers::credentials::scrub_provider_env_fallbacks();
        std::env::set_var("OPENAI_COMPATIBLE_API_KEY", "test-key");

        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .update_settings(SettingsUpdate {
                capability_providers: Some(HashMap::from([(
                    AiCapability::FeedAnalysis.key().to_owned(),
                    vec![
                        CapabilityProviderEntry {
                            provider: registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned(),
                            model: "gemini-3.5-flash".to_owned(),
                        },
                        CapabilityProviderEntry {
                            provider: registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID.to_owned(),
                            model: "custom-model".to_owned(),
                        },
                    ],
                )])),
                ..Default::default()
            })
            .expect("settings should update");

        let result = build_capability_provider(&state, AiCapability::FeedAnalysis, 90);
        std::env::remove_var("OPENAI_COMPATIBLE_API_KEY");
        let error = match result {
            Ok(_) => panic!(
                "no member should survive: gemini has no credential, openai-compatible has no base URL"
            ),
            Err(error) => error,
        };

        assert!(
            error.contains("missing credential"),
            "error must mention the missing-credential reason: {error}"
        );
        assert!(
            error.contains("missing base URL"),
            "error must mention the missing-base-url reason: {error}"
        );
        assert!(error.contains(registry::GEMINI_ANALYSIS_PROVIDER_ID));
        assert!(error.contains(registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID));
        assert!(
            error.contains("Settings"),
            "error should point the user at Settings \u{2192} AI: {error}"
        );
    }

    #[test]
    fn skips_member_without_credential() {
        crate::providers::credentials::scrub_provider_env_fallbacks();
        // `validate_capability_providers` only ever allows currently-selectable
        // (non-test-sample) providers into the map, so a mixed real +
        // test-sample pool can never be configured through the public
        // `update_settings` path. Seed the settings row directly to exercise the
        // pool-building concern in isolation — the same bypass technique the
        // autopilot tests use to reach states validation would otherwise block.
        let connection = open_in_memory_database().expect("database should initialize");
        let capability_providers_json = serde_json::json!({
            AiCapability::FeedAnalysis.key(): [
                { "provider": registry::GEMINI_ANALYSIS_PROVIDER_ID, "model": "gemini-3.5-flash" },
                { "provider": TEST_SAMPLE_ANALYSIS_PROVIDER_ID, "model": TEST_SAMPLE_ANALYSIS_MODEL },
            ]
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) VALUES ('capability_providers', ?1, 'json') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [capability_providers_json],
            )
            .expect("seed capability_providers bypassing validation");
        let state = AppState::new(connection);

        // Gemini requires a credential; the test environment has none configured
        // (no OS keychain entry, no dev-env fallback), so it must be skipped and
        // the pool must fall through to the credential-less test-sample member.
        let provider = build_capability_provider(&state, AiCapability::FeedAnalysis, 90)
            .expect("the pool should build from the surviving credential-less member");

        assert_eq!(provider.provider_id(), TEST_SAMPLE_ANALYSIS_PROVIDER_ID);
    }

    #[test]
    fn pool_members_get_distinct_semaphores() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let gemini = state.provider_semaphore(registry::GEMINI_ANALYSIS_PROVIDER_ID, 2);
        let anthropic = state.provider_semaphore(registry::ANTHROPIC_ANALYSIS_PROVIDER_ID, 2);
        let gemini_again = state.provider_semaphore(registry::GEMINI_ANALYSIS_PROVIDER_ID, 2);

        assert!(
            !std::sync::Arc::ptr_eq(&gemini, &anthropic),
            "distinct provider ids must get distinct semaphores"
        );
        assert!(
            std::sync::Arc::ptr_eq(&gemini, &gemini_again),
            "the same provider id must reuse its semaphore"
        );
    }
}
