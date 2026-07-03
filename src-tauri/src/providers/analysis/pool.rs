//! Ordered failover pool over gated analysis providers (ADR 0061 decision 5).
//!
//! A capability can be routed to more than one (provider, model) pair, tried in
//! configured order: the first member handles the call, and only a failure
//! plausibly caused by *that provider's own availability* — rate limiting,
//! outage, or a network error, per [`AnalysisProviderError::is_availability_error`]
//! — advances to the next member. Any other error (bad request, missing
//! credential, a 200 with unparsable content) surfaces immediately; it is not
//! evidence the *next* provider would do better, so failing over on it would
//! just hide the real problem behind an unrelated provider's response.
//!
//! A member that just failed with an availability error is not removed from the
//! pool — it is deprioritized behind the pool's other members for
//! [`PROVIDER_FAILOVER_COOLDOWN`], via the shared, cross-call [`ProviderCooldowns`]
//! state (one instance lives on [`crate::app_state::AppState`] and is reused by
//! every pool built for the app's lifetime). Once every member is inside its
//! cooldown window the pool still tries all of them, in their original order —
//! a pool never refuses to try just because everything recently failed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::types::{
    AiAnalysisProvider, AnalysisDocument, AnalysisProviderError, AnalysisProviderOutput,
    AnalysisRequest, DocumentSupport, ResearchBriefProviderOutput, ResearchBriefRequest,
    ResearchDigestRequest,
};

/// How long a pool member that just failed with an availability error is
/// deprioritized behind the pool's other members before being tried first again.
pub const PROVIDER_FAILOVER_COOLDOWN: Duration = Duration::from_secs(60);

/// Shared, cross-call record of which pool members (keyed by
/// `"{provider_id}::{model}"`) recently failed with an availability error.
/// Cheap to clone — an `Arc` around the shared map — so every capability pool
/// built during the app's lifetime can share one instance
/// ([`crate::app_state::AppState::provider_cooldowns`]).
#[derive(Clone, Default)]
pub struct ProviderCooldowns(Arc<Mutex<HashMap<String, Instant>>>);

impl ProviderCooldowns {
    /// Record that the member identified by `key` failed with an availability
    /// error at `now`.
    pub fn mark(&self, key: &str, now: Instant) {
        let mut cooldowns = self.0.lock().expect("provider cooldown map poisoned");
        cooldowns.insert(key.to_owned(), now);
    }

    /// Whether `key` is still inside its cooldown window as of `now`.
    pub fn is_cooling(&self, key: &str, now: Instant) -> bool {
        let cooldowns = self.0.lock().expect("provider cooldown map poisoned");
        match cooldowns.get(key) {
            Some(marked_at) => {
                now.saturating_duration_since(*marked_at) < PROVIDER_FAILOVER_COOLDOWN
            }
            None => false,
        }
    }
}

/// One member of a failover pool: a (typically gated, see
/// [`super::registry::gate_analysis_provider`]) provider plus the cooldown key
/// it is tracked under.
pub struct PoolMember {
    /// `"{provider_id}::{model}"` — the key this member is tracked under in
    /// [`ProviderCooldowns`]. Distinct providers, or the same provider at a
    /// different model, cool down independently.
    pub cooldown_key: String,
    pub provider: Box<dyn AiAnalysisProvider>,
}

/// An ordered failover pool of one or more [`PoolMember`]s, itself an
/// [`AiAnalysisProvider`] (ADR 0061 decision 5).
pub struct PooledAnalysisProvider {
    members: Vec<PoolMember>,
    cooldowns: ProviderCooldowns,
}

impl PooledAnalysisProvider {
    /// # Panics
    /// Panics if `members` is empty — a pool with no members is a caller bug
    /// (the caller should have surfaced a "no usable provider" error instead of
    /// constructing an empty pool).
    pub fn new(members: Vec<PoolMember>, cooldowns: ProviderCooldowns) -> Self {
        assert!(
            !members.is_empty(),
            "PooledAnalysisProvider requires at least one member"
        );
        Self { members, cooldowns }
    }

    /// Indices of `self.members` in call order for this attempt: every member is
    /// always included, cooling ones just sort after non-cooling ones.
    fn ordered_member_indices(&self) -> Vec<usize> {
        let now = Instant::now();
        let cooling: Vec<bool> = self
            .members
            .iter()
            .map(|member| self.cooldowns.is_cooling(&member.cooldown_key, now))
            .collect();
        ordered_indices(&cooling)
    }

    /// Record that `member` just failed with an availability error, so later
    /// calls (on this pool and any other pool sharing the same
    /// [`ProviderCooldowns`]) deprioritize it for [`PROVIDER_FAILOVER_COOLDOWN`].
    fn note_failover(&self, member: &PoolMember, error: &AnalysisProviderError) {
        self.cooldowns.mark(&member.cooldown_key, Instant::now());
        log::warn!(
            "module=analysis_pool stage=failover providerId={} model={} cooldownKey={} errorCode={} error={}",
            member.provider.provider_id(),
            member.provider.model(),
            member.cooldown_key,
            error.code(),
            error
        );
    }
}

/// Pure ordering helper: indices of `cooling` in call order — every
/// non-cooling index first (original order preserved), then every cooling
/// index (original order preserved). Every index appears exactly once, so the
/// whole pool is always tried even when every member is cooling.
fn ordered_indices(cooling: &[bool]) -> Vec<usize> {
    let mut warm = Vec::new();
    let mut cool = Vec::new();
    for (index, is_cooling) in cooling.iter().enumerate() {
        if *is_cooling {
            cool.push(index);
        } else {
            warm.push(index);
        }
    }
    warm.into_iter().chain(cool).collect()
}

#[async_trait]
impl AiAnalysisProvider for PooledAnalysisProvider {
    fn provider_id(&self) -> &'static str {
        self.members[0].provider.provider_id()
    }

    fn model(&self) -> &str {
        self.members[0].provider.model()
    }

    fn document_support(&self) -> DocumentSupport {
        self.members[0].provider.document_support()
    }

    async fn analyze(
        &self,
        request: &AnalysisRequest,
    ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
        let mut last_error = None;
        for index in self.ordered_member_indices() {
            let member = &self.members[index];
            match member.provider.analyze(request).await {
                Ok(value) => return Ok(value),
                Err(error) if error.is_availability_error() => {
                    self.note_failover(member, &error);
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.expect("PooledAnalysisProvider has at least one member"))
    }

    async fn generate_research_brief(
        &self,
        request: &ResearchBriefRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let mut last_error = None;
        for index in self.ordered_member_indices() {
            let member = &self.members[index];
            match member.provider.generate_research_brief(request).await {
                Ok(value) => return Ok(value),
                Err(error) if error.is_availability_error() => {
                    self.note_failover(member, &error);
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.expect("PooledAnalysisProvider has at least one member"))
    }

    async fn generate_research_digest(
        &self,
        request: &ResearchDigestRequest,
    ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
        let mut last_error = None;
        for index in self.ordered_member_indices() {
            let member = &self.members[index];
            match member.provider.generate_research_digest(request).await {
                Ok(value) => return Ok(value),
                Err(error) if error.is_availability_error() => {
                    self.note_failover(member, &error);
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.expect("PooledAnalysisProvider has at least one member"))
    }

    async fn complete_document(
        &self,
        prompt: &str,
        document: &AnalysisDocument,
    ) -> Result<String, AnalysisProviderError> {
        let mut last_error = None;
        for index in self.ordered_member_indices() {
            let member = &self.members[index];
            match member.provider.complete_document(prompt, document).await {
                Ok(value) => return Ok(value),
                Err(error) if error.is_availability_error() => {
                    self.note_failover(member, &error);
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.expect("PooledAnalysisProvider has at least one member"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Copy)]
    enum ScriptedOutcome {
        Succeed,
        Fail(ErrorKind),
    }

    #[derive(Clone, Copy)]
    enum ErrorKind {
        Limit,
        Unavailable,
        Network,
        BadRequest,
        Parse,
    }

    impl ErrorKind {
        fn build(self) -> AnalysisProviderError {
            match self {
                Self::Limit => AnalysisProviderError::ProviderLimit,
                Self::Unavailable => AnalysisProviderError::ProviderUnavailable("down".to_owned()),
                Self::Network => AnalysisProviderError::NetworkError("connection reset".to_owned()),
                Self::BadRequest => AnalysisProviderError::ProviderError("bad request".to_owned()),
                Self::Parse => AnalysisProviderError::ParseError("bad json".to_owned()),
            }
        }
    }

    struct ScriptedProvider {
        id: &'static str,
        model_id: String,
        calls: Arc<AtomicUsize>,
        outcome: ScriptedOutcome,
    }

    #[async_trait]
    impl AiAnalysisProvider for ScriptedProvider {
        fn provider_id(&self) -> &'static str {
            self.id
        }

        fn model(&self) -> &str {
            &self.model_id
        }

        async fn analyze(
            &self,
            _request: &AnalysisRequest,
        ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.outcome {
                ScriptedOutcome::Succeed => Ok(AnalysisProviderOutput {
                    summary: self.id.to_owned(),
                    significance: "unknown".to_owned(),
                    reasoning: String::new(),
                    language: None,
                    tags: Vec::new(),
                    source_references: Vec::new(),
                }),
                ScriptedOutcome::Fail(kind) => Err(kind.build()),
            }
        }

        async fn generate_research_brief(
            &self,
            _request: &ResearchBriefRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.outcome {
                ScriptedOutcome::Succeed => Ok(ResearchBriefProviderOutput {
                    title: self.id.to_owned(),
                    summary: String::new(),
                    sections: Vec::new(),
                    language: None,
                    citations: Vec::new(),
                }),
                ScriptedOutcome::Fail(kind) => Err(kind.build()),
            }
        }

        async fn generate_research_digest(
            &self,
            _request: &ResearchDigestRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.outcome {
                ScriptedOutcome::Succeed => Ok(ResearchBriefProviderOutput {
                    title: self.id.to_owned(),
                    summary: String::new(),
                    sections: Vec::new(),
                    language: None,
                    citations: Vec::new(),
                }),
                ScriptedOutcome::Fail(kind) => Err(kind.build()),
            }
        }

        async fn complete_document(
            &self,
            _prompt: &str,
            _document: &AnalysisDocument,
        ) -> Result<String, AnalysisProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.outcome {
                ScriptedOutcome::Succeed => Ok(self.id.to_owned()),
                ScriptedOutcome::Fail(kind) => Err(kind.build()),
            }
        }
    }

    fn cooldown_key(id: &str) -> String {
        format!("{id}::{id}-model")
    }

    fn make_member(id: &'static str, outcome: ScriptedOutcome) -> (PoolMember, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = ScriptedProvider {
            id,
            model_id: format!("{id}-model"),
            calls: calls.clone(),
            outcome,
        };
        (
            PoolMember {
                cooldown_key: cooldown_key(id),
                provider: Box::new(provider),
            },
            calls,
        )
    }

    fn sample_analysis_request() -> AnalysisRequest {
        AnalysisRequest {
            feed_item: crate::storage::FeedItem {
                id: "feed_1".to_owned(),
                company: "GPW:CDR".to_owned(),
                item_type: "official_report".to_owned(),
                source: "Bankier Company Komunikaty".to_owned(),
                time: "2026-06-03T10:00:00Z".to_owned(),
                title: "Quarterly report".to_owned(),
                unread: true,
                saved: false,
                source_url: "https://example.com/report".to_owned(),
                language: "en".to_owned(),
                published_at: "2026-06-03T10:00:00Z".to_owned(),
                fetched_at: "2026-06-03T10:05:00Z".to_owned(),
                attribution: "Bankier".to_owned(),
                summary: "Summary".to_owned(),
                body_text: "Body".to_owned(),
                attachments: Vec::new(),
            },
            prompt_preset_id: "default_summary".to_owned(),
            custom_question: None,
        }
    }

    #[test]
    fn first_success_short_circuits() {
        let (member_a, calls_a) = make_member("a", ScriptedOutcome::Succeed);
        let (member_b, calls_b) = make_member("b", ScriptedOutcome::Fail(ErrorKind::Unavailable));
        let pool =
            PooledAnalysisProvider::new(vec![member_a, member_b], ProviderCooldowns::default());

        let output = tauri::async_runtime::block_on(pool.analyze(&sample_analysis_request()))
            .expect("first member succeeds");

        assert_eq!(output.summary, "a");
        assert_eq!(calls_a.load(Ordering::SeqCst), 1);
        assert_eq!(
            calls_b.load(Ordering::SeqCst),
            0,
            "the second member must never be touched once the first succeeds"
        );
    }

    #[test]
    fn fails_over_on_unavailable_and_limit_and_network() {
        for kind in [ErrorKind::Limit, ErrorKind::Unavailable, ErrorKind::Network] {
            let (member_a, calls_a) = make_member("a", ScriptedOutcome::Fail(kind));
            let (member_b, calls_b) = make_member("b", ScriptedOutcome::Succeed);
            let pool =
                PooledAnalysisProvider::new(vec![member_a, member_b], ProviderCooldowns::default());

            let output = tauri::async_runtime::block_on(pool.analyze(&sample_analysis_request()))
                .expect("second member should succeed after failover");

            assert_eq!(output.summary, "b");
            assert_eq!(calls_a.load(Ordering::SeqCst), 1);
            assert_eq!(calls_b.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn does_not_fail_over_on_provider_error_or_parse_error() {
        for kind in [ErrorKind::BadRequest, ErrorKind::Parse] {
            let (member_a, calls_a) = make_member("a", ScriptedOutcome::Fail(kind));
            let (member_b, calls_b) = make_member("b", ScriptedOutcome::Succeed);
            let pool =
                PooledAnalysisProvider::new(vec![member_a, member_b], ProviderCooldowns::default());

            let error = tauri::async_runtime::block_on(pool.analyze(&sample_analysis_request()))
                .expect_err("a non-availability error must surface immediately");

            assert_eq!(error.code(), kind.build().code());
            assert_eq!(calls_a.load(Ordering::SeqCst), 1);
            assert_eq!(
                calls_b.load(Ordering::SeqCst),
                0,
                "a non-availability error must not trigger failover"
            );
        }
    }

    #[test]
    fn cooldown_skips_failed_member_first_but_still_tries_it() {
        let cooldowns = ProviderCooldowns::default();
        cooldowns.mark(&cooldown_key("a"), Instant::now());
        let (member_a, calls_a) = make_member("a", ScriptedOutcome::Succeed);
        let (member_b, calls_b) = make_member("b", ScriptedOutcome::Fail(ErrorKind::Unavailable));
        let pool = PooledAnalysisProvider::new(vec![member_a, member_b], cooldowns);

        let output = tauri::async_runtime::block_on(pool.analyze(&sample_analysis_request()))
            .expect("the cooling member is still tried once the warm member fails");

        assert_eq!(output.summary, "a");
        assert_eq!(
            calls_b.load(Ordering::SeqCst),
            1,
            "the warm member is tried first"
        );
        assert_eq!(
            calls_a.load(Ordering::SeqCst),
            1,
            "the cooling member is deprioritized, not excluded"
        );
    }

    #[test]
    fn cooldown_expires_after_window() {
        let cooldowns = ProviderCooldowns::default();
        let base = Instant::now();
        cooldowns.mark("provider_x::model", base);

        assert!(cooldowns.is_cooling("provider_x::model", base + Duration::from_secs(1)));
        assert!(cooldowns.is_cooling(
            "provider_x::model",
            base + PROVIDER_FAILOVER_COOLDOWN - Duration::from_millis(1)
        ));
        assert!(!cooldowns.is_cooling("provider_x::model", base + PROVIDER_FAILOVER_COOLDOWN));
        assert!(!cooldowns.is_cooling(
            "provider_x::model",
            base + PROVIDER_FAILOVER_COOLDOWN + Duration::from_secs(30)
        ));
    }

    #[test]
    fn all_cooling_still_tries_in_order() {
        let cooldowns = ProviderCooldowns::default();
        let now = Instant::now();
        cooldowns.mark(&cooldown_key("a"), now);
        cooldowns.mark(&cooldown_key("b"), now);
        let (member_a, calls_a) = make_member("a", ScriptedOutcome::Fail(ErrorKind::Unavailable));
        let (member_b, calls_b) = make_member("b", ScriptedOutcome::Succeed);
        let pool = PooledAnalysisProvider::new(vec![member_a, member_b], cooldowns);

        let output = tauri::async_runtime::block_on(pool.analyze(&sample_analysis_request()))
            .expect("an all-cooling pool still tries every member, in original order");

        assert_eq!(output.summary, "b");
        assert_eq!(calls_a.load(Ordering::SeqCst), 1);
        assert_eq!(calls_b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn exhaustion_returns_last_error() {
        let (member_a, _) = make_member("a", ScriptedOutcome::Fail(ErrorKind::Unavailable));
        let (member_b, _) = make_member("b", ScriptedOutcome::Fail(ErrorKind::Network));
        let pool =
            PooledAnalysisProvider::new(vec![member_a, member_b], ProviderCooldowns::default());

        let error = tauri::async_runtime::block_on(pool.analyze(&sample_analysis_request()))
            .expect_err("both members fail");

        assert_eq!(
            error.code(),
            "network_error",
            "exhaustion must surface the LAST member's error"
        );
    }

    #[test]
    fn complete_document_fails_over_like_analyze() {
        let (member_a, calls_a) = make_member("a", ScriptedOutcome::Fail(ErrorKind::Unavailable));
        let (member_b, calls_b) = make_member("b", ScriptedOutcome::Succeed);
        let pool =
            PooledAnalysisProvider::new(vec![member_a, member_b], ProviderCooldowns::default());
        let document = AnalysisDocument::Text {
            text: "body".to_owned(),
        };

        let text = tauri::async_runtime::block_on(pool.complete_document("prompt", &document))
            .expect(
                "complete_document should fail over too — the KPI/claim/ESPI job sites call it",
            );

        assert_eq!(text, "b");
        assert_eq!(calls_a.load(Ordering::SeqCst), 1);
        assert_eq!(calls_b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn identity_methods_delegate_to_first_member() {
        let (member_a, _) = make_member("a", ScriptedOutcome::Succeed);
        let (member_b, _) = make_member("b", ScriptedOutcome::Succeed);
        let pool =
            PooledAnalysisProvider::new(vec![member_a, member_b], ProviderCooldowns::default());

        assert_eq!(pool.provider_id(), "a");
        assert_eq!(pool.model(), "a-model");
    }

    #[test]
    fn ordered_indices_puts_cooling_last_preserving_order() {
        assert_eq!(ordered_indices(&[false, false, false]), vec![0, 1, 2]);
        assert_eq!(ordered_indices(&[true, false, true]), vec![1, 0, 2]);
        assert_eq!(ordered_indices(&[true, true]), vec![0, 1]);
        assert_eq!(ordered_indices(&[]), Vec::<usize>::new());
    }
}
