//! AI holder-type classify-with-confirm (v0.56 T5, ADR 0072 §3).
//!
//! The deterministic dictionary + heuristic markers
//! ([`crate::fundamentals::ownership::classify`]) stamp most holders at ingest.
//! This job covers the **residual** — holders left `holder_type = NULL` — by
//! asking a routable AI capability ([`AiCapability::OwnershipHolderClassification`])
//! to propose a type. It **never auto-applies**: every result is persisted as a
//! `pending` proposal that a user confirms (commands land in T6). A malformed or
//! low-signal model response yields no proposal and never a stamp.
//!
//! Provider resolution mirrors `signal_classification`: the capability routes to
//! the general-analysis pool unless the user configured an explicit pool. With no
//! provider configured and residual work present, the run is a hard error (a real
//! settings gap to surface), not a silent no-op.
//!
//! UNREGISTERED by design (T5): the queue handler + lane wiring and the trigger
//! command land with T6. The core is directly callable for tests and for T6.

use crate::{
    app_state::AppState,
    providers::analysis::{capabilities::AiCapability, AiAnalysisProvider, AnalysisDocument},
    storage::NewHolderTypeProposal,
};

/// The nine holder types (ADR 0072 §3); the parser rejects anything outside this
/// set so the CHECK constraint can never be violated by a hallucinated label.
pub const ALLOWED_HOLDER_TYPES: [&str; 9] = [
    "founder_insider",
    "family_foundation",
    "tfi",
    "ofe_pension",
    "state_treasury",
    "parent_company",
    "treasury_shares",
    "other_institutional",
    "free_float_rest",
];

/// Outcome of one classification run. No `ts_rs` export yet: the trigger command
/// (and its DTO) lands in T6 — mirrors the T2 headless-storage convention, so no
/// orphaned `src/api/generated/` export exists in the meantime.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiOwnershipClassificationSummary {
    /// Residual holders sent to the model this run.
    pub examined: usize,
    /// New/refreshed `pending` proposals created this run.
    pub proposed: usize,
    /// Holders the model returned `unknown` (or an out-of-taxonomy) for — no
    /// proposal, no error.
    pub skipped: usize,
    /// Provider/parse failures — no proposal, surfaced (logged + counted), never a
    /// stamp.
    pub errors: usize,
}

/// Marker the prompt carries so a deterministic test provider can branch on it.
const OWNERSHIP_CLASSIFICATION_MARKER: &str =
    "classify this shareholder into exactly one holder type";

/// Build the holder-type classification prompt for one holder name.
pub fn ownership_classification_prompt(holder_name: &str) -> String {
    let type_lines = ALLOWED_HOLDER_TYPES
        .iter()
        .map(|key| format!("- {key}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You classify shareholders of Polish stock-exchange companies. \
{OWNERSHIP_CLASSIFICATION_MARKER} below, or report \"unknown\" when none clearly applies. \
Do not guess: if the holder does not clearly match a type, return \"unknown\". The holder \
name is provided as the attached document.\n\n\
Holder types (key):\n{type_lines}\n\n\
Holder name:\n{holder_name}\n\n\
Respond with strict JSON only, no prose, in this exact shape:\n\
{{\"holderType\": \"<one type key, or 'unknown'>\", \"confidence\": <number between 0 and 1>, \
\"rationale\": \"<one short sentence>\"}}"
    )
}

/// A parsed, validated proposal: `(holder_type, confidence, rationale)`.
type ParsedProposal = (String, f64, Option<String>);

#[derive(Debug, serde::Deserialize)]
struct OwnershipClassificationJson {
    #[serde(rename = "holderType")]
    holder_type: Option<String>,
    confidence: Option<f64>,
    rationale: Option<String>,
}

/// Extract the first balanced `{...}` object from model text (providers sometimes
/// wrap JSON in prose or code fences).
fn first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

/// Parse the model's classification response. `Ok(None)` when the model returned
/// `unknown` or an out-of-taxonomy type (no proposal, no error); `Err` when the
/// response is not valid JSON of the expected shape (surfaced, never a stamp).
pub fn parse_ownership_classification_output(
    text: &str,
    provider_label: &str,
) -> Result<Option<ParsedProposal>, String> {
    let json_text = first_json_object(text).ok_or_else(|| {
        format!("{provider_label} ownership classification response has no JSON object")
    })?;
    let parsed: OwnershipClassificationJson = serde_json::from_str(json_text)
        .map_err(|error| format!("{provider_label} ownership classification JSON: {error}"))?;

    let holder_type = parsed
        .holder_type
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let holder_type = match holder_type {
        Some(value)
            if value != "unknown" && ALLOWED_HOLDER_TYPES.iter().any(|key| *key == value) =>
        {
            value
        }
        _ => return Ok(None),
    };
    let confidence = parsed.confidence.unwrap_or(0.0).clamp(0.0, 1.0);
    let rationale = parsed
        .rationale
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    Ok(Some((holder_type, confidence, rationale)))
}

/// Residual work: `(company_id, unclassified holders)` after the deterministic pass.
fn residual_work(state: &AppState) -> Result<Vec<(String, Vec<String>)>, String> {
    let ownership = state.ownership();
    // Free deterministic pass first: stamp every dictionary/heuristic hit so the
    // model only ever sees true residuals.
    for company_id in ownership
        .companies_with_unclassified_holders()
        .map_err(|error| error.to_string())?
    {
        ownership
            .classify_unclassified_for_company(&company_id)
            .map_err(|error| error.to_string())?;
    }
    let mut work = Vec::new();
    for company_id in ownership
        .companies_with_unclassified_holders()
        .map_err(|error| error.to_string())?
    {
        let holders = ownership
            .unclassified_holders(&company_id)
            .map_err(|error| error.to_string())?;
        if !holders.is_empty() {
            work.push((company_id, holders));
        }
    }
    Ok(work)
}

/// Propose types for the residual work with an already-built provider. Isolated
/// from provider resolution so tests can drive it with a scripted mock.
async fn propose_types(
    state: &AppState,
    provider: &dyn AiAnalysisProvider,
    work: &[(String, Vec<String>)],
) -> Result<AiOwnershipClassificationSummary, String> {
    let ownership = state.ownership();
    let mut summary = AiOwnershipClassificationSummary {
        examined: 0,
        proposed: 0,
        skipped: 0,
        errors: 0,
    };
    for (company_id, holders) in work {
        for holder in holders {
            summary.examined += 1;
            let prompt = ownership_classification_prompt(holder);
            let document = AnalysisDocument::Text {
                text: holder.clone(),
            };
            let text = match provider.complete_document(&prompt, &document).await {
                Ok(text) => text,
                Err(error) => {
                    log::warn!(
                        "module=ownership_classification stage=provider_error companyId={} holder={} error={}",
                        company_id,
                        holder,
                        error
                    );
                    summary.errors += 1;
                    continue;
                }
            };
            match parse_ownership_classification_output(&text, provider.provider_id()) {
                Ok(Some((holder_type, confidence, rationale))) => {
                    let created = ownership
                        .upsert_holder_type_proposal(NewHolderTypeProposal {
                            company_id: company_id.clone(),
                            holder_name_normalized: holder.clone(),
                            proposed_type: holder_type,
                            confidence: Some(confidence),
                            rationale,
                            provider_id: Some(provider.provider_id().to_owned()),
                            model: Some(provider.model().to_owned()),
                        })
                        .map_err(|error| error.to_string())?;
                    if created {
                        summary.proposed += 1;
                    } else {
                        summary.skipped += 1;
                    }
                }
                Ok(None) => summary.skipped += 1,
                Err(error) => {
                    log::warn!(
                        "module=ownership_classification stage=parse_error companyId={} holder={} error={}",
                        company_id,
                        holder,
                        error
                    );
                    summary.errors += 1;
                }
            }
        }
    }
    Ok(summary)
}

/// Run AI holder-type classification over every company's residual holders.
pub async fn run_ai_ownership_classification(
    state: &AppState,
) -> Result<AiOwnershipClassificationSummary, String> {
    let work = residual_work(state)?;
    let total: usize = work.iter().map(|(_, holders)| holders.len()).sum();
    // Nothing left after the deterministic pass: skip provider resolution so an
    // unconfigured provider never turns a clean no-op into a failure.
    if total == 0 {
        return Ok(AiOwnershipClassificationSummary {
            examined: 0,
            proposed: 0,
            skipped: 0,
            errors: 0,
        });
    }

    let members = crate::jobs::resolve_capability_members(
        state,
        AiCapability::OwnershipHolderClassification,
    )?;
    if members.is_empty() {
        return Err(
            "no general AI analysis provider is configured for ownership holder classification"
                .to_owned(),
        );
    }
    let timeout_seconds = state
        .get_settings()
        .map_err(|error| error.to_string())?
        .ai_providers
        .general_analysis_timeout_seconds;
    let provider = crate::jobs::build_capability_provider(
        state,
        AiCapability::OwnershipHolderClassification,
        timeout_seconds,
    )?;

    let summary = propose_types(state, provider.as_ref(), &work).await?;
    log::info!(
        "module=ownership_classification stage=done providerId={} examined={} proposed={} skipped={} errors={}",
        provider.provider_id(),
        summary.examined,
        summary.proposed,
        summary.skipped,
        summary.errors
    );
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::analysis::{
        AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest,
        ResearchBriefProviderOutput, ResearchBriefRequest, ResearchDigestRequest,
    };
    use crate::storage::{open_in_memory_database, AppState, NewCompany, NewOwnershipStake};
    use async_trait::async_trait;
    use tauri::async_runtime::block_on;

    /// A scripted mock provider: `complete_document` returns whatever it is told,
    /// so tests can exercise the valid / unknown / malformed branches without a
    /// network. The non-classification methods are never called on this path.
    struct ScriptedProvider {
        response: String,
    }

    #[async_trait]
    impl AiAnalysisProvider for ScriptedProvider {
        fn provider_id(&self) -> &'static str {
            "test_scripted"
        }
        fn model(&self) -> &str {
            "scripted-v1"
        }
        async fn analyze(
            &self,
            _request: &AnalysisRequest,
        ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
            unimplemented!("not used by ownership classification")
        }
        async fn generate_research_brief(
            &self,
            _request: &ResearchBriefRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            unimplemented!("not used by ownership classification")
        }
        async fn generate_research_digest(
            &self,
            _request: &ResearchDigestRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            unimplemented!("not used by ownership classification")
        }
        async fn complete_document(
            &self,
            _prompt: &str,
            _document: &AnalysisDocument,
        ) -> Result<String, AnalysisProviderError> {
            Ok(self.response.clone())
        }
    }

    fn seed_company(state: &AppState, ticker: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company should create")
            .id
    }

    fn append_unclassified(state: &AppState, company_id: &str, holder_raw: &str) {
        state
            .ownership()
            .append_snapshot(NewOwnershipStake {
                company_id: company_id.to_owned(),
                holder_name_raw: holder_raw.to_owned(),
                holder_type: None,
                capital_pct: Some("7.5".to_owned()),
                votes_pct: None,
                as_of: "2025-06-30".to_owned(),
                source: "report_document".to_owned(),
                report_document_id: None,
                feed_item_id: None,
            })
            .expect("snapshot should append");
    }

    #[test]
    fn parse_accepts_valid_type_and_rejects_out_of_taxonomy() {
        let ok = parse_ownership_classification_output(
            r#"{"holderType":"ofe_pension","confidence":0.9,"rationale":"pension fund"}"#,
            "test",
        )
        .expect("valid JSON parses");
        assert_eq!(
            ok,
            Some((
                "ofe_pension".to_owned(),
                0.9,
                Some("pension fund".to_owned())
            ))
        );

        let unknown = parse_ownership_classification_output(
            r#"{"holderType":"unknown","confidence":0.1}"#,
            "test",
        )
        .expect("valid JSON parses");
        assert_eq!(unknown, None, "unknown maps to no proposal");

        let out_of_taxonomy = parse_ownership_classification_output(
            r#"{"holderType":"made_up","confidence":0.99}"#,
            "test",
        )
        .expect("valid JSON parses");
        assert_eq!(out_of_taxonomy, None, "unlisted type maps to no proposal");

        let malformed = parse_ownership_classification_output("not json at all", "test");
        assert!(
            malformed.is_err(),
            "malformed response is an error, never a stamp"
        );
    }

    #[test]
    fn unknown_holder_creates_pending_proposal_without_stamping() {
        let connection = open_in_memory_database().expect("db init");
        let state = AppState::new(connection);
        let company = seed_company(&state, "ACME");
        append_unclassified(&state, &company, "ULTRO S.a.r.l.");

        let work = residual_work(&state).expect("residual work");
        assert_eq!(
            work,
            vec![(company.clone(), vec!["ULTRO S.A.R.L.".to_owned()])]
        );

        let provider = ScriptedProvider {
            response: r#"{"holderType":"other_institutional","confidence":0.7,"rationale":"foreign fund"}"#
                .to_owned(),
        };
        let summary = block_on(propose_types(&state, &provider, &work)).expect("run");
        assert_eq!(summary.proposed, 1);
        assert_eq!(summary.errors, 0);

        // Proposal is pending; the stake stays NULL (never auto-applied).
        let proposals = state
            .ownership()
            .list_holder_type_proposals(&company, true)
            .expect("list");
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposed_type, "other_institutional");
        assert_eq!(proposals[0].status, "pending");
        let stakes = state.ownership().current_state(&company).expect("state");
        assert!(
            stakes.iter().all(|stake| stake.holder_type.is_none()),
            "pending proposal must not stamp the stake"
        );
    }

    #[test]
    fn confirm_applies_reject_leaves_null_and_rerun_does_not_duplicate() {
        let connection = open_in_memory_database().expect("db init");
        let state = AppState::new(connection);
        let company = seed_company(&state, "ACME");
        append_unclassified(&state, &company, "ULTRO S.a.r.l.");
        append_unclassified(&state, &company, "Mystery Holdings Inc.");

        let work = residual_work(&state).expect("work");
        let provider = ScriptedProvider {
            response: r#"{"holderType":"other_institutional","confidence":0.7}"#.to_owned(),
        };
        let first = block_on(propose_types(&state, &provider, &work)).expect("first run");
        assert_eq!(first.proposed, 2);

        // Re-run does not duplicate (idempotent per company+holder).
        let work2 = residual_work(&state).expect("work2");
        let second = block_on(propose_types(&state, &provider, &work2)).expect("second run");
        assert_eq!(
            second.proposed, 2,
            "re-run refreshes in place, still 2 holders"
        );
        let pending = state
            .ownership()
            .list_holder_type_proposals(&company, true)
            .expect("pending");
        assert_eq!(pending.len(), 2, "no duplicate proposals");

        // Confirm one → its type is applied to the stake.
        let ultro = pending
            .iter()
            .find(|p| p.holder_name_normalized == "ULTRO S.A.R.L.")
            .expect("ultro proposal");
        state
            .ownership()
            .confirm_holder_type_proposal(&ultro.id)
            .expect("confirm");
        let ultro_stake = state
            .ownership()
            .history(&company, Some("ULTRO S.A.R.L."))
            .expect("history");
        assert!(
            ultro_stake
                .iter()
                .all(|s| s.holder_type.as_deref() == Some("other_institutional")),
            "confirm applies the type across the holder's rows"
        );

        // Reject the other → stays NULL.
        let mystery = pending
            .iter()
            .find(|p| p.holder_name_normalized == "MYSTERY HOLDINGS INC.")
            .expect("mystery proposal");
        state
            .ownership()
            .reject_holder_type_proposal(&mystery.id)
            .expect("reject");
        let mystery_stake = state
            .ownership()
            .history(&company, Some("MYSTERY HOLDINGS INC."))
            .expect("history");
        assert!(
            mystery_stake.iter().all(|s| s.holder_type.is_none()),
            "reject leaves the holder unclassified"
        );

        // A confirmed proposal is never disturbed by a later run.
        let work3 = residual_work(&state).expect("work3");
        assert!(
            !work3
                .iter()
                .any(|(_, holders)| holders.iter().any(|h| h == "ULTRO S.A.R.L.")),
            "confirmed holder is no longer residual"
        );
    }

    #[test]
    fn malformed_response_yields_no_proposal_and_surfaces_error() {
        let connection = open_in_memory_database().expect("db init");
        let state = AppState::new(connection);
        let company = seed_company(&state, "ACME");
        append_unclassified(&state, &company, "ULTRO S.a.r.l.");

        let work = residual_work(&state).expect("work");
        let provider = ScriptedProvider {
            response: "sorry, I cannot help with that".to_owned(),
        };
        let summary = block_on(propose_types(&state, &provider, &work)).expect("run");
        assert_eq!(summary.proposed, 0, "malformed response never proposes");
        assert_eq!(summary.errors, 1, "the error is surfaced");
        let proposals = state
            .ownership()
            .list_holder_type_proposals(&company, false)
            .expect("list");
        assert!(
            proposals.is_empty(),
            "no proposal row on a malformed response"
        );
        let stakes = state.ownership().current_state(&company).expect("state");
        assert!(
            stakes.iter().all(|s| s.holder_type.is_none()),
            "never a stamp on a malformed response"
        );
    }

    #[test]
    fn deterministic_pass_removes_dictionary_hits_before_ai() {
        let connection = open_in_memory_database().expect("db init");
        let state = AppState::new(connection);
        let company = seed_company(&state, "ACME");
        append_unclassified(&state, &company, "Nationale-Nederlanden OFE"); // dictionary
        append_unclassified(&state, &company, "ULTRO S.a.r.l."); // residual

        let work = residual_work(&state).expect("work");
        let residual: Vec<String> = work.into_iter().flat_map(|(_, holders)| holders).collect();
        assert_eq!(
            residual,
            vec!["ULTRO S.A.R.L.".to_owned()],
            "the OFE is stamped by the dictionary; only the foreign entity remains for AI"
        );
    }
}
